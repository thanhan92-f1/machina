// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! VM edge L7: request starts on L7 ports (VM_L7_EVENTS) are parsed and
//! checked against the rules in force; the verdict lands in VM_L7_FLOW, so
//! the client's retransmit of a held segment passes or the flow stays
//! denied. Denied clients get an HTTP 403 (or a TCP RST), denied DNS
//! queries a REFUSED; allowed UDP DNS queries are forwarded from the host
//! and the answer is injected into the tap, where `toFQDNs` learning sees
//! it like any other reply.

use std::collections::HashMap as StdMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use aya::maps::{HashMap as BpfHash, MapData};

use super::*;
use crate::netpol::l7::{self, Matcher};

const FORBIDDEN: &[u8] =
    b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: 15\r\nConnection: close\r\n\r\nAccess denied\r\n";
const DNS_FORWARD_TIMEOUT: Duration = Duration::from_secs(2);
const RECENT_CAP: usize = 4096;
const TCP_FIN: u8 = 0x01;
const TCP_RST: u8 = 0x04;
const TCP_PSH: u8 = 0x08;
const TCP_ACK: u8 = 0x10;

type CacheKey = (u32, u32, bool, u8, u16);

#[derive(Clone)]
struct Compiled {
    matcher: Arc<Matcher>,
    kinds: Vec<&'static str>,
    source: Option<String>,
}

struct Decision {
    allowed: bool,
    kind: &'static str,
    summary: String,
    /// Payload bytes from `seq` the verdict covers; None = rest of the flow.
    consumed: Option<u32>,
}

pub(super) struct L7Worker {
    flows: BpfHash<MapData, FlowKey, VmL7Flow>,
    gen: u64,
    cache: StdMap<CacheKey, Option<Compiled>>,
    recent: StdMap<(FlowKey, u32), Instant>,
    sock: Option<Arc<OwnedFd>>,
}

fn compile(
    rules: &[VmEdgeL7Rule],
    (subject, peer, egress, proto, port): CacheKey,
) -> Option<Compiled> {
    let hits: Vec<&VmEdgeL7Rule> = rules
        .iter()
        .filter(|r| {
            r.subject_identity == subject
                && r.egress == egress
                && (r.peer_identity == 0 || r.peer_identity == peer)
        })
        .filter(|r| r.proto == 0 || r.proto == proto)
        .filter(|r| r.port == 0 || (r.port..=r.port_end.max(r.port)).contains(&port))
        .collect();
    if hits.is_empty() {
        return None;
    }
    let mut kinds: Vec<&'static str> = hits.iter().map(|r| r.rules.kind()).collect();
    kinds.sort();
    kinds.dedup();
    Some(Compiled {
        matcher: Arc::new(Matcher::new(hits.iter().map(|r| &r.rules))),
        kinds,
        source: hits.iter().find_map(|r| r.source.clone()),
    })
}

fn addr_ip(a: &[u8; ADDR_LEN], v6: bool) -> IpAddr {
    if v6 {
        IpAddr::V6(Ipv6Addr::from(*a))
    } else {
        IpAddr::V4(Ipv4Addr::new(a[12], a[13], a[14], a[15]))
    }
}

fn check_all(m: &Matcher, reqs: &[l7::Request]) -> (bool, String) {
    let mut parts = Vec::new();
    let mut ok_all = true;
    for r in reqs {
        let (ok, note) = m.check(r);
        let mut s = r.summary();
        if let Some(n) = note {
            s.push_str(&format!(" ({n})"));
        }
        if !ok {
            ok_all = false;
            s.push_str(" [denied]");
        }
        parts.push(s);
    }
    (ok_all, parts.join("; "))
}

fn deny(kind: &'static str, why: impl Into<String>) -> Decision {
    Decision {
        allowed: false,
        kind,
        summary: why.into(),
        consumed: None,
    }
}

fn decide(ev: &VmL7Event, data: &[u8], c: &Compiled) -> Decision {
    if ev.proto == policy::IPPROTO_UDP {
        return match dns::parse(data) {
            Some(msg) if !msg.is_response => {
                let req = l7::Request::Dns {
                    name: crate::netpol::fqdn::normalize(&msg.qname),
                };
                let (allowed, summary) = check_all(&c.matcher, &[req]);
                Decision {
                    allowed,
                    kind: "dns",
                    summary,
                    consumed: None,
                }
            }
            _ => deny("dns", "not a DNS query"),
        };
    }
    let skip = ev.skip as usize;
    let Some(p) = data.get(skip..).filter(|p| !p.is_empty()) else {
        return deny(c.kinds[0], "request start beyond the capture");
    };
    let kind = if c.kinds.contains(&"tls") && (p[0] == 0x16 || c.kinds.len() == 1) {
        "tls"
    } else if c.kinds.contains(&"dns") {
        "dns"
    } else if c.kinds.contains(&"kafka") {
        "kafka"
    } else {
        "http"
    };
    match kind {
        "tls" => match l7::parse_sni(p) {
            Ok(sni) => {
                let (allowed, summary) =
                    check_all(&c.matcher, &[l7::Request::Tls { server_name: sni }]);
                Decision {
                    allowed,
                    kind,
                    summary,
                    consumed: None,
                }
            }
            Err(e) => deny(kind, format!("TLS: {e}")),
        },
        "dns" => {
            if p.len() < 2 {
                return deny(kind, "short DNS-over-TCP message");
            }
            let n = u16::from_be_bytes([p[0], p[1]]) as usize;
            match dns::parse(&p[2..]) {
                Some(msg) if !msg.is_response => {
                    let req = l7::Request::Dns {
                        name: crate::netpol::fqdn::normalize(&msg.qname),
                    };
                    let (allowed, summary) = check_all(&c.matcher, &[req]);
                    Decision {
                        allowed,
                        kind,
                        summary,
                        consumed: Some((skip + 2 + n) as u32),
                    }
                }
                _ => deny(kind, "not a DNS query"),
            }
        }
        "kafka" => match l7::parse_kafka(p) {
            Ok(v) if !v.is_empty() => {
                let total: usize = v.iter().map(|(_, t)| *t).sum();
                let reqs: Vec<l7::Request> =
                    v.into_iter().map(|(r, _)| l7::Request::Kafka(r)).collect();
                let (allowed, summary) = check_all(&c.matcher, &reqs);
                Decision {
                    allowed,
                    kind,
                    summary,
                    consumed: Some((skip + total) as u32),
                }
            }
            Ok(_) => deny(kind, "empty Kafka request"),
            Err(e) => deny(kind, format!("Kafka: {e}")),
        },
        _ => {
            let mut reqs = Vec::new();
            let mut off = 0usize;
            let mut consumed = None;
            loop {
                match l7::parse_http(&p[off..]) {
                    Ok((r, Some(total))) => {
                        reqs.push(l7::Request::Http(r));
                        off += total;
                        if off >= p.len() {
                            consumed = Some((skip + off) as u32);
                            break;
                        }
                    }
                    Ok((r, None)) => {
                        reqs.push(l7::Request::Http(r));
                        break;
                    }
                    Err(e) if reqs.is_empty() => return deny(kind, format!("HTTP: {e}")),
                    Err(_) => return deny(kind, "pipelined HTTP request split across segments"),
                }
            }
            let (allowed, summary) = check_all(&c.matcher, &reqs);
            Decision {
                allowed,
                kind,
                summary,
                consumed,
            }
        }
    }
}

// ---- frames ------------------------------------------------------------------

fn csum_add(mut sum: u32, b: &[u8]) -> u32 {
    let mut i = 0;
    while i + 1 < b.len() {
        sum += u16::from_be_bytes([b[i], b[i + 1]]) as u32;
        i += 2;
    }
    if i < b.len() {
        sum += (b[i] as u32) << 8;
    }
    sum
}

fn csum_fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Ethernet + IP frame around a TCP or UDP `l4` segment with its checksum zeroed.
fn frame(
    eth_dst: &[u8],
    eth_src: &[u8],
    src: &[u8; ADDR_LEN],
    dst: &[u8; ADDR_LEN],
    v6: bool,
    proto: u8,
    mut l4: Vec<u8>,
) -> Vec<u8> {
    let csum_at = if proto == policy::IPPROTO_UDP { 6 } else { 16 };
    let len = l4.len();
    let mut pseudo = Vec::with_capacity(40);
    if v6 {
        pseudo.extend_from_slice(src);
        pseudo.extend_from_slice(dst);
        pseudo.extend_from_slice(&(len as u32).to_be_bytes());
        pseudo.extend_from_slice(&[0, 0, 0, proto]);
    } else {
        pseudo.extend_from_slice(&src[12..]);
        pseudo.extend_from_slice(&dst[12..]);
        pseudo.extend_from_slice(&[0, proto]);
        pseudo.extend_from_slice(&(len as u16).to_be_bytes());
    }
    let mut c = csum_fold(csum_add(csum_add(0, &pseudo), &l4));
    if c == 0 && proto == policy::IPPROTO_UDP {
        c = 0xffff;
    }
    l4[csum_at..csum_at + 2].copy_from_slice(&c.to_be_bytes());
    let mut f = Vec::with_capacity(14 + 40 + len);
    f.extend_from_slice(eth_dst);
    f.extend_from_slice(eth_src);
    if v6 {
        f.extend_from_slice(&[0x86, 0xdd, 0x60, 0, 0, 0]);
        f.extend_from_slice(&(len as u16).to_be_bytes());
        f.extend_from_slice(&[proto, 64]);
        f.extend_from_slice(src);
        f.extend_from_slice(dst);
    } else {
        let mut ip = vec![0x45, 0];
        ip.extend_from_slice(&((20 + len) as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0x40, 0, 64, proto, 0, 0]);
        ip.extend_from_slice(&src[12..]);
        ip.extend_from_slice(&dst[12..]);
        let hc = csum_fold(csum_add(0, &ip));
        ip[10..12].copy_from_slice(&hc.to_be_bytes());
        f.extend_from_slice(&[0x08, 0x00]);
        f.extend_from_slice(&ip);
    }
    f.extend_from_slice(&l4);
    f
}

fn tcp_seg(sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, payload: &[u8]) -> Vec<u8> {
    let mut t = Vec::with_capacity(20 + payload.len());
    t.extend_from_slice(&sport.to_be_bytes());
    t.extend_from_slice(&dport.to_be_bytes());
    t.extend_from_slice(&seq.to_be_bytes());
    t.extend_from_slice(&ack.to_be_bytes());
    t.extend_from_slice(&[0x50, flags]);
    t.extend_from_slice(&(if flags & TCP_RST != 0 { 0u16 } else { 65535 }).to_be_bytes());
    t.extend_from_slice(&[0, 0, 0, 0]);
    t.extend_from_slice(payload);
    t
}

fn udp_dgram(sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let mut u = Vec::with_capacity(8 + payload.len());
    u.extend_from_slice(&sport.to_be_bytes());
    u.extend_from_slice(&dport.to_be_bytes());
    u.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
    u.extend_from_slice(&[0, 0]);
    u.extend_from_slice(payload);
    u
}

/// REFUSED answer to a DNS query (header + question only).
fn dns_refused(q: &[u8]) -> Option<Vec<u8>> {
    if q.len() < 12 {
        return None;
    }
    let mut i = 12;
    while i < q.len() && q[i] != 0 {
        if q[i] & 0xc0 != 0 {
            return None;
        }
        i += 1 + q[i] as usize;
    }
    let end = i + 5;
    if end > q.len() {
        return None;
    }
    let mut r = q[..end].to_vec();
    r[2] = 0x80 | (q[2] & 0x79);
    r[3] = 0x80 | 5;
    r[4] = 0;
    r[5] = 1;
    r[6..12].fill(0);
    Some(r)
}

fn send_frame(sock: &OwnedFd, ifindex: u32, f: &[u8]) {
    if ifindex == 0 || f.len() < 14 {
        return;
    }
    let mut sa: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    sa.sll_family = libc::AF_PACKET as u16;
    sa.sll_protocol = u16::from_be_bytes([f[12], f[13]]).to_be();
    sa.sll_ifindex = ifindex as i32;
    sa.sll_halen = 6;
    sa.sll_addr[..6].copy_from_slice(&f[..6]);
    let r = unsafe {
        libc::sendto(
            sock.as_raw_fd(),
            f.as_ptr().cast(),
            f.len(),
            0,
            (&sa as *const libc::sockaddr_ll).cast(),
            std::mem::size_of::<libc::sockaddr_ll>() as u32,
        )
    };
    if r < 0 {
        tracing::debug!(
            "vm l7: inject on ifindex {ifindex}: {}",
            std::io::Error::last_os_error()
        );
    }
}

/// Bridge (or other master) of a tap: where frames towards the far side go.
fn master_of(ifindex: u32) -> u32 {
    let mut buf = [0 as libc::c_char; libc::IF_NAMESIZE];
    if unsafe { libc::if_indextoname(ifindex, buf.as_mut_ptr()) }.is_null() {
        return 0;
    }
    let name = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    std::fs::read_link(format!("/sys/class/net/{name}/master"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .and_then(|m| if_nametoindex(&m))
        .unwrap_or(0)
}

impl L7Worker {
    pub(super) fn new(flows: BpfHash<MapData, FlowKey, VmL7Flow>) -> Self {
        let fd = unsafe { libc::socket(libc::AF_PACKET, libc::SOCK_RAW, 0) };
        let sock = if fd >= 0 {
            Some(Arc::new(unsafe { OwnedFd::from_raw_fd(fd) }))
        } else {
            tracing::warn!(
                "vm l7: AF_PACKET socket: {}; denied flows get no reply",
                std::io::Error::last_os_error()
            );
            None
        };
        L7Worker {
            flows,
            gen: u64::MAX,
            cache: StdMap::new(),
            recent: StdMap::new(),
            sock,
        }
    }

    pub(super) fn on_event(
        &mut self,
        sh: &SharedState,
        bus: &broadcast::Sender<StreamEvent>,
        b: &[u8],
    ) {
        if b.len() < std::mem::size_of::<VmL7Event>() {
            return;
        }
        let ev = unsafe { std::ptr::read_unaligned(b.as_ptr() as *const VmL7Event) };
        let (rules, gen) = {
            let s = lock(sh);
            (s.vm_l7_rules.clone(), s.vm_l7_gen)
        };
        if gen != self.gen {
            self.cache.clear();
            self.gen = gen;
        }
        let key = FlowKey {
            proto: ev.proto,
            local_port: ev.sport,
            remote_port: ev.dport,
            local: ev.src,
            remote: ev.dst,
            ..FlowKey::default()
        };
        let now = Instant::now();
        if ev.proto == policy::IPPROTO_TCP {
            if self
                .recent
                .get(&(key, ev.seq))
                .is_some_and(|t| now.duration_since(*t) < Duration::from_secs(3))
            {
                return;
            }
            if self.recent.len() >= RECENT_CAP {
                self.recent
                    .retain(|_, t| now.duration_since(*t) < Duration::from_secs(3));
            }
            self.recent.insert((key, ev.seq), now);
        }
        let egress = ev.from_vm != 0;
        let held = ev.held != 0;
        let ck = (ev.subject, ev.peer, egress, ev.proto, ev.dport);
        let compiled = self
            .cache
            .entry(ck)
            .or_insert_with(|| compile(&rules, ck))
            .clone();
        let Some(c) = compiled else {
            if ev.proto == policy::IPPROTO_TCP {
                let open = VmL7Flow {
                    allow_seq: ev.seq,
                    pass_until: ev.seq,
                    pending_seq: ev.seq,
                    state: L7S_OPEN,
                    _pad: [0; 3],
                };
                let _ = self.flows.insert(key, open, 0);
            }
            return;
        };
        let cap = (ev.cap as usize).min(VM_L7_CAP);
        let data = &ev.data[..cap];
        let d = decide(&ev, data, &c);

        if ev.proto == policy::IPPROTO_TCP {
            let cur = self.flows.get(&key, 0).ok();
            if let Some(cur) = cur.filter(|c| {
                c.pass_until != c.allow_seq && (ev.seq.wrapping_sub(c.allow_seq) as i32) < 0
            }) {
                if held {
                    let _ = self.flows.insert(
                        key,
                        VmL7Flow {
                            state: L7S_NONE,
                            ..cur
                        },
                        0,
                    );
                }
                return;
            }
            let v = if d.allowed || !held {
                let (until, state) = match d.consumed {
                    Some(n) => (ev.seq.wrapping_add(n.max(1)), L7S_NONE),
                    None => (ev.seq, L7S_OPEN),
                };
                VmL7Flow {
                    allow_seq: ev.seq,
                    pass_until: until,
                    pending_seq: ev.seq,
                    state,
                    _pad: [0; 3],
                }
            } else {
                let base = cur.unwrap_or_default();
                VmL7Flow {
                    pending_seq: ev.seq,
                    state: L7S_DENIED,
                    ..base
                }
            };
            if let Err(e) = self.flows.insert(key, v, 0) {
                tracing::warn!("vm l7: verdict write: {e}");
            }
        }

        if held {
            if let Some(sock) = self.sock.clone() {
                self.answer(&ev, data, &d, sock);
            }
        }
        self.record(sh, bus, &ev, &d, held, c.source.clone());
    }

    fn answer(&self, ev: &VmL7Event, data: &[u8], d: &Decision, sock: Arc<OwnedFd>) {
        let v6 = ev.v6 != 0;
        let (vm_mac, far_mac) = (&ev.mac[6..12], &ev.mac[0..6]);
        let from_vm = ev.from_vm != 0;
        let (client_if, server_if) = if from_vm {
            (ev.ifindex, master_of(ev.ifindex))
        } else {
            (master_of(ev.ifindex), ev.ifindex)
        };
        if ev.proto == policy::IPPROTO_UDP {
            if d.allowed {
                if (ev.len as usize) > data.len() {
                    return;
                }
                let query = data.to_vec();
                let (src, dst, sport, dport, ifindex) =
                    (ev.src, ev.dst, ev.sport, ev.dport, ev.ifindex);
                let (eth_dst, eth_src) = (vm_mac.to_vec(), far_mac.to_vec());
                std::thread::spawn(move || {
                    let server = addr_ip(&dst, v6);
                    let bind = if v6 { "[::]:0" } else { "0.0.0.0:0" };
                    let Ok(u) = UdpSocket::bind(bind) else { return };
                    let _ = u.set_read_timeout(Some(DNS_FORWARD_TIMEOUT));
                    if u.send_to(&query, (server, dport)).is_err() {
                        return;
                    }
                    let mut buf = [0u8; 4096];
                    let Ok((n, from)) = u.recv_from(&mut buf) else {
                        return;
                    };
                    if from.ip() != server || n < 12 || buf[..2] != query[..2] {
                        return;
                    }
                    let f = frame(
                        &eth_dst,
                        &eth_src,
                        &dst,
                        &src,
                        v6,
                        policy::IPPROTO_UDP,
                        udp_dgram(dport, sport, &buf[..n]),
                    );
                    send_frame(&sock, ifindex, &f);
                });
            } else if let Some(r) = dns_refused(data) {
                let f = frame(
                    vm_mac,
                    far_mac,
                    &ev.dst,
                    &ev.src,
                    v6,
                    policy::IPPROTO_UDP,
                    udp_dgram(ev.dport, ev.sport, &r),
                );
                send_frame(&sock, ev.ifindex, &f);
            }
            return;
        }
        if d.allowed {
            return;
        }
        let client_ack = ev.seq.wrapping_add(ev.len);
        let to_client = if d.kind == "http" {
            tcp_seg(
                ev.dport,
                ev.sport,
                ev.ack,
                client_ack,
                TCP_PSH | TCP_ACK | TCP_FIN,
                FORBIDDEN,
            )
        } else {
            tcp_seg(
                ev.dport,
                ev.sport,
                ev.ack,
                client_ack,
                TCP_RST | TCP_ACK,
                &[],
            )
        };
        let f = frame(
            &ev.mac[6..12],
            &ev.mac[0..6],
            &ev.dst,
            &ev.src,
            v6,
            policy::IPPROTO_TCP,
            to_client,
        );
        send_frame(&sock, client_if, &f);
        let to_server = tcp_seg(ev.sport, ev.dport, ev.seq, 0, TCP_RST, &[]);
        let f = frame(
            &ev.mac[0..6],
            &ev.mac[6..12],
            &ev.src,
            &ev.dst,
            v6,
            policy::IPPROTO_TCP,
            to_server,
        );
        send_frame(&sock, server_if, &f);
    }

    fn record(
        &self,
        sh: &SharedState,
        bus: &broadcast::Sender<StreamEvent>,
        ev: &VmL7Event,
        d: &Decision,
        held: bool,
        source: Option<String>,
    ) {
        let rec = {
            let mut s = lock(sh);
            let (iface, tap_vm) = s.iface(ev.ifindex);
            let idx = &s.vm_flow_index;
            let egress = ev.from_vm != 0;
            let subject = idx.name(ev.subject).cloned();
            let peer = idx.name(ev.peer).cloned();
            let vm = tap_vm
                .or_else(|| subject.as_ref().map(|s| s.0.clone()))
                .unwrap_or_default();
            let (src_side, dst_side) = if egress {
                (subject, peer)
            } else {
                (peer, subject)
            };
            let (src_id, dst_id) = if egress {
                (ev.subject, ev.peer)
            } else {
                (ev.peer, ev.subject)
            };
            let rec = VmFlowRecord {
                ts: mono_to_rfc3339(ev.ts_ns),
                iface: iface.unwrap_or_default(),
                vm,
                direction: if egress { "egress" } else { "ingress" }.into(),
                src: fmt_addr(&ev.src),
                src_port: ev.sport,
                dst: fmt_addr(&ev.dst),
                dst_port: ev.dport,
                src_vm: src_side.as_ref().map(|s| s.0.clone()),
                dst_vm: dst_side.as_ref().map(|s| s.0.clone()),
                src_labels: src_side.map(|s| s.1).unwrap_or_default(),
                dst_labels: dst_side.map(|s| s.1).unwrap_or_default(),
                src_identity: src_id,
                dst_identity: dst_id,
                proto: proto_name(ev.proto).into(),
                bytes: ev.len,
                verdict: if d.allowed {
                    "FORWARDED"
                } else if held {
                    "DROPPED"
                } else {
                    "AUDIT"
                }
                .into(),
                drop_reason: (!d.allowed).then(|| "l7-deny".into()),
                policy: source,
                l7_type: Some(d.kind.into()),
                l7: Some(d.summary.clone()),
                ..Default::default()
            };
            Shared::push_capped(&mut s.vm_flows, rec.clone(), VM_FLOW_STORE_CAP);
            rec
        };
        publish(bus, "flow", &rec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_and_refused() {
        let mut src = [0u8; ADDR_LEN];
        let mut dst = [0u8; ADDR_LEN];
        src[10] = 0xff;
        src[11] = 0xff;
        dst[10] = 0xff;
        dst[11] = 0xff;
        src[12..].copy_from_slice(&[10, 0, 0, 1]);
        dst[12..].copy_from_slice(&[10, 0, 0, 2]);
        let f = frame(
            &[1; 6],
            &[2; 6],
            &src,
            &dst,
            false,
            policy::IPPROTO_TCP,
            tcp_seg(80, 5555, 1, 2, TCP_RST, &[]),
        );
        assert_eq!(f.len(), 54);
        assert_eq!(
            csum_fold(csum_add(0, &f[14..34])),
            0,
            "IPv4 header checksum"
        );
        let mut pseudo = vec![10, 0, 0, 1, 10, 0, 0, 2, 0, 6, 0, 20];
        pseudo.extend_from_slice(&f[34..]);
        assert_eq!(csum_fold(csum_add(0, &pseudo)), 0, "TCP checksum");

        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
        q.extend_from_slice(b"\x03www\x07example\x03com\x00\x00\x01\x00\x01");
        let r = dns_refused(&q).unwrap();
        let m = dns::parse(&r).unwrap();
        assert!(
            m.is_response
                && m.rcode == 5
                && m.id == 0x1234
                && m.qname.trim_end_matches('.') == "www.example.com"
        );
    }
}

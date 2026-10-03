// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Ring-buffer readers: decode kernel events into records, run detectors,
//! feed the event store and live subscribers.

use std::net::IpAddr;

use aya::maps::{MapData, RingBuf};
use tokio::sync::Notify;

use super::*;

fn decode<T: Copy>(b: &[u8]) -> Option<T> {
    (b.len() >= std::mem::size_of::<T>())
        .then(|| unsafe { std::ptr::read_unaligned(b.as_ptr() as *const T) })
}

fn cstr(b: &[u8]) -> String {
    let n = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..n]).into_owned()
}

fn wall_now() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64 / 1000.0
}

/// Drive one ring buffer until the process exits.
pub(super) fn spawn_reader<F>(rb: RingBuf<MapData>, name: &'static str, mut on_item: F)
where
    F: FnMut(&[u8]) + Send + 'static,
{
    tokio::spawn(async move {
        let mut fd = match AsyncFd::new(rb) {
            Ok(fd) => fd,
            Err(e) => {
                tracing::error!("ringbuf {name}: {e}");
                return;
            }
        };
        loop {
            let mut guard = match fd.readable_mut().await {
                Ok(g) => g,
                Err(e) => {
                    tracing::error!("ringbuf {name}: {e}");
                    return;
                }
            };
            let rb = guard.get_inner_mut();
            while let Some(item) = rb.next() {
                on_item(&item);
            }
            guard.clear_ready();
        }
    });
}

fn net_kind(k: u32) -> &'static str {
    match k {
        NET_EV_FLOW_OPEN => "flow_open",
        NET_EV_FLOW_CLOSE => "flow_close",
        NET_EV_DENY => "deny",
        NET_EV_ALLOW_MISS => "allow_miss",
        NET_EV_QOS_DROP => "qos_drop",
        _ => "unknown",
    }
}

fn verdict_name(v: u32) -> &'static str {
    match v {
        VERDICT_DROP => "drop",
        VERDICT_OBSERVED => "observed",
        _ => "pass",
    }
}

pub(super) fn on_net(sh: &SharedState, bus: &broadcast::Sender<StreamEvent>, b: &[u8]) {
    let Some(ev) = decode::<NetEvent>(b) else {
        return;
    };
    let k = ev.key;
    let local = fmt_addr(&k.local);
    let remote = fmt_addr(&k.remote);
    let mut anomalies = Vec::new();
    let rec = {
        let mut s = lock(sh);
        let (iface, vm) = s.iface(k.ifindex);
        let is_open = ev.kind == NET_EV_FLOW_OPEN;
        let rec = NetEventRecord {
            ts: mono_to_rfc3339(ev.ts_ns),
            kind: net_kind(ev.kind).into(),
            verdict: verdict_name(ev.verdict).into(),
            policy_id: if is_open {
                None
            } else {
                s.policy_labels.get(&ev.policy_id).cloned()
            },
            iface: iface.clone(),
            vm: vm.clone(),
            proto: proto_name(k.proto).into(),
            local: local.clone(),
            local_port: k.local_port,
            remote: remote.clone(),
            remote_port: k.remote_port,
            pkt_len: ev.pkt_len,
            tx_bytes: ev.tx_bytes,
            rx_bytes: ev.rx_bytes,
        };
        s.counters.net_events += 1;
        match ev.verdict {
            VERDICT_DROP => s.counters.drops += 1,
            VERDICT_OBSERVED => s.counters.observed += 1,
            _ => {}
        }
        let ctx = Ctx { vm, iface };
        let now = wall_now();
        if is_open {
            s.counters.flows_opened += 1;
            let origin_local = ev.policy_id == ORIGIN_LOCAL as u32;
            anomalies.extend(s.detector.on_flow_open(
                now,
                &ctx,
                &local,
                &remote,
                k.remote_port,
                k.local_port,
                origin_local,
            ));
        } else if matches!(ev.kind, NET_EV_DENY | NET_EV_ALLOW_MISS) {
            anomalies.extend(s.detector.on_deny(now, &ctx, &local, &remote));
        }
        Shared::push_capped(&mut s.net, rec.clone(), NET_STORE_CAP);
        for a in &anomalies {
            s.counters.anomalies += 1;
            Shared::push_capped(&mut s.anomalies, a.clone(), ANOMALY_STORE_CAP);
        }
        rec
    };
    publish(bus, "net", &rec);
    for a in anomalies {
        publish(bus, "anomaly", &a);
    }
}

pub(super) fn on_dns(
    sh: &SharedState,
    bus: &broadcast::Sender<StreamEvent>,
    wake: &Notify,
    b: &[u8],
) {
    let Some(ev) = decode::<DnsEvent>(b) else {
        return;
    };
    let n = (ev.payload_len as usize).min(DNS_PAYLOAD_LEN);
    let Some(msg) = dns::parse(&ev.payload[..n]) else {
        return;
    };
    let local = fmt_addr(&ev.local);
    let remote = fmt_addr(&ev.remote);
    let mut anomaly = None;
    let mut queued = false;
    let rec = {
        let mut s = lock(sh);
        let (iface, vm) = s.iface(ev.ifindex);
        let rec = DnsRecord {
            ts: mono_to_rfc3339(ev.ts_ns),
            iface: iface.clone(),
            vm: vm.clone(),
            client: local.clone(),
            server: remote,
            id: msg.id,
            is_response: msg.is_response,
            qname: msg.qname.clone(),
            qtype: dns::rtype_name(msg.qtype),
            rcode: dns::rcode_name(msg.rcode).into(),
            answers: msg.answers.clone(),
        };
        s.counters.dns_events += 1;
        if !msg.is_response {
            anomaly = s
                .detector
                .on_dns_query(wall_now(), &Ctx { vm, iface }, &local, &msg.qname);
        } else {
            let hits: Vec<(u32, u32)> = s
                .dns_denies
                .iter()
                .filter(|(_, (suffix, _))| {
                    policy::dns_suffix_match(&msg.qname, suffix)
                        || msg.answers.iter().any(|a| policy::dns_suffix_match(&a.name, suffix))
                })
                .map(|(num, (_, scope))| (*num, *scope))
                .collect();
            for (num, scope) in hits {
                for a in &msg.answers {
                    if a.rtype != "A" && a.rtype != "AAAA" {
                        continue;
                    }
                    if let Ok(ip) = a.data.parse::<IpAddr>() {
                        let prefix = Prefix {
                            addr: policy::ip_to_addr(ip),
                            bits: 128,
                        };
                        s.dns_block_queue.push((scope, prefix, num));
                        queued = true;
                    }
                }
            }
        }
        Shared::push_capped(&mut s.dns, rec.clone(), DNS_STORE_CAP);
        if let Some(a) = &anomaly {
            s.counters.anomalies += 1;
            Shared::push_capped(&mut s.anomalies, a.clone(), ANOMALY_STORE_CAP);
        }
        rec
    };
    if queued {
        wake.notify_one();
    }
    publish(bus, "dns", &rec);
    if let Some(a) = anomaly {
        publish(bus, "anomaly", &a);
    }
}

fn proc_kind(k: u32) -> &'static str {
    match k {
        PROC_EV_EXEC => "exec",
        PROC_EV_EXIT => "exit",
        PROC_EV_FORK => "fork",
        PROC_EV_FILE_OPEN => "file_open",
        PROC_EV_CONNECT => "connect",
        PROC_EV_CAP_DENIED => "cap_denied",
        _ => "unknown",
    }
}

pub(super) fn on_proc(sh: &SharedState, bus: &broadcast::Sender<StreamEvent>, b: &[u8]) {
    let Some(ev) = decode::<ProcEvent>(b) else {
        return;
    };
    let path = cstr(&ev.path);
    let comm = cstr(&ev.comm);
    let (ppid, cmdline) = if ev.kind == PROC_EV_EXEC {
        attribution::proc_details(ev.tgid)
    } else {
        (None, None)
    };
    let mut anomaly = None;
    let rec = {
        let mut s = lock(sh);
        let cgroup = s.cgroups.lookup(ev.cgroup_id);
        let info = cgroup
            .as_deref()
            .map(attribution::classify_cgroup)
            .unwrap_or_default();
        let denied = ev.flags & PROC_FLAG_EXEC_DENIED != 0;
        let is_connect = ev.kind == PROC_EV_CONNECT;
        let rec = ProcRecord {
            ts: mono_to_rfc3339(ev.ts_ns),
            kind: proc_kind(ev.kind).into(),
            pid: ev.pid,
            tgid: ev.tgid,
            ppid,
            child_pid: (ev.kind == PROC_EV_FORK).then_some(ev.child_pid),
            uid: ev.uid,
            gid: ev.gid,
            comm: comm.clone(),
            path: (!path.is_empty()).then(|| path.clone()),
            cmdline,
            cgroup_id: ev.cgroup_id,
            cgroup,
            unit: info.unit,
            vm: info.vm.clone(),
            container: info.container,
            denied,
            killed: ev.flags & PROC_FLAG_EXEC_KILLED != 0,
            policy_id: denied
                .then(|| s.policy_labels.get(&ev.policy_id).cloned())
                .flatten(),
            open_flags: (ev.kind == PROC_EV_FILE_OPEN).then_some(ev.open_flags),
            capability: (ev.kind == PROC_EV_CAP_DENIED).then(|| policy::cap_name(ev.open_flags)),
            proto: is_connect.then(|| proto_name(ev.proto as u8).to_string()),
            saddr: is_connect.then(|| fmt_addr(&ev.saddr)),
            sport: is_connect.then_some(ev.sport),
            daddr: is_connect.then(|| fmt_addr(&ev.daddr)),
            dport: is_connect.then_some(ev.dport),
        };
        s.counters.proc_events += 1;
        if ev.kind == PROC_EV_EXEC {
            let ctx = Ctx {
                vm: info.vm,
                iface: None,
            };
            anomaly = s.detector.on_exec(wall_now(), &ctx, &path, &comm, ev.tgid);
        }
        Shared::push_capped(&mut s.procs, rec.clone(), PROC_STORE_CAP);
        if let Some(a) = &anomaly {
            s.counters.anomalies += 1;
            Shared::push_capped(&mut s.anomalies, a.clone(), ANOMALY_STORE_CAP);
        }
        rec
    };
    publish(bus, "proc", &rec);
    if let Some(a) = anomaly {
        publish(bus, "anomaly", &a);
    }
}

pub(super) fn on_capture(sh: &SharedState, b: &[u8]) {
    let Some(ev) = decode::<CaptureEvent>(b) else {
        return;
    };
    let n = (ev.cap_len as usize).min(CAPTURE_SNAPLEN);
    let mut s = lock(sh);
    let Some(c) = s
        .captures
        .values_mut()
        .find(|c| c.ifindex == ev.ifindex && !c.info.done)
    else {
        return;
    };
    if c.writer.packets() as usize >= c.info.max_packets {
        return;
    }
    c.writer.push(mono_to_epoch_us(ev.ts_ns), ev.pkt_len, &ev.data[..n]);
    c.info.packets = c.writer.packets();
    c.info.bytes = c.writer.bytes();
    s.counters.capture_packets += 1;
}

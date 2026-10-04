//! VM edge and QEMU sandbox.
//!
//! `mn_vm_edge_in/out` run on VM taps after the host datapath (`mn_tc_*`
//! returns TCX_NEXT): identity policy with conntrack (same key shape as
//! CNI NetworkPolicy; allow and deny entries, deny wins), Mbps / PPS token
//! buckets, per-tap counters and flow verdict events. Peers resolve by exact
//! address (VM_IPS), then longest CIDR prefix (VM_CIDR_IDS), then `world`.
//! For ICMP the policy port is the ICMP type + 1.
//!
//! L7 entries (VM_POLICY_L7) copy each request start to VM_L7_EVENTS; with
//! enforcement live the segment is held (dropped) until bpfd writes its
//! verdict to VM_L7_FLOW, so the client's retransmit passes or the flow is
//! answered by bpfd (RST / HTTP 403 / DNS REFUSED). UDP L7 (DNS) queries
//! are always handed to bpfd, which forwards allowed ones itself.
//! Authenticated entries (VM_POLICY_AUTH) admit new flows only while
//! VM_AUTH holds a live entry for the identity pair.
//!
//! `mn_qemu_device` (cgroup device) and `mn_qemu_egress` (cgroup_skb egress)
//! sandbox the QEMU process in its machine scope: a device-node allowlist
//! and loopback / migration / NBD-only IP egress. Both observe unless the
//! sandbox is set to enforce *and* the enforcement lease is live; libvirt's
//! own device program stays attached and the kernel ANDs the verdicts.

use aya_ebpf::{
    helpers::generated::{bpf_get_current_cgroup_id, bpf_skb_cgroup_id, bpf_skb_load_bytes},
    macros::{cgroup_device, cgroup_skb, classifier, map},
    maps::{lpm_trie::Key, Array, HashMap, LpmTrie, LruHashMap, PerCpuHashMap, RingBuf},
    programs::{DeviceContext, SkBuffContext, TcContext},
};
use machina_bpf_common::*;

use crate::{
    maps::IFACE_CFG,
    net::{TC_ACT_SHOT, TC_ACT_UNSPEC, emit_dns, enforce_active, now_ns},
    parse::*,
};

const IPPROTO_ICMP: u8 = 1;
const IPPROTO_ICMPV6: u8 = 58;
/// Conntrack "port" of the request side of an ICMP echo exchange.
const ICMP_ECHO_MARK: u16 = 0xffff;
const NSEC: u64 = 1_000_000_000;
/// Bucket depth: 100 ms of the configured rate (at least one jumbo frame).
const BURST_DIV: u64 = 10;
const MIN_BURST_BYTES: u64 = 64 * 1024;

#[map]
pub static VM_EDGE: HashMap<u32, VmEdgeCfg> = HashMap::with_max_entries(4096, 0);

/// VM address (16-byte, IPv4-mapped) → group identity, for peers.
#[map]
pub static VM_IPS: HashMap<[u8; ADDR_LEN], u32> = HashMap::with_max_entries(65536, 0);

/// CIDR peers, other hypervisors' prefixes → identity.
#[map]
pub static VM_CIDR_IDS: LpmTrie<[u8; ADDR_LEN], u32> = LpmTrie::with_max_entries(16384, 0);

/// Value: VM_POLICY_ALLOW or VM_POLICY_DENY.
#[map]
pub static VM_POLICY: HashMap<PolicyKey, u32> = HashMap::with_max_entries(131072, 0);

#[map]
pub static VM_FLOW_EVENTS: RingBuf = RingBuf::with_byte_size(1 << 21, 0);

/// Drop / audit event throttle: flow → last emitted.
#[map]
pub static VM_FLOW_SEEN: LruHashMap<FlowKey, u64> = LruHashMap::with_max_entries(16384, 0);

/// Conntrack: FlowKey (ifindex 0, local = originator) → last seen.
#[map]
pub static VM_CT: LruHashMap<FlowKey, u64> = LruHashMap::with_max_entries(131072, 0);

#[map]
pub static VM_L7_EVENTS: RingBuf = RingBuf::with_byte_size(1 << 22, 0);

#[map]
pub static VM_L7_FLOW: LruHashMap<FlowKey, VmL7Flow> = LruHashMap::with_max_entries(65536, 0);

#[map]
pub static VM_AUTH: LruHashMap<VmAuthKey, u64> = LruHashMap::with_max_entries(65536, 0);

#[map]
pub static VM_BUCKETS: HashMap<u32, VmBucket> = HashMap::with_max_entries(8192, 0);

#[map]
pub static VM_EDGE_STATS: PerCpuHashMap<u32, VmEdgeStats> = PerCpuHashMap::with_max_entries(4096, 0);

#[map]
pub static QEMU_SANDBOX: Array<QemuSandboxCfg> = Array::with_max_entries(1, 0);

/// Allowed QEMU egress destination ports (host order) besides loopback.
#[map]
pub static QEMU_PORTS: HashMap<u16, u8> = HashMap::with_max_entries(1024, 0);

#[map]
pub static QEMU_DEV_HITS: LruHashMap<DevHitKey, u64> = LruHashMap::with_max_entries(4096, 0);

#[map]
pub static QEMU_NET_HITS: LruHashMap<NetHitKey, u64> = LruHashMap::with_max_entries(4096, 0);

// ---- VM edge -------------------------------------------------------------------

#[inline(always)]
fn pol(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> u32 {
    let k = PolicyKey { subject_identity: subject, peer_identity: peer, direction: dir, proto, port: port.to_be_bytes() };
    match unsafe { VM_POLICY.get(&k) } {
        Some(v) => *v,
        None => 0,
    }
}

#[inline(always)]
fn l7_or_none(v: u32) -> u32 {
    if v == 0 { VM_POLICY_L7 } else { v & VM_POLICY_L7 }
}

/// 0 = no entry, VM_POLICY_DENY (any deny match wins), or VM_POLICY_ALLOW
/// plus the AUTH bits of any hit and L7 when every hit carries it.
#[inline(never)]
fn vm_policy_verdict(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> u32 {
    let a = pol(subject, peer, dir, proto, port);
    let b = pol(subject, peer, dir, proto, 0);
    let c = pol(subject, 0, dir, proto, port);
    let d = pol(subject, 0, dir, proto, 0);
    let e = pol(subject, peer, dir, 0, 0);
    let f = pol(subject, 0, dir, 0, 0);
    let any = a | b | c | d | e | f;
    if any & VM_POLICY_DENY != 0 {
        VM_POLICY_DENY
    } else if any != 0 {
        let l7 = l7_or_none(a) & l7_or_none(b) & l7_or_none(c) & l7_or_none(d) & l7_or_none(e) & l7_or_none(f);
        VM_POLICY_ALLOW | (any & (VM_POLICY_AUTH | VM_POLICY_AUTH_FAIL)) | l7
    } else {
        0
    }
}

#[inline(never)]
fn peer_identity(addr: &[u8; ADDR_LEN]) -> u32 {
    if let Some(id) = unsafe { VM_IPS.get(addr) } {
        return *id;
    }
    match VM_CIDR_IDS.get(&Key::new(128, *addr)) {
        Some(id) => *id,
        None => IDENTITY_WORLD,
    }
}

struct FlowMeta {
    ifindex: u32,
    subject: u32,
    peer: u32,
    len: u32,
    from_vm: u8,
    verdict: u8,
    reason: u8,
    icmp: u8,
    auth: u8,
}

/// Emit one verdict event; drops and audits at most once per flow per second.
#[inline(never)]
fn flow_event(t: &Tuple, m: &FlowMeta) {
    let now = now_ns();
    if m.verdict != VMF_FORWARDED {
        let k = ct_key(t, false);
        if let Some(last) = VM_FLOW_SEEN.get_ptr_mut(&k) {
            let last = unsafe { &mut *last };
            if now.saturating_sub(*last) < NSEC {
                return;
            }
            *last = now;
        } else {
            let _ = VM_FLOW_SEEN.insert(&k, &now, 0);
        }
    }
    let Some(mut e) = VM_FLOW_EVENTS.reserve::<VmFlowEvent>(0) else {
        return;
    };
    let ev = e.as_mut_ptr();
    unsafe {
        (*ev).ts_ns = now;
        (*ev).ifindex = m.ifindex;
        (*ev).subject = m.subject;
        (*ev).peer = m.peer;
        (*ev).len = m.len;
        (*ev).src = t.src;
        (*ev).dst = t.dst;
        (*ev).sport = t.sport;
        (*ev).dport = t.dport;
        (*ev).proto = t.proto;
        (*ev).from_vm = m.from_vm;
        (*ev).verdict = m.verdict;
        (*ev).reason = m.reason;
        (*ev).tcp_flags = t.tcp_flags;
        (*ev).icmp = m.icmp;
        (*ev).auth = m.auth;
        (*ev)._pad = [0; 5];
    }
    e.submit(0);
}

/// DHCP and IPv6 neighbour discovery / link-local control always pass.
#[inline(always)]
fn is_control(t: &Tuple) -> bool {
    (t.proto == IPPROTO_UDP && (t.dport == 67 || t.dport == 68 || t.dport == 546 || t.dport == 547))
        || (t.v6
            && t.proto == IPPROTO_ICMPV6
            && ((t.src[0] == 0xfe && t.src[1] & 0xc0 == 0x80)
                || (t.dst[0] == 0xfe && t.dst[1] & 0xc0 == 0x80)
                || t.dst[0] == 0xff))
}

#[inline(always)]
fn ct_key(t: &Tuple, reverse: bool) -> FlowKey {
    if reverse {
        FlowKey { proto: t.proto, local_port: t.dport, remote_port: t.sport, local: t.dst, remote: t.src, ..FlowKey::default() }
    } else {
        FlowKey { proto: t.proto, local_port: t.sport, remote_port: t.dport, local: t.src, remote: t.dst, ..FlowKey::default() }
    }
}

/// Token-bucket police. Out of line: the caller already holds a Tuple.
#[inline(never)]
fn police(key: u32, bps: u64, pps: u64, len: u64, now: u64) -> bool {
    let Some(b) = VM_BUCKETS.get_ptr_mut(&key) else {
        let v = VmBucket {
            bytes: (bps / BURST_DIV).max(MIN_BURST_BYTES).saturating_sub(len),
            pkts: (pps * 1000 / BURST_DIV).max(10_000).saturating_sub(1000),
            last_ns: now,
        };
        let _ = VM_BUCKETS.insert(&key, &v, 0);
        return true;
    };
    let b = unsafe { &mut *b };
    let elapsed = now.saturating_sub(b.last_ns).min(NSEC);
    b.last_ns = now;
    let mut ok = true;
    if bps > 0 {
        let cap = (bps / BURST_DIV).max(MIN_BURST_BYTES);
        let tokens = (b.bytes + elapsed * (bps / 1000) / 1_000_000).min(cap);
        if tokens < len {
            ok = false;
            b.bytes = tokens;
        } else {
            b.bytes = tokens - len;
        }
    }
    if pps > 0 {
        let cap = (pps * 1000 / BURST_DIV).max(10_000);
        let tokens = (b.pkts + elapsed * pps / 1_000_000).min(cap);
        if tokens < 1000 {
            ok = false;
            b.pkts = tokens;
        } else if ok {
            b.pkts = tokens - 1000;
        } else {
            b.pkts = tokens;
        }
    }
    ok
}

#[inline(never)]
fn count(ifindex: u32, from_vm: bool, len: u64, what: u32) {
    let s = match VM_EDGE_STATS.get_ptr_mut(&ifindex) {
        Some(p) => p,
        None => {
            let _ = VM_EDGE_STATS.insert(&ifindex, &VmEdgeStats::default(), 0);
            match VM_EDGE_STATS.get_ptr_mut(&ifindex) {
                Some(p) => p,
                None => return,
            }
        }
    };
    let s = unsafe { &mut *s };
    match what {
        0 if from_vm => {
            s.out_pkts += 1;
            s.out_bytes += len;
        }
        0 => {
            s.in_pkts += 1;
            s.in_bytes += len;
        }
        1 => s.denied += 1,
        2 => s.observed += 1,
        _ => s.rate_dropped += 1,
    }
}

/// L4 payload length from the IP header (frames may carry padding).
#[inline(always)]
fn payload_len(ctx: &TcContext, t: &Tuple) -> u32 {
    if t.payload_off == 0 {
        return 0;
    }
    let end = if t.v6 {
        match ctx.load::<[u8; 2]>(t.l3_off + 4) {
            Ok(b) => t.l3_off + 40 + u16::from_be_bytes(b) as usize,
            Err(_) => return 0,
        }
    } else {
        match ctx.load::<[u8; 2]>(t.l3_off + 2) {
            Ok(b) => t.l3_off + u16::from_be_bytes(b) as usize,
            Err(_) => return 0,
        }
    };
    let end = end.min(ctx.len() as usize);
    if end > t.payload_off { (end - t.payload_off) as u32 } else { 0 }
}

/// `len_skip`: payload length (low 32), skip (bits 32..62), held (bit 63);
/// BPF calls take at most five arguments.
#[inline(never)]
fn l7_emit(ctx: &TcContext, t: &Tuple, m: &FlowMeta, seq_ack: u64, len_skip: u64) {
    let len = len_skip as u32;
    let skip = ((len_skip >> 32) & 0x7fff_ffff) as u32;
    let held = len_skip >> 63 != 0;
    let Some(mut e) = VM_L7_EVENTS.reserve::<VmL7Event>(0) else {
        return;
    };
    let ev = e.as_mut_ptr();
    let n = (len as usize).min(VM_L7_CAP);
    if n == 0 {
        e.discard(0);
        return;
    }
    let n = ((n - 1) & (VM_L7_CAP - 1)) + 1;
    unsafe {
        if bpf_skb_load_bytes(ctx.skb.skb.cast(), t.payload_off as u32, (*ev).data.as_mut_ptr().cast(), n as u32) != 0 {
            e.discard(0);
            return;
        }
        let mac = ctx.load::<[u8; 12]>(0).unwrap_or([0; 12]);
        (*ev).ts_ns = now_ns();
        (*ev).ifindex = m.ifindex;
        (*ev).subject = m.subject;
        (*ev).peer = m.peer;
        (*ev).seq = (seq_ack >> 32) as u32;
        (*ev).ack = seq_ack as u32;
        (*ev).len = len;
        (*ev).cap = n as u32;
        (*ev).skip = skip;
        (*ev).src = t.src;
        (*ev).dst = t.dst;
        (*ev).sport = t.sport;
        (*ev).dport = t.dport;
        (*ev).proto = t.proto;
        (*ev).from_vm = m.from_vm;
        (*ev).held = held as u8;
        (*ev).v6 = t.v6 as u8;
        (*ev).mac = mac;
        (*ev)._pad = 0;
    }
    e.submit(0);
}

#[inline(always)]
fn l7_pack(len: u32, skip: u32, held: bool) -> u64 {
    len as u64 | ((skip & 0x7fff_ffff) as u64) << 32 | (held as u64) << 63
}

/// L7 gate for one packet of an allowed flow; true = drop.
#[inline(never)]
fn vm_l7(ctx: &TcContext, t: &Tuple, m: &FlowMeta, enforce: bool) -> bool {
    let len = payload_len(ctx, t);
    if len == 0 {
        return false;
    }
    if t.proto == IPPROTO_UDP {
        l7_emit(ctx, t, m, 0, l7_pack(len, 0, enforce));
        return enforce;
    }
    if t.proto != IPPROTO_TCP {
        return false;
    }
    let seq = match ctx.load::<[u8; 4]>(t.l4_off + 4) {
        Ok(b) => u32::from_be_bytes(b),
        Err(_) => return false,
    };
    let ack = match ctx.load::<[u8; 4]>(t.l4_off + 8) {
        Ok(b) => u32::from_be_bytes(b),
        Err(_) => 0,
    };
    let seq_ack = ((seq as u64) << 32) | ack as u64;
    let k = ct_key(t, false);
    let Some(f) = VM_L7_FLOW.get_ptr_mut(&k) else {
        l7_emit(ctx, t, m, seq_ack, l7_pack(len, 0, enforce));
        if enforce {
            let v = VmL7Flow { allow_seq: seq, pass_until: seq, pending_seq: seq, state: L7S_PENDING, _pad: [0; 3] };
            let _ = VM_L7_FLOW.insert(&k, &v, 0);
        }
        return enforce;
    };
    let f = unsafe { &mut *f };
    if f.state == L7S_OPEN {
        return false;
    }
    if f.state == L7S_DENIED {
        return enforce;
    }
    let span = f.pass_until.wrapping_sub(f.allow_seq);
    let off = seq.wrapping_sub(f.allow_seq);
    let mut skip = 0;
    if off < span {
        if len <= span - off {
            return false;
        }
        skip = span - off;
    } else if f.state == L7S_PENDING && seq != f.pending_seq {
        return enforce;
    }
    l7_emit(ctx, t, m, seq_ack, l7_pack(len, skip, enforce));
    if enforce {
        f.state = L7S_PENDING;
        f.pending_seq = seq;
    }
    enforce
}

#[inline(always)]
fn vm_edge(ctx: &TcContext, ingress: bool) -> i32 {
    let ifindex = unsafe { (*ctx.skb.skb).ifindex };
    let Some(cfg) = (unsafe { VM_EDGE.get(&ifindex) }) else {
        return TC_ACT_UNSPEC;
    };
    let (identity, flags, out_bps, in_bps, pps) = (cfg.identity, cfg.flags, cfg.out_bps, cfg.in_bps, cfg.pps);
    let from_vm = (flags & VME_GUEST_SIDE != 0) == ingress;
    let len = ctx.len() as u64;
    let now = now_ns();

    let bps = if from_vm { out_bps } else { in_bps };
    if (bps | pps as u64) != 0 && !police((ifindex << 1) | from_vm as u32, bps, pps as u64, len, now) {
        count(ifindex, from_vm, len, 3);
        return TC_ACT_SHOT;
    }
    count(ifindex, from_vm, len, 0);

    let isolated = flags & if from_vm { VME_ISOLATE_OUT } else { VME_ISOLATE_IN } != 0;
    let has_deny = flags & if from_vm { VME_DENY_OUT } else { VME_DENY_IN } != 0;
    let flow_log = flags & VME_FLOW_LOG != 0;
    let fqdn = flags & VME_FQDN != 0 && !from_vm;
    let ext = flags & (VME_L7_AUTH | VME_SRC_GUARD) != 0;
    if !isolated && !has_deny && !flow_log && !fqdn && !ext {
        return TC_ACT_UNSPEC;
    }
    let mut t = Tuple::zero();
    if parse_tc(ctx, &mut t) == 0 || is_control(&t) {
        return TC_ACT_UNSPEC;
    }
    if fqdn
        && t.proto == IPPROTO_UDP
        && t.sport == 53
        && !unsafe { IFACE_CFG.get(&ifindex) }.is_some_and(|c| c.flags & IF_DNS != 0)
    {
        emit_dns(ctx, &t, ifindex as u64);
    }
    if !isolated && !has_deny && !flow_log && !ext {
        return TC_ACT_UNSPEC;
    }
    if from_vm && flags & VME_SRC_GUARD != 0 {
        if let Some(owner) = unsafe { VM_IPS.get(&t.src) } {
            if *owner != identity {
                let m = FlowMeta {
                    ifindex,
                    subject: identity,
                    peer: *owner,
                    len: len as u32,
                    from_vm: 1,
                    verdict: VMF_AUDIT,
                    reason: VMF_REASON_SPOOFED,
                    icmp: 0,
                    auth: 0,
                };
                if enforce_active(now) {
                    count(ifindex, true, len, 1);
                    flow_event(&t, &FlowMeta { verdict: VMF_DROPPED, ..m });
                    return TC_ACT_SHOT;
                }
                count(ifindex, true, len, 2);
                flow_event(&t, &m);
            }
        }
    }
    let mut icmp: u8 = 0;
    if t.proto == IPPROTO_ICMP || t.proto == IPPROTO_ICMPV6 {
        if let Ok(ty) = ctx.load::<u8>(t.l4_off) {
            icmp = ty.wrapping_add(1);
            // Echo request/reply carry the identifier in the ports, so only
            // a reply matches the reverse of a request (an inbound request
            // is never mistaken for a reply to our own ping).
            let v6 = t.proto == IPPROTO_ICMPV6;
            let request = if v6 { ty == 128 } else { ty == 8 };
            let reply = if v6 { ty == 129 } else { ty == 0 };
            if request || reply {
                let id = ctx.load::<u16>(t.l4_off + 4).unwrap_or(0);
                if request {
                    t.sport = id;
                    t.dport = ICMP_ECHO_MARK;
                } else {
                    t.sport = ICMP_ECHO_MARK;
                    t.dport = id;
                }
            }
        }
    }
    if VM_CT.get_ptr(&ct_key(&t, true)).is_some() {
        return TC_ACT_UNSPEC;
    }
    let port = if icmp != 0 { icmp as u16 } else { t.dport };
    let (peer_addr, dir) = if from_vm { (&t.dst, POLICY_EGRESS) } else { (&t.src, POLICY_INGRESS) };
    let peer = peer_identity(peer_addr);
    let verdict = vm_policy_verdict(identity, peer, dir, t.proto, port);
    let k = ct_key(&t, false);
    let is_new = VM_CT.get_ptr(&k).is_none();
    let mut m = FlowMeta {
        ifindex,
        subject: identity,
        peer,
        len: len as u32,
        from_vm: from_vm as u8,
        verdict: VMF_FORWARDED,
        reason: VMF_REASON_NONE,
        icmp,
        auth: 0,
    };
    if verdict == VM_POLICY_DENY || (verdict == 0 && isolated) {
        m.reason = if verdict == VM_POLICY_DENY { VMF_REASON_POLICY_DENY } else { VMF_REASON_DEFAULT_DENY };
        if enforce_active(now) {
            count(ifindex, from_vm, len, 1);
            if flow_log {
                m.verdict = VMF_DROPPED;
                flow_event(&t, &m);
            }
            return TC_ACT_SHOT;
        }
        count(ifindex, from_vm, len, 2);
        if flow_log {
            m.verdict = VMF_AUDIT;
            flow_event(&t, &m);
        }
    } else {
        if is_new && verdict & (VM_POLICY_AUTH | VM_POLICY_AUTH_FAIL) != 0 {
            let fail = verdict & VM_POLICY_AUTH_FAIL != 0;
            let ak = VmAuthKey { subject: identity, peer, mode: 1, _pad: [0; 3] };
            let ok = !fail && unsafe { VM_AUTH.get(&ak) }.is_some_and(|exp| *exp > now);
            if !ok {
                m.reason = VMF_REASON_AUTH_REQUIRED;
                m.auth = if fail { 2 } else { 1 };
                if enforce_active(now) {
                    count(ifindex, from_vm, len, 1);
                    m.verdict = VMF_DROPPED;
                    flow_event(&t, &m);
                    return TC_ACT_SHOT;
                }
                count(ifindex, from_vm, len, 2);
                m.verdict = VMF_AUDIT;
                flow_event(&t, &m);
            }
        }
        if is_new && flow_log && m.reason == VMF_REASON_NONE {
            flow_event(&t, &m);
        }
    }
    match VM_CT.get_ptr_mut(&k) {
        Some(p) => unsafe { *p = now },
        None => {
            let _ = VM_CT.insert(&k, &now, 0);
        }
    }
    if verdict & VM_POLICY_L7 != 0 && verdict & VM_POLICY_DENY == 0 && vm_l7(ctx, &t, &m, enforce_active(now)) {
        return TC_ACT_SHOT;
    }
    TC_ACT_UNSPEC
}

#[classifier]
pub fn mn_vm_edge_in(ctx: TcContext) -> i32 {
    vm_edge(&ctx, true)
}

#[classifier]
pub fn mn_vm_edge_out(ctx: TcContext) -> i32 {
    vm_edge(&ctx, false)
}

// ---- QEMU sandbox --------------------------------------------------------------

#[inline(always)]
fn sandbox_enforcing() -> bool {
    match QEMU_SANDBOX.get(0) {
        Some(c) => c.flags & SANDBOX_ENFORCE != 0 && enforce_active(now_ns()),
        None => false,
    }
}

#[cgroup_device]
pub fn mn_qemu_device(ctx: DeviceContext) -> i32 {
    let d = unsafe { &*ctx.device };
    let dev_type = d.access_type & 0xffff;
    let access = d.access_type >> 16;
    let (major, minor) = (d.major, d.minor);
    // Unconfigured = fail open: bpfd always fills the rules before attaching.
    let Some(cfg) = QEMU_SANDBOX.get(0) else {
        return 1;
    };
    let n = cfg.n;
    let mut i = 0;
    while i < QEMU_DEV_RULES {
        if i as u32 >= n {
            break;
        }
        let r = &cfg.rules[i];
        if r.dev_type == dev_type
            && r.major == major
            && (r.minor == DEV_MINOR_ANY || r.minor == minor)
            && access & r.access == access
        {
            return 1;
        }
        i += 1;
    }
    let key = DevHitKey {
        cgroup: unsafe { bpf_get_current_cgroup_id() },
        major,
        minor,
        dev_type: dev_type as u16,
        access: access as u16,
        _pad: 0,
    };
    match QEMU_DEV_HITS.get_ptr_mut(&key) {
        Some(p) => unsafe { *p += 1 },
        None => {
            let _ = QEMU_DEV_HITS.insert(&key, &1, 0);
        }
    }
    if sandbox_enforcing() { 0 } else { 1 }
}

#[inline(always)]
fn is_loopback(a: &[u8; ADDR_LEN], v6: bool) -> bool {
    if v6 {
        let mut i = 0;
        while i < 15 {
            if a[i] != 0 {
                return false;
            }
            i += 1;
        }
        a[15] == 1
    } else {
        a[12] == 127
    }
}

#[cgroup_skb]
pub fn mn_qemu_egress(ctx: SkBuffContext) -> i32 {
    let mut t = Tuple::zero();
    if parse_skb(&ctx, &mut t) == 0 || is_loopback(&t.dst, t.v6) {
        return 1;
    }
    if (t.proto == IPPROTO_TCP || t.proto == IPPROTO_UDP) && unsafe { QEMU_PORTS.get(&t.dport) }.is_some() {
        return 1;
    }
    let key = NetHitKey {
        cgroup: unsafe { bpf_skb_cgroup_id(ctx.skb.skb) },
        addr: t.dst,
        port: t.dport.to_be_bytes(),
        proto: t.proto,
        _pad: [0; 5],
    };
    match QEMU_NET_HITS.get_ptr_mut(&key) {
        Some(p) => unsafe { *p += 1 },
        None => {
            let _ = QEMU_NET_HITS.insert(&key, &1, 0);
        }
    }
    if sandbox_enforcing() { 0 } else { 1 }
}

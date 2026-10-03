//! VM edge and QEMU sandbox.
//!
//! `mn_vm_edge_in/out` run on VM taps after the host datapath (`mn_tc_*`
//! returns TCX_NEXT): group-identity policy with conntrack (same key shape as
//! CNI NetworkPolicy), Mbps / PPS token buckets and per-tap counters.
//!
//! `mn_qemu_device` (cgroup device) and `mn_qemu_egress` (cgroup_skb egress)
//! sandbox the QEMU process in its machine scope: a device-node allowlist
//! and loopback / migration / NBD-only IP egress. Both observe unless the
//! sandbox is set to enforce *and* the enforcement lease is live; libvirt's
//! own device program stays attached and the kernel ANDs the verdicts.

use aya_ebpf::{
    helpers::generated::{bpf_get_current_cgroup_id, bpf_skb_cgroup_id},
    macros::{cgroup_device, cgroup_skb, classifier, map},
    maps::{Array, HashMap, LruHashMap, PerCpuHashMap},
    programs::{DeviceContext, SkBuffContext, TcContext},
};
use machina_bpf_common::*;

use crate::{
    net::{TC_ACT_SHOT, TC_ACT_UNSPEC, enforce_active, now_ns},
    parse::*,
};

const IPPROTO_ICMPV6: u8 = 58;
const NSEC: u64 = 1_000_000_000;
/// Bucket depth: 100 ms of the configured rate (at least one jumbo frame).
const BURST_DIV: u64 = 10;
const MIN_BURST_BYTES: u64 = 64 * 1024;

#[map]
pub static VM_EDGE: HashMap<u32, VmEdgeCfg> = HashMap::with_max_entries(4096, 0);

/// VM address (16-byte, IPv4-mapped) → group identity, for peers.
#[map]
pub static VM_IPS: HashMap<[u8; ADDR_LEN], u32> = HashMap::with_max_entries(65536, 0);

#[map]
pub static VM_POLICY: HashMap<PolicyKey, u32> = HashMap::with_max_entries(65536, 0);

/// Conntrack: FlowKey (ifindex 0, local = originator) → last seen.
#[map]
pub static VM_CT: LruHashMap<FlowKey, u64> = LruHashMap::with_max_entries(131072, 0);

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
fn pol(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> bool {
    let k = PolicyKey { subject_identity: subject, peer_identity: peer, direction: dir, proto, port: port.to_be_bytes() };
    unsafe { VM_POLICY.get(&k) }.is_some()
}

#[inline(never)]
fn vm_policy_allows(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> bool {
    pol(subject, peer, dir, proto, port)
        || pol(subject, peer, dir, proto, 0)
        || pol(subject, 0, dir, proto, port)
        || pol(subject, 0, dir, proto, 0)
        || pol(subject, peer, dir, 0, 0)
        || pol(subject, 0, dir, 0, 0)
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
    if !isolated {
        return TC_ACT_UNSPEC;
    }
    let mut t = Tuple::zero();
    if parse_tc(ctx, &mut t) == 0 || is_control(&t) {
        return TC_ACT_UNSPEC;
    }
    if VM_CT.get_ptr(&ct_key(&t, true)).is_some() {
        return TC_ACT_UNSPEC;
    }
    let (peer_addr, dir, port) =
        if from_vm { (&t.dst, POLICY_EGRESS, t.dport) } else { (&t.src, POLICY_INGRESS, t.dport) };
    let peer = unsafe { VM_IPS.get(peer_addr) }.copied().unwrap_or(IDENTITY_WORLD);
    if !vm_policy_allows(identity, peer, dir, t.proto, port) {
        if enforce_active(now) {
            count(ifindex, from_vm, len, 1);
            return TC_ACT_SHOT;
        }
        count(ifindex, from_vm, len, 2);
    }
    let k = ct_key(&t, false);
    match VM_CT.get_ptr_mut(&k) {
        Some(p) => unsafe { *p = now },
        None => {
            let _ = VM_CT.insert(&k, &now, 0);
        }
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

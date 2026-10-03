//! Host datapath: enforcement (deny / allowlist), flows, DNS, capture, QoS.
//!
//! Attached by machina-bpfd to VM taps (TCX ingress+egress), optionally XDP on
//! uplinks, and cgroup hooks for container scopes.

use core::ffi::c_void;

use aya_ebpf::{
    bindings::xdp_action,
    helpers::generated::{
        bpf_get_current_ancestor_cgroup_id, bpf_get_current_cgroup_id, bpf_get_prandom_u32,
        bpf_ktime_get_ns, bpf_skb_ancestor_cgroup_id, bpf_skb_cgroup_id, bpf_skb_load_bytes,
        bpf_skb_set_tstamp,
    },
    macros::{cgroup_skb, cgroup_sock_addr, classifier, xdp},
    maps::lpm_trie::Key,
    programs::{SkBuffContext, SockAddrContext, TcContext, XdpContext},
};
use machina_bpf_common::*;

use crate::{maps::*, parse::*};

pub const TC_ACT_UNSPEC: i32 = -1;
pub const TC_ACT_SHOT: i32 = 2;

const BPF_SKB_TSTAMP_DELIVERY_MONO: u32 = 1;
const QOS_HORIZON_NS: u64 = 2_000_000_000;
const NSEC: u64 = 1_000_000_000;

#[inline(always)]
pub fn now_ns() -> u64 {
    unsafe { bpf_ktime_get_ns() }
}

#[inline(always)]
pub fn enforce_active(now: u64) -> bool {
    match CONFIG.get(0) {
        Some(c) => c.mode == MODE_ENFORCE && now < c.lease_deadline_ns,
        None => false,
    }
}

/// Returns policy id + 1 (0 = no match). Out of line to keep the caller's
/// stack frame small.
#[inline(never)]
fn deny_lookup(scope: u32, addr: &[u8; ADDR_LEN]) -> u32 {
    let k = Key::new(
        DENY_KEY_FIXED_BITS + 128,
        DenyKey {
            scope: scope.to_be_bytes(),
            addr: *addr,
        },
    );
    match DENY_LPM.get(&k) {
        Some(v) => v.policy_id.wrapping_add(1),
        None => 0,
    }
}

#[inline(always)]
pub fn check_deny(scope: u32, addr: &[u8; ADDR_LEN]) -> Option<u32> {
    let p = deny_lookup(0, addr);
    if p != 0 {
        return Some(p - 1);
    }
    if scope != 0 {
        let p = deny_lookup(scope, addr);
        if p != 0 {
            return Some(p - 1);
        }
    }
    None
}

#[inline(never)]
fn port_lookup(scope: u32, proto: u32, port: u32) -> u32 {
    let k = PortKey {
        scope,
        proto: proto as u8,
        _pad: 0,
        port: (port as u16).to_be_bytes(),
    };
    match unsafe { DENY_PORTS.get(&k) } {
        Some(v) => v.policy_id.wrapping_add(1),
        None => 0,
    }
}

/// `deny_port`: remote port deny (exact proto, then any proto).
#[inline(always)]
pub fn check_deny_port(scope: u32, proto: u8, port: u16) -> Option<u32> {
    if port == 0 {
        return None;
    }
    let mut p = port_lookup(0, proto as u32, port as u32);
    if p == 0 {
        p = port_lookup(0, 0, port as u32);
    }
    if p == 0 && scope != 0 {
        p = port_lookup(scope, proto as u32, port as u32);
        if p == 0 {
            p = port_lookup(scope, 0, port as u32);
        }
    }
    if p != 0 { Some(p - 1) } else { None }
}

#[inline(never)]
fn allow_lookup(scope: u32, proto: u32, port: u32, addr: &[u8; ADDR_LEN]) -> u32 {
    let k = Key::new(
        ALLOW_KEY_FIXED_BITS + 128,
        AllowKey {
            scope: scope.to_be_bytes(),
            proto: proto as u8,
            _pad: 0,
            port: (port as u16).to_be_bytes(),
            addr: *addr,
        },
    );
    ALLOW_LPM.get(&k).is_some() as u32
}

#[inline(always)]
fn allow_scope(scope: u32, proto: u8, port: u16, addr: &[u8; ADDR_LEN]) -> bool {
    allow_lookup(scope, proto as u32, port as u32, addr) != 0
        || allow_lookup(scope, proto as u32, 0, addr) != 0
        || allow_lookup(scope, 0, 0, addr) != 0
}

#[inline(always)]
pub fn check_allow(scope: u32, proto: u8, port: u16, addr: &[u8; ADDR_LEN]) -> bool {
    if scope != 0 && allow_scope(scope, proto, port, addr) {
        return true;
    }
    allow_scope(0, proto, port, addr)
}

#[inline(always)]
pub fn emit_net(
    now: u64,
    kind: u32,
    verdict: u32,
    policy_id: u32,
    pkt_len: u32,
    key: &FlowKey,
    tx_bytes: u64,
    rx_bytes: u64,
) {
    if let Some(mut e) = NET_EVENTS.reserve::<NetEvent>(0) {
        let ev = e.as_mut_ptr();
        unsafe {
            (*ev).ts_ns = now;
            (*ev).kind = kind;
            (*ev).verdict = verdict;
            (*ev).policy_id = policy_id;
            (*ev).pkt_len = pkt_len;
            core::ptr::copy_nonoverlapping(key, &raw mut (*ev).key, 1);
            (*ev).tx_bytes = tx_bytes;
            (*ev).rx_bytes = rx_bytes;
        }
        e.submit(0);
    }
}

#[inline(always)]
fn skb_ptr(ctx: &TcContext) -> *mut c_void {
    ctx.skb.skb.cast()
}

/// EDT pacing on egress, token bucket on ingress. Returns false to drop.
/// `if_dir` packs ifindex (low 32 bits) and ingress (bit 32). Returns 1 to pass.
///
/// Out-of-line helpers take scalars only: BPF calls carry at most five
/// register arguments, and LLVM argument promotion can split a struct
/// reference into several scalars, spilling to an r11 stack slot the
/// verifier rejects.
#[inline(never)]
fn qos(ctx: &TcContext, rate: u64, if_dir: u64, now: u64, len: u64) -> u32 {
    qos_inner(ctx, if_dir as u32, rate, (if_dir >> 32) & 1 != 0, now, len) as u32
}

#[inline(always)]
fn qos_inner(ctx: &TcContext, ifindex: u32, rate: u64, ingress: bool, now: u64, len: u64) -> bool {
    if rate == 0 {
        return true;
    }
    let st = match QOS_STATE.get_ptr_mut(&ifindex) {
        Some(p) => p,
        None => {
            let burst = if rate / 10 > 65536 { rate / 10 } else { 65536 };
            let fresh = QosState {
                tokens: burst,
                last_ns: now,
                next_tstamp: now,
            };
            if QOS_STATE.insert(&ifindex, &fresh, 0).is_err() {
                return true;
            }
            match QOS_STATE.get_ptr_mut(&ifindex) {
                Some(p) => p,
                None => return true,
            }
        }
    };
    unsafe {
        if ingress {
            let burst = if rate / 10 > 65536 { rate / 10 } else { 65536 };
            let mut elapsed = now.saturating_sub((*st).last_ns);
            if elapsed > NSEC {
                elapsed = NSEC;
            }
            let mut tokens = (*st).tokens + elapsed * rate / NSEC;
            if tokens > burst {
                tokens = burst;
            }
            (*st).last_ns = now;
            if tokens < len {
                (*st).tokens = tokens;
                return false;
            }
            (*st).tokens = tokens - len;
            true
        } else {
            let delay = len * NSEC / rate;
            let next = if (*st).next_tstamp > now {
                (*st).next_tstamp
            } else {
                now
            };
            if next - now > QOS_HORIZON_NS {
                return false;
            }
            bpf_skb_set_tstamp(
                ctx.skb.skb,
                next,
                BPF_SKB_TSTAMP_DELIVERY_MONO,
            );
            (*st).next_tstamp = next + delay;
            true
        }
    }
}

/// `if_dir` packs ifindex (low 32 bits) and from_workload (bit 32).
#[inline(never)]
fn emit_dns(ctx: &TcContext, t: &Tuple, if_dir: u64) {
    let ifindex = if_dir as u32;
    let from_workload = (if_dir >> 32) & 1 != 0;
    let now = now_ns();
    let len = ctx.len() as usize;
    if t.payload_off == 0 || len <= t.payload_off {
        return;
    }
    let mut n = len - t.payload_off;
    if n > DNS_PAYLOAD_LEN {
        n = DNS_PAYLOAD_LEN;
    }
    let n = ((n - 1) & (DNS_PAYLOAD_LEN - 1)) + 1;
    let Some(mut e) = DNS_EVENTS.reserve::<DnsEvent>(0) else {
        return;
    };
    let ev = e.as_mut_ptr();
    unsafe {
        (*ev).ts_ns = now;
        (*ev).ifindex = ifindex;
        (*ev).dir = if from_workload {
            DIR_FROM_WORKLOAD
        } else {
            DIR_TO_WORKLOAD
        };
        (*ev)._pad = 0;
        if from_workload {
            (*ev).local = t.src;
            (*ev).remote = t.dst;
        } else {
            (*ev).local = t.dst;
            (*ev).remote = t.src;
        }
        let ret = bpf_skb_load_bytes(
            skb_ptr(ctx),
            t.payload_off as u32,
            (*ev).payload.as_mut_ptr().cast(),
            n as u32,
        );
        if ret != 0 {
            e.discard(0);
            return;
        }
        (*ev).payload_len = n as u32;
    }
    e.submit(0);
}

/// `if_dir` as for [`emit_dns`]; `sample_snap` packs sample (low 32) and snaplen (high 32).
#[inline(never)]
fn emit_capture(ctx: &TcContext, if_dir: u64, sample_snap: u64) {
    let ifindex = if_dir as u32;
    let from_workload = (if_dir >> 32) & 1 != 0;
    let sample = sample_snap as u32;
    let now = now_ns();
    if sample > 1 && unsafe { bpf_get_prandom_u32() } % sample != 0 {
        return;
    }
    let len = ctx.len() as usize;
    if len == 0 {
        return;
    }
    let mut n = len;
    let snap = (sample_snap >> 32) as usize;
    if snap != 0 && n > snap {
        n = snap;
    }
    if n > CAPTURE_SNAPLEN {
        n = CAPTURE_SNAPLEN;
    }
    let n = ((n - 1) & (CAPTURE_SNAPLEN - 1)) + 1;
    let Some(mut e) = CAPTURE_EVENTS.reserve::<CaptureEvent>(0) else {
        return;
    };
    let ev = e.as_mut_ptr();
    unsafe {
        (*ev).ts_ns = now;
        (*ev).ifindex = ifindex;
        (*ev).dir = if from_workload {
            DIR_FROM_WORKLOAD
        } else {
            DIR_TO_WORKLOAD
        };
        (*ev).pkt_len = len as u32;
        let ret = bpf_skb_load_bytes(skb_ptr(ctx), 0, (*ev).data.as_mut_ptr().cast(), n as u32);
        if ret != 0 {
            e.discard(0);
            return;
        }
        (*ev).cap_len = n as u32;
    }
    e.submit(0);
}

/// Update (or create) the flow entry. `meta` packs: len (bits 0..32),
/// tcp flags (32..40), from_workload (40), verdict (41..43), emit-open (43).
#[inline(never)]
fn flow_update(key: &FlowKey, meta: u64, now: u64) -> u32 {
    let len = meta as u32 as u64;
    let tcp_flags = ((meta >> 32) & 0xff) as u32;
    let from_workload = (meta >> 40) & 1 != 0;
    let verdict = ((meta >> 41) & 0x3) as u8;
    let emit_open = (meta >> 43) & 1 != 0;
    if let Some(f) = FLOWS.get_ptr_mut(key) {
        unsafe {
            let old_flags = (*f).tcp_flags;
            (*f).last_ns = now;
            if from_workload {
                (*f).tx_pkts += 1;
                (*f).tx_bytes += len;
            } else {
                (*f).rx_pkts += 1;
                (*f).rx_bytes += len;
            }
            if verdict != 0 {
                (*f).verdict = verdict;
            }
            (*f).tcp_flags = old_flags | tcp_flags;
            let closing = (TCP_FIN | TCP_RST) as u32;
            if key.proto == IPPROTO_TCP && old_flags & closing == 0 && tcp_flags & closing != 0 {
                emit_net(
                    now,
                    NET_EV_FLOW_CLOSE,
                    VERDICT_PASS,
                    0,
                    len as u32,
                    key,
                    (*f).tx_bytes,
                    (*f).rx_bytes,
                );
            }
        }
        return 0;
    }
    let mut v = FlowVal {
        first_ns: now,
        last_ns: now,
        tcp_flags,
        origin: if from_workload { ORIGIN_LOCAL } else { ORIGIN_REMOTE },
        verdict,
        ..FlowVal::default()
    };
    if from_workload {
        v.tx_pkts = 1;
        v.tx_bytes = len;
    } else {
        v.rx_pkts = 1;
        v.rx_bytes = len;
    }
    if FLOWS.insert(key, &v, 0).is_ok() && emit_open && verdict == 0 {
        emit_net(now, NET_EV_FLOW_OPEN, VERDICT_PASS, v.origin as u32, len as u32, key, 0, 0);
    }
    0
}

/// Rates are from the workload's point of view; the hook direction only
/// picks the mechanism (EDT on egress hooks, token bucket on ingress hooks).
#[inline(always)]
fn qos_rate(cfg: &IfaceCfg, from_workload: bool) -> u64 {
    if from_workload {
        cfg.qos_egress_bps
    } else {
        cfg.qos_ingress_bps
    }
}

#[inline(always)]
fn tc_handle(ctx: &TcContext, ingress: bool) -> i32 {
    let ifindex = unsafe { (*ctx.skb.skb).ifindex };
    let cfg = match unsafe { IFACE_CFG.get(&ifindex) } {
        Some(c) => *c,
        None => return TC_ACT_UNSPEC,
    };
    let guest_side = cfg.flags & IF_GUEST_SIDE != 0;
    let from_workload = guest_side == ingress;
    let now = now_ns();
    let len = ctx.len();

    let if_dir = ifindex as u64 | ((ingress as u64) << 32);
    let mut t = Tuple::zero();
    if parse_tc(ctx, &mut t) == 0 {
        if cfg.flags & IF_QOS != 0 && qos(ctx, qos_rate(&cfg, from_workload), if_dir, now, len as u64) == 0 {
            return TC_ACT_SHOT;
        }
        return TC_ACT_UNSPEC;
    }

    let (local, remote, lport, rport) = if from_workload {
        (t.src, t.dst, t.sport, t.dport)
    } else {
        (t.dst, t.src, t.dport, t.sport)
    };
    let key = FlowKey {
        ifindex,
        proto: t.proto,
        _pad: [0; 3],
        local_port: lport,
        remote_port: rport,
        _pad2: 0,
        local,
        remote,
    };
    let track = cfg.flags & (IF_FLOWS | IF_ALLOW) != 0;
    let existing = if track { FLOWS.get_ptr_mut(&key) } else { None };

    let mut kind = 0u32;
    let mut policy = 0u32;
    if cfg.flags & IF_DENY != 0 {
        if let Some(p) = check_deny(cfg.scope, &remote) {
            kind = NET_EV_DENY;
            policy = p;
        } else if from_workload {
            if let Some(p) = check_deny_port(cfg.scope, t.proto, rport) {
                kind = NET_EV_DENY;
                policy = p;
            }
        }
    }
    if kind == 0 && from_workload && cfg.flags & IF_ALLOW != 0 {
        let reply = match existing {
            Some(f) => unsafe { (*f).origin == ORIGIN_REMOTE },
            None => false,
        };
        if !reply && !check_allow(cfg.scope, t.proto, rport, &remote) {
            kind = NET_EV_ALLOW_MISS;
        }
    }
    let mut verdict = VERDICT_PASS;
    if kind != 0 {
        verdict = if enforce_active(now) {
            VERDICT_DROP
        } else {
            VERDICT_OBSERVED
        };
        // Only report the first packet of a flagged flow.
        let already = match existing {
            Some(f) => unsafe { (*f).verdict as u32 == verdict },
            None => false,
        };
        if !already {
            emit_net(now, kind, verdict, policy, len, &key, 0, 0);
        }
    }

    if verdict != VERDICT_DROP
        && cfg.flags & IF_QOS != 0
        && qos(
            ctx,
            qos_rate(&cfg, from_workload),
            if_dir,
            now,
            len as u64,
        ) == 0
    {
        emit_net(now, NET_EV_QOS_DROP, VERDICT_DROP, 0, len, &key, 0, 0);
        return TC_ACT_SHOT;
    }

    if track {
        let meta = (len as u64)
            | ((t.tcp_flags as u64) << 32)
            | ((from_workload as u64) << 40)
            | ((verdict as u64) << 41)
            | (((cfg.flags & IF_FLOWS != 0) as u64) << 43);
        flow_update(&key, meta, now);
    }
    if verdict == VERDICT_DROP {
        return TC_ACT_SHOT;
    }

    if cfg.flags & IF_DNS != 0 && t.proto == IPPROTO_UDP && (t.sport == 53 || t.dport == 53) {
        emit_dns(ctx, &t, ifindex as u64 | ((from_workload as u64) << 32));
    }
    if cfg.flags & IF_CAPTURE != 0 {
        emit_capture(
            ctx,
            ifindex as u64 | ((from_workload as u64) << 32),
            cfg.capture_sample as u64 | ((cfg.capture_snaplen as u64) << 32),
        );
    }
    TC_ACT_UNSPEC
}

#[classifier]
pub fn mn_tc_ingress(ctx: TcContext) -> i32 {
    tc_handle(&ctx, true)
}

#[classifier]
pub fn mn_tc_egress(ctx: TcContext) -> i32 {
    tc_handle(&ctx, false)
}

// ---- XDP fast-path deny (uplinks) -----------------------------------------

#[inline(always)]
fn xdp_ptr<T>(ctx: &XdpContext, off: usize) -> Option<*const T> {
    let start = ctx.data();
    let end = ctx.data_end();
    if start + off + core::mem::size_of::<T>() > end {
        return None;
    }
    Some((start + off) as *const T)
}

#[xdp]
pub fn mn_xdp_deny(ctx: XdpContext) -> u32 {
    let ifindex = ctx.ingress_ifindex() as u32;
    let cfg = match unsafe { IFACE_CFG.get(&ifindex) } {
        Some(c) => *c,
        None => return xdp_action::XDP_PASS,
    };
    if cfg.flags & IF_DENY == 0 {
        return xdp_action::XDP_PASS;
    }
    let Some(et) = xdp_ptr::<[u8; 2]>(&ctx, 12) else {
        return xdp_action::XDP_PASS;
    };
    let et = u16::from_be_bytes(unsafe { *et });
    let from_workload = cfg.flags & IF_GUEST_SIDE != 0;
    let addr = match et {
        ETH_P_IP => {
            let off = if from_workload { ETH_HLEN + 16 } else { ETH_HLEN + 12 };
            match xdp_ptr::<[u8; 4]>(&ctx, off) {
                Some(p) => v4_mapped(unsafe { *p }),
                None => return xdp_action::XDP_PASS,
            }
        }
        ETH_P_IPV6 => {
            let off = if from_workload { ETH_HLEN + 24 } else { ETH_HLEN + 8 };
            match xdp_ptr::<[u8; 16]>(&ctx, off) {
                Some(p) => unsafe { *p },
                None => return xdp_action::XDP_PASS,
            }
        }
        _ => return xdp_action::XDP_PASS,
    };
    let Some(policy) = check_deny(cfg.scope, &addr) else {
        return xdp_action::XDP_PASS;
    };
    let now = now_ns();
    let key = FlowKey {
        ifindex,
        remote: addr,
        ..FlowKey::default()
    };
    let len = (ctx.data_end() - ctx.data()) as u32;
    if enforce_active(now) {
        emit_net(now, NET_EV_DENY, VERDICT_DROP, policy, len, &key, 0, 0);
        xdp_action::XDP_DROP
    } else {
        emit_net(now, NET_EV_DENY, VERDICT_OBSERVED, policy, len, &key, 0, 0);
        xdp_action::XDP_PASS
    }
}

// ---- cgroup scope (containers) -------------------------------------------

#[inline(always)]
fn scope_for_cgroup(id: u64) -> Option<u32> {
    if id == 0 {
        return None;
    }
    unsafe { CGROUP_SCOPE.get(&id) }.copied()
}

#[inline(always)]
fn skb_scope(ctx: &SkBuffContext) -> Option<u32> {
    let skb = ctx.skb.skb;
    if let Some(s) = scope_for_cgroup(unsafe { bpf_skb_cgroup_id(skb) }) {
        return Some(s);
    }
    // Walk root → leaf so the deepest registered ancestor wins.
    let mut found = None;
    for level in 0..8 {
        let id = unsafe { bpf_skb_ancestor_cgroup_id(skb, level) };
        if id == 0 {
            break;
        }
        if let Some(s) = scope_for_cgroup(id) {
            found = Some(s);
        }
    }
    found
}

#[inline(always)]
pub fn current_scope() -> Option<u32> {
    if let Some(s) = scope_for_cgroup(unsafe { bpf_get_current_cgroup_id() }) {
        return Some(s);
    }
    let mut found = None;
    for level in 0..8 {
        let id = unsafe { bpf_get_current_ancestor_cgroup_id(level) };
        if id == 0 {
            break;
        }
        if let Some(s) = scope_for_cgroup(id) {
            found = Some(s);
        }
    }
    found
}

#[inline(always)]
fn scope_flags(scope: u32) -> u32 {
    unsafe { SCOPE_FLAGS.get(&scope) }.copied().unwrap_or(0)
}

#[inline(always)]
fn cg_skb(ctx: &SkBuffContext, egress: bool) -> i32 {
    let Some(scope) = skb_scope(ctx) else {
        return 1;
    };
    let flags = scope_flags(scope);
    let mut t = Tuple::zero();
    if parse_skb(ctx, &mut t) == 0 {
        return 1;
    }
    let (remote, rport) = if egress { (t.dst, t.dport) } else { (t.src, t.sport) };
    let mut kind = 0;
    let mut policy = 0;
    if flags & IF_DENY != 0 {
        if let Some(p) = check_deny(scope, &remote) {
            kind = NET_EV_DENY;
            policy = p;
        } else if egress && t.proto == IPPROTO_UDP {
            if let Some(p) = check_deny_port(scope, t.proto, rport) {
                kind = NET_EV_DENY;
                policy = p;
            }
        }
    }
    // TCP egress is gated at connect(); only connectionless traffic is checked here.
    if kind == 0
        && egress
        && flags & IF_ALLOW != 0
        && t.proto == IPPROTO_UDP
        && !check_allow(scope, t.proto, rport, &remote)
    {
        kind = NET_EV_ALLOW_MISS;
    }
    if kind == 0 {
        return 1;
    }
    let now = now_ns();
    let key = FlowKey {
        proto: t.proto,
        remote,
        remote_port: rport,
        ..FlowKey::default()
    };
    if enforce_active(now) {
        emit_net(now, kind, VERDICT_DROP, policy, ctx.len(), &key, 0, 0);
        0
    } else {
        emit_net(now, kind, VERDICT_OBSERVED, policy, ctx.len(), &key, 0, 0);
        1
    }
}

#[cgroup_skb]
pub fn mn_cg_skb_ingress(ctx: SkBuffContext) -> i32 {
    cg_skb(&ctx, false)
}

#[cgroup_skb]
pub fn mn_cg_skb_egress(ctx: SkBuffContext) -> i32 {
    cg_skb(&ctx, true)
}

#[inline(always)]
fn sock_addr_check(ctx: &SockAddrContext, v6: bool) -> i32 {
    let Some(scope) = current_scope() else {
        return 1;
    };
    let flags = scope_flags(scope);
    let sa = ctx.sock_addr;
    let (remote, port, proto) = unsafe {
        let port = u16::from_be((*sa).user_port as u16);
        let proto = (*sa).protocol as u8;
        let remote = if v6 {
            let w = (*sa).user_ip6;
            let mut a = [0u8; ADDR_LEN];
            let b0 = w[0].to_ne_bytes();
            let b1 = w[1].to_ne_bytes();
            let b2 = w[2].to_ne_bytes();
            let b3 = w[3].to_ne_bytes();
            a[0..4].copy_from_slice(&b0);
            a[4..8].copy_from_slice(&b1);
            a[8..12].copy_from_slice(&b2);
            a[12..16].copy_from_slice(&b3);
            a
        } else {
            v4_mapped((*sa).user_ip4.to_ne_bytes())
        };
        (remote, port, proto)
    };
    let mut kind = 0;
    let mut policy = 0;
    if flags & IF_DENY != 0 {
        if let Some(p) = check_deny(scope, &remote) {
            kind = NET_EV_DENY;
            policy = p;
        } else if let Some(p) = check_deny_port(scope, proto, port) {
            kind = NET_EV_DENY;
            policy = p;
        }
    }
    if kind == 0 && flags & IF_ALLOW != 0 && !check_allow(scope, proto, port, &remote) {
        kind = NET_EV_ALLOW_MISS;
    }
    if kind == 0 {
        return 1;
    }
    let now = now_ns();
    let key = FlowKey {
        proto,
        remote,
        remote_port: port,
        ..FlowKey::default()
    };
    if enforce_active(now) {
        emit_net(now, kind, VERDICT_DROP, policy, 0, &key, 0, 0);
        0
    } else {
        emit_net(now, kind, VERDICT_OBSERVED, policy, 0, &key, 0, 0);
        1
    }
}

#[cgroup_sock_addr(connect4)]
pub fn mn_cg_connect4(ctx: SockAddrContext) -> i32 {
    sock_addr_check(&ctx, false)
}

#[cgroup_sock_addr(connect6)]
pub fn mn_cg_connect6(ctx: SockAddrContext) -> i32 {
    sock_addr_check(&ctx, true)
}

#[cgroup_sock_addr(sendmsg4)]
pub fn mn_cg_sendmsg4(ctx: SockAddrContext) -> i32 {
    sock_addr_check(&ctx, false)
}

#[cgroup_sock_addr(sendmsg6)]
pub fn mn_cg_sendmsg6(ctx: SockAddrContext) -> i32 {
    sock_addr_check(&ctx, true)
}

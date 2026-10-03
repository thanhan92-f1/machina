//! machina-cni datapath: pod-to-pod redirect, stateful NetworkPolicy,
//! NodePort DNAT (local backends) and socket-level service load balancing.
//! IPv4 only.

use aya_ebpf::{
    helpers::generated::{bpf_get_prandom_u32, bpf_get_socket_cookie, bpf_redirect_peer},
    macros::{cgroup_sock_addr, classifier},
    maps::lpm_trie::Key,
    programs::{SockAddrContext, TcContext},
};
use machina_bpf_common::*;

use crate::{
    maps::*,
    net::{TC_ACT_SHOT, TC_ACT_UNSPEC, emit_net, now_ns},
    parse::*,
};

const BPF_F_PSEUDO_HDR: u64 = 0x10;
const BPF_NOEXIST: u64 = 1;

#[inline(always)]
fn v4(a: &[u8; ADDR_LEN]) -> [u8; 4] {
    [a[12], a[13], a[14], a[15]]
}

#[inline(always)]
fn identity_of(addr: &[u8; 4]) -> u32 {
    if let Some(n) = CNI_NODE.get(0) {
        if n.node_addr == *addr {
            return IDENTITY_HOST;
        }
    }
    if let Some(&id) = unsafe { CNI_IDENTITIES.get(addr) } {
        return id;
    }
    match CNI_CIDR_IDS.get(&Key::new(32, *addr)) {
        Some(&id) => id,
        None => IDENTITY_WORLD,
    }
}

#[inline(always)]
fn pol(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> bool {
    let k = PolicyKey {
        subject_identity: subject,
        peer_identity: peer,
        direction: dir,
        proto,
        port: port.to_be_bytes(),
    };
    unsafe { CNI_POLICY.get(&k) }.is_some()
}

#[inline(always)]
fn policy_allows(subject: u32, peer: u32, dir: u8, proto: u8, port: u16) -> bool {
    pol(subject, peer, dir, proto, port)
        || pol(subject, peer, dir, proto, 0)
        || pol(subject, 0, dir, proto, port)
        || pol(subject, 0, dir, proto, 0)
        || pol(subject, peer, dir, 0, 0)
        || pol(subject, 0, dir, 0, 0)
}

#[inline(always)]
fn ct_key(t: &Tuple, reverse: bool) -> FlowKey {
    if reverse {
        FlowKey {
            proto: t.proto,
            local_port: t.dport,
            remote_port: t.sport,
            local: t.dst,
            remote: t.src,
            ..FlowKey::default()
        }
    } else {
        FlowKey {
            proto: t.proto,
            local_port: t.sport,
            remote_port: t.dport,
            local: t.src,
            remote: t.dst,
            ..FlowKey::default()
        }
    }
}

#[inline(always)]
fn is_reply(t: &Tuple) -> bool {
    CNI_CT.get_ptr(&ct_key(t, true)).is_some()
}

#[inline(always)]
fn ct_track(t: &Tuple, now: u64) {
    let k = ct_key(t, false);
    match CNI_CT.get_ptr_mut(&k) {
        Some(p) => unsafe { *p = now },
        None => {
            let _ = CNI_CT.insert(&k, &now, 0);
        }
    }
}

#[inline(always)]
fn l4_csum_off(t: &Tuple) -> Option<usize> {
    match t.proto {
        IPPROTO_TCP => Some(t.l4_off + 16),
        IPPROTO_UDP => Some(t.l4_off + 6),
        _ => None,
    }
}

/// Rewrite an IPv4 address (src: l3+12, dst: l3+16) and its L4 port.
#[inline(always)]
fn rewrite(ctx: &TcContext, t: &Tuple, dst: bool, addr: [u8; 4], port: [u8; 2]) -> bool {
    let Some(csum_off) = l4_csum_off(t) else {
        return false;
    };
    let ip_off = t.l3_off + if dst { 16 } else { 12 };
    let port_off = t.l4_off + if dst { 2 } else { 0 };
    let old_ip: [u8; 4] = if dst { v4(&t.dst) } else { v4(&t.src) };
    let old_port: [u8; 2] = if dst {
        t.dport.to_be_bytes()
    } else {
        t.sport.to_be_bytes()
    };
    let from_ip = u32::from_ne_bytes(old_ip) as u64;
    let to_ip = u32::from_ne_bytes(addr) as u64;
    let from_port = u16::from_ne_bytes(old_port) as u64;
    let to_port = u16::from_ne_bytes(port) as u64;

    let udp_nocsum = t.proto == IPPROTO_UDP
        && matches!(ctx.load::<[u8; 2]>(csum_off), Ok([0, 0]));
    if !udp_nocsum {
        if ctx
            .l4_csum_replace(csum_off, from_ip, to_ip, BPF_F_PSEUDO_HDR | 4)
            .is_err()
            || ctx.l4_csum_replace(csum_off, from_port, to_port, 2).is_err()
        {
            return false;
        }
    }
    if ctx.l3_csum_replace(t.l3_off + 10, from_ip, to_ip, 4).is_err() {
        return false;
    }
    ctx.store(ip_off, &addr, 0).is_ok() && ctx.store(port_off, &port, 0).is_ok()
}

#[inline(always)]
fn deny(now: u64, t: &Tuple, len: u32) -> i32 {
    let key = ct_key(t, false);
    emit_net(now, NET_EV_DENY, VERDICT_DROP, 0, len, &key, 0, 0);
    TC_ACT_SHOT
}

/// TC ingress on a pod's host-side veth (traffic leaving the pod).
#[classifier]
pub fn mn_cni_from_pod(ctx: TcContext) -> i32 {
    let mut t = Tuple::zero();
    if parse_tc(&ctx, &mut t) == 0 {
        return TC_ACT_UNSPEC;
    }
    if t.v6 {
        return TC_ACT_UNSPEC;
    }
    let now = now_ns();
    let len = ctx.len();
    let src4 = v4(&t.src);
    let dst4 = v4(&t.dst);
    let Some(src_ep) = (unsafe { CNI_ENDPOINTS.get(&src4) }).copied() else {
        return TC_ACT_UNSPEC;
    };

    // NodePort reply: undo the DNAT done on the uplink.
    if t.proto == IPPROTO_TCP || t.proto == IPPROTO_UDP {
        let rk = NatCtKey {
            backend_addr: src4,
            client_addr: dst4,
            backend_port: t.sport.to_be_bytes(),
            client_port: t.dport.to_be_bytes(),
            proto: t.proto,
            _pad: [0; 3],
        };
        if let Some(ct) = CNI_NODEPORT_CT.get_ptr_mut(&rk) {
            let (fa, fp) = unsafe {
                (*ct).last_ns = now;
                ((*ct).frontend_addr, (*ct).frontend_port)
            };
            if !rewrite(&ctx, &t, false, fa, fp) {
                return TC_ACT_SHOT;
            }
            return TC_ACT_UNSPEC;
        }
    }

    let reply = is_reply(&t);
    let dst_ep = unsafe { CNI_ENDPOINTS.get(&dst4) }.copied();
    let dst_id = match dst_ep {
        Some(ep) => ep.identity,
        None => identity_of(&dst4),
    };
    if !reply {
        if src_ep.flags & EP_EGRESS_ISOLATED != 0
            && !policy_allows(src_ep.identity, dst_id, POLICY_EGRESS, t.proto, t.dport)
        {
            return deny(now, &t, len);
        }
        if let Some(ep) = dst_ep {
            if ep.flags & EP_INGRESS_ISOLATED != 0
                && !policy_allows(ep.identity, src_ep.identity, POLICY_INGRESS, t.proto, t.dport)
            {
                return deny(now, &t, len);
            }
        }
        ct_track(&t, now);
    }

    let Some(ep) = dst_ep else {
        return TC_ACT_UNSPEC;
    };
    if ctx.store(0, &ep.pod_mac, 0).is_err() || ctx.store(6, &ep.host_mac, 0).is_err() {
        return TC_ACT_UNSPEC;
    }
    unsafe { bpf_redirect_peer(ep.host_ifindex, 0) as i32 }
}

/// TC egress on a pod's host-side veth (traffic entering the pod via the
/// kernel stack: other nodes, host, NodePort).
#[classifier]
pub fn mn_cni_to_pod(ctx: TcContext) -> i32 {
    let mut t = Tuple::zero();
    if parse_tc(&ctx, &mut t) == 0 {
        return TC_ACT_UNSPEC;
    }
    if t.v6 {
        return TC_ACT_UNSPEC;
    }
    let dst4 = v4(&t.dst);
    let Some(ep) = (unsafe { CNI_ENDPOINTS.get(&dst4) }).copied() else {
        return TC_ACT_UNSPEC;
    };
    let now = now_ns();
    if is_reply(&t) {
        return TC_ACT_UNSPEC;
    }
    if ep.flags & EP_INGRESS_ISOLATED != 0 {
        let src_id = identity_of(&v4(&t.src));
        if src_id != IDENTITY_HOST
            && !policy_allows(ep.identity, src_id, POLICY_INGRESS, t.proto, t.dport)
        {
            return deny(now, &t, ctx.len());
        }
    }
    ct_track(&t, now);
    TC_ACT_UNSPEC
}

/// TC ingress on the node uplink: NodePort → local backend DNAT.
#[classifier]
pub fn mn_cni_nodeport(ctx: TcContext) -> i32 {
    let mut t = Tuple::zero();
    if parse_tc(&ctx, &mut t) == 0 {
        return TC_ACT_UNSPEC;
    }
    if t.v6 || (t.proto != IPPROTO_TCP && t.proto != IPPROTO_UDP) {
        return TC_ACT_UNSPEC;
    }
    let Some(node) = CNI_NODE.get(0) else {
        return TC_ACT_UNSPEC;
    };
    let dst4 = v4(&t.dst);
    if node.node_addr != dst4 {
        return TC_ACT_UNSPEC;
    }
    let svc_key = SvcKey {
        addr: [0; 4],
        port: t.dport.to_be_bytes(),
        proto: t.proto,
        _pad: 0,
    };
    let Some(svc) = (unsafe { CNI_NODEPORTS.get(&svc_key) }).copied() else {
        return TC_ACT_UNSPEC;
    };
    if svc.backend_count == 0 {
        return TC_ACT_UNSPEC;
    }
    let src4 = v4(&t.src);
    let fwd_key = NatCtKey {
        backend_addr: dst4,
        client_addr: src4,
        backend_port: t.dport.to_be_bytes(),
        client_port: t.sport.to_be_bytes(),
        proto: t.proto,
        _pad: [0; 3],
    };
    let be = match unsafe { CNI_NODEPORT_FWD.get(&fwd_key) } {
        Some(b) => *b,
        None => {
            let idx = unsafe { bpf_get_prandom_u32() } % svc.backend_count;
            let Some(b) = (unsafe {
                CNI_BACKENDS.get(&BackendKey {
                    svc_id: svc.svc_id,
                    index: idx,
                })
            })
            .copied() else {
                return TC_ACT_UNSPEC;
            };
            let _ = CNI_NODEPORT_FWD.insert(&fwd_key, &b, 0);
            b
        }
    };
    let now = now_ns();
    let rev = NatCtKey {
        backend_addr: be.addr,
        client_addr: src4,
        backend_port: be.port,
        client_port: t.sport.to_be_bytes(),
        proto: t.proto,
        _pad: [0; 3],
    };
    let val = NatCtVal {
        frontend_addr: dst4,
        frontend_port: t.dport.to_be_bytes(),
        _pad: 0,
        last_ns: now,
    };
    let _ = CNI_NODEPORT_CT.insert(&rev, &val, 0);
    if !rewrite(&ctx, &t, true, be.addr, be.port) {
        return TC_ACT_SHOT;
    }
    TC_ACT_UNSPEC
}

// ---- socket LB --------------------------------------------------------------

#[inline(always)]
fn pick_backend(ctx: &SockAddrContext) -> Option<(SvcKey, Backend)> {
    let sa = ctx.sock_addr;
    let (addr, port, proto) = unsafe {
        (
            (*sa).user_ip4.to_ne_bytes(),
            ((*sa).user_port as u16).to_ne_bytes(),
            (*sa).protocol as u8,
        )
    };
    let key = SvcKey {
        addr,
        port,
        proto,
        _pad: 0,
    };
    let svc = match unsafe { CNI_SERVICES.get(&key) } {
        Some(s) => *s,
        None => {
            let node = CNI_NODE.get(0)?;
            if node.node_addr != addr {
                return None;
            }
            let np = SvcKey {
                addr: [0; 4],
                ..key
            };
            *unsafe { CNI_NODEPORTS.get(&np) }?
        }
    };
    if svc.backend_count == 0 {
        return None;
    }
    let idx = unsafe { bpf_get_prandom_u32() } % svc.backend_count;
    let be = unsafe {
        CNI_BACKENDS.get(&BackendKey {
            svc_id: svc.svc_id,
            index: idx,
        })
    }?;
    Some((key, *be))
}

#[inline(always)]
fn set_dest(ctx: &SockAddrContext, be: &Backend) {
    unsafe {
        (*ctx.sock_addr).user_ip4 = u32::from_ne_bytes(be.addr);
        (*ctx.sock_addr).user_port = u16::from_ne_bytes(be.port) as u32;
    }
}

#[cgroup_sock_addr(connect4)]
pub fn mn_cni_connect4(ctx: SockAddrContext) -> i32 {
    if let Some((_, be)) = pick_backend(&ctx) {
        set_dest(&ctx, &be);
    }
    1
}

#[cgroup_sock_addr(sendmsg4)]
pub fn mn_cni_sendmsg4(ctx: SockAddrContext) -> i32 {
    if let Some((svc, be)) = pick_backend(&ctx) {
        let cookie = unsafe { bpf_get_socket_cookie(ctx.sock_addr.cast()) };
        let k = RevNatKey {
            cookie,
            addr: be.addr,
            port: be.port,
            _pad: 0,
        };
        let frontend = Backend {
            addr: svc.addr,
            port: svc.port,
            _pad: 0,
        };
        let _ = CNI_UDP_REVNAT.insert(&k, &frontend, BPF_NOEXIST);
        set_dest(&ctx, &be);
    }
    1
}

#[cgroup_sock_addr(recvmsg4)]
pub fn mn_cni_recvmsg4(ctx: SockAddrContext) -> i32 {
    let (addr, port) = unsafe {
        (
            (*ctx.sock_addr).user_ip4.to_ne_bytes(),
            ((*ctx.sock_addr).user_port as u16).to_ne_bytes(),
        )
    };
    let cookie = unsafe { bpf_get_socket_cookie(ctx.sock_addr.cast()) };
    let k = RevNatKey {
        cookie,
        addr,
        port,
        _pad: 0,
    };
    if let Some(fe) = unsafe { CNI_UDP_REVNAT.get(&k) } {
        let fe = *fe;
        set_dest(&ctx, &fe);
    }
    1
}

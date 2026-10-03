// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Shared layouts between `machina-bpf-ebpf` (kernel) and `machina-bpf` (userspace).
//!
//! Every struct is `#[repr(C)]` with explicit padding so map keys never carry
//! uninitialised bytes. Addresses are always 16 bytes: IPv4 is stored as an
//! IPv4-mapped IPv6 address (`::ffff:a.b.c.d`).

#![no_std]

pub const ADDR_LEN: usize = 16;
pub const COMM_LEN: usize = 16;
pub const PATH_LEN: usize = 256;
pub const DNS_PAYLOAD_LEN: usize = 512;
pub const L7_PAYLOAD_LEN: usize = 512;
pub const CAPTURE_SNAPLEN: usize = 1024;
pub const FILE_WATCH_SLOTS: u32 = 8;
pub const FILE_WATCH_PREFIX_LEN: usize = 64;

/// Bits of the IPv4-mapped prefix (`::ffff:0:0/96`).
pub const V4_MAPPED_PREFIX_BITS: u32 = 96;

// ---------------------------------------------------------------------------
// Global config (CONFIG array, index 0)
// ---------------------------------------------------------------------------

pub const MODE_OBSERVE: u32 = 0;
pub const MODE_ENFORCE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalCfg {
    /// `MODE_OBSERVE` or `MODE_ENFORCE`.
    pub mode: u32,
    pub flags: u32,
    /// CLOCK_MONOTONIC deadline (ns). Enforce mode is only honoured while
    /// `bpf_ktime_get_ns() < lease_deadline_ns`; the datapath fails open to
    /// observe on its own when the lease lapses, even if the loader is gone.
    pub lease_deadline_ns: u64,
}

pub const GLOBAL_EXEC_DENY: u32 = 1 << 0;
pub const GLOBAL_FILE_WATCH: u32 = 1 << 1;
pub const GLOBAL_CONNECT_EVENTS: u32 = 1 << 2;
pub const GLOBAL_FORK_EVENTS: u32 = 1 << 3;
pub const GLOBAL_EXEC_EVENTS: u32 = 1 << 4;
pub const GLOBAL_CAP_DENY: u32 = 1 << 5;

// ---------------------------------------------------------------------------
// Per-interface config (IFACE_CFG hash, key = ifindex)
// ---------------------------------------------------------------------------

/// Ingress hook on this interface carries traffic *from* the workload
/// (true for VM taps / pod host-side veths, false for uplinks).
pub const IF_GUEST_SIDE: u32 = 1 << 0;
pub const IF_DENY: u32 = 1 << 1;
/// Default-deny workload egress unless an ALLOW rule matches.
pub const IF_ALLOW: u32 = 1 << 2;
pub const IF_FLOWS: u32 = 1 << 3;
pub const IF_CAPTURE: u32 = 1 << 4;
pub const IF_QOS: u32 = 1 << 5;
pub const IF_DNS: u32 = 1 << 6;
/// Token-bucket limit on new workload-initiated connections (`rate_limit`).
pub const IF_RATE: u32 = 1 << 7;
/// Report the first client payload of each TCP flow (TLS SNI / HTTP request).
pub const IF_L7: u32 = 1 << 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfaceCfg {
    /// Policy scope id (0 = host-global only).
    pub scope: u32,
    pub flags: u32,
    /// Rate of traffic sent by the workload, bytes/s. 0 = unlimited.
    /// Paced with EDT (needs fq) on egress hooks, policed on ingress hooks.
    pub qos_egress_bps: u64,
    /// Rate of traffic towards the workload, bytes/s. 0 = unlimited.
    pub qos_ingress_bps: u64,
    /// Capture 1-in-N packets (0 or 1 = every packet).
    pub capture_sample: u32,
    pub capture_snaplen: u32,
}

// ---------------------------------------------------------------------------
// Enforcement keys
// ---------------------------------------------------------------------------

/// LPM data for DENY_LPM. Prefix length = 32 (scope) + address bits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DenyKey {
    pub scope: [u8; 4],
    pub addr: [u8; ADDR_LEN],
}

/// LPM data for ALLOW_LPM. Prefix length = 64 (scope+proto+pad+port) + address bits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AllowKey {
    pub scope: [u8; 4],
    /// IPPROTO_TCP / IPPROTO_UDP, 0 = any.
    pub proto: u8,
    pub _pad: u8,
    /// Network byte order, 0 = any.
    pub port: [u8; 2],
    pub addr: [u8; ADDR_LEN],
}

pub const DENY_KEY_FIXED_BITS: u32 = 32;
pub const ALLOW_KEY_FIXED_BITS: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuleVal {
    pub policy_id: u32,
    pub flags: u32,
}

// ---------------------------------------------------------------------------
// Flows
// ---------------------------------------------------------------------------

/// Flow key from the workload's point of view.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub ifindex: u32,
    pub proto: u8,
    pub _pad: [u8; 3],
    pub local_port: u16,
    pub remote_port: u16,
    pub _pad2: u32,
    pub local: [u8; ADDR_LEN],
    pub remote: [u8; ADDR_LEN],
}

pub const ORIGIN_LOCAL: u8 = 1;
pub const ORIGIN_REMOTE: u8 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlowVal {
    pub first_ns: u64,
    pub last_ns: u64,
    pub tx_pkts: u64,
    pub tx_bytes: u64,
    pub rx_pkts: u64,
    pub rx_bytes: u64,
    pub tcp_flags: u32,
    pub origin: u8,
    pub verdict: u8,
    pub _pad: u16,
}

/// For FLOW_OPEN, `NetEvent::policy_id` carries the flow origin (ORIGIN_*).
pub const NET_EV_FLOW_OPEN: u32 = 1;
pub const NET_EV_FLOW_CLOSE: u32 = 2;
pub const NET_EV_DENY: u32 = 3;
pub const NET_EV_ALLOW_MISS: u32 = 4;
pub const NET_EV_QOS_DROP: u32 = 5;
pub const NET_EV_RATE_LIMIT: u32 = 6;

/// Set in `FlowVal::tcp_flags` (above the 8 TCP flag bits) once the flow's
/// first client payload has been reported on L7_EVENTS.
pub const FLOW_L7_SEEN: u32 = 1 << 16;

/// RATE_CFG value (key = policy scope): new connections per second + burst.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RateCfg {
    pub per_sec: u32,
    pub burst: u32,
    pub policy_id: u32,
    pub _pad: u32,
}

/// IFACE_STATS per-CPU value (key = ifindex), workload point of view.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfaceStats {
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_pkts: u64,
    pub rx_pkts: u64,
    pub drops: u64,
}

pub const VERDICT_PASS: u32 = 0;
pub const VERDICT_DROP: u32 = 1;
/// Would have dropped, but the datapath is in observe mode (or lease expired).
pub const VERDICT_OBSERVED: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetEvent {
    pub ts_ns: u64,
    pub kind: u32,
    pub verdict: u32,
    pub policy_id: u32,
    pub pkt_len: u32,
    pub key: FlowKey,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

// ---------------------------------------------------------------------------
// DNS / capture
// ---------------------------------------------------------------------------

pub const DIR_FROM_WORKLOAD: u32 = 1;
pub const DIR_TO_WORKLOAD: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DnsEvent {
    pub ts_ns: u64,
    pub ifindex: u32,
    pub dir: u32,
    pub payload_len: u32,
    pub _pad: u32,
    pub local: [u8; ADDR_LEN],
    pub remote: [u8; ADDR_LEN],
    pub payload: [u8; DNS_PAYLOAD_LEN],
}

/// First client→server payload of a TCP flow (`key` from the workload's view).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct L7Event {
    pub ts_ns: u64,
    pub ifindex: u32,
    pub dir: u32,
    pub payload_len: u32,
    pub _pad: u32,
    pub key: FlowKey,
    pub payload: [u8; L7_PAYLOAD_LEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CaptureEvent {
    pub ts_ns: u64,
    pub ifindex: u32,
    pub dir: u32,
    pub pkt_len: u32,
    pub cap_len: u32,
    pub data: [u8; CAPTURE_SNAPLEN],
}

// ---------------------------------------------------------------------------
// Process telemetry
// ---------------------------------------------------------------------------

pub const PROC_EV_EXEC: u32 = 1;
pub const PROC_EV_EXIT: u32 = 2;
pub const PROC_EV_FORK: u32 = 3;
pub const PROC_EV_FILE_OPEN: u32 = 4;
pub const PROC_EV_CONNECT: u32 = 5;

pub const PROC_FLAG_EXEC_DENIED: u32 = 1 << 0;
pub const PROC_FLAG_EXEC_KILLED: u32 = 1 << 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcEvent {
    pub ts_ns: u64,
    pub cgroup_id: u64,
    pub kind: u32,
    pub pid: u32,
    pub tgid: u32,
    /// Child pid for FORK, 0 otherwise.
    pub child_pid: u32,
    pub uid: u32,
    pub gid: u32,
    pub flags: u32,
    pub policy_id: u32,
    /// openat flags for FILE_OPEN.
    pub open_flags: u32,
    pub family: u16,
    pub proto: u16,
    pub sport: u16,
    pub dport: u16,
    pub _pad: u32,
    pub saddr: [u8; ADDR_LEN],
    pub daddr: [u8; ADDR_LEN],
    pub comm: [u8; COMM_LEN],
    pub path: [u8; PATH_LEN],
}

/// A path prefix watched by the openat tracepoint (FILE_WATCH array).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FileWatch {
    pub len: u32,
    /// `WATCH_DENY`: SIGKILL the opener while enforcement is active.
    pub flags: u32,
    pub policy_id: u32,
    pub _pad: u32,
    pub prefix: [u8; FILE_WATCH_PREFIX_LEN],
}

pub const WATCH_DENY: u32 = 1;

/// DENY_PORTS key: remote port deny (`deny_port`). `proto` 0 = any.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PortKey {
    pub scope: u32,
    pub proto: u8,
    pub _pad: u8,
    /// Network byte order.
    pub port: [u8; 2],
}

/// CAP_DENY key (`deny_cap`): capability number within a scope.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CapKey {
    pub scope: u32,
    pub cap: u32,
}

pub const PROC_EV_CAP_DENIED: u32 = 6;

/// Tracepoint field offsets discovered from tracefs `format` files at load time
/// (TP_OFF array). The kernel side never hard-codes tracepoint layouts.
pub mod tp {
    pub const EXEC_FILENAME_LOC: u32 = 0;
    pub const FORK_PARENT_PID: u32 = 1;
    pub const FORK_CHILD_PID: u32 = 2;
    pub const OPENAT_FILENAME: u32 = 3;
    pub const OPENAT_FLAGS: u32 = 4;
    pub const ISS_OLDSTATE: u32 = 5;
    pub const ISS_NEWSTATE: u32 = 6;
    pub const ISS_SPORT: u32 = 7;
    pub const ISS_DPORT: u32 = 8;
    pub const ISS_FAMILY: u32 = 9;
    pub const ISS_PROTOCOL: u32 = 10;
    pub const ISS_SADDR_V6: u32 = 11;
    pub const ISS_DADDR_V6: u32 = 12;
    pub const KFREE_REASON: u32 = 13;
    pub const RETRANS_SADDR_V6: u32 = 14;
    pub const SEND_RST_SADDR_V6: u32 = 15;
    pub const RECV_RST_SADDR_V6: u32 = 16;
    pub const COUNT: u32 = 17;
    /// Sentinel for "field not present on this kernel".
    pub const MISSING: u32 = u32::MAX;
}

// ---------------------------------------------------------------------------
// Network health
// ---------------------------------------------------------------------------

pub const HEALTH_RETRANSMIT: u32 = 1;
pub const HEALTH_RST_SENT: u32 = 2;
pub const HEALTH_RST_RECV: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HealthKey {
    pub kind: u32,
    pub _pad: u32,
    pub addr: [u8; ADDR_LEN],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QosState {
    pub tokens: u64,
    pub last_ns: u64,
    pub next_tstamp: u64,
}

// ---------------------------------------------------------------------------
// CNI datapath
// ---------------------------------------------------------------------------

pub const IDENTITY_HOST: u32 = 1;
pub const IDENTITY_WORLD: u32 = 2;

pub const EP_INGRESS_ISOLATED: u32 = 1 << 0;
pub const EP_EGRESS_ISOLATED: u32 = 1 << 1;

/// Version of the CNI map/state ABI; bpfd rejects `CniSync` from agents
/// built against another one. Addresses are 16-byte (IPv4-mapped for v4).
pub const CNI_ABI_VERSION: u32 = 2;

/// Local pod endpoint (CNI_ENDPOINTS, key = 16-byte pod address).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Endpoint {
    pub host_ifindex: u32,
    pub identity: u32,
    pub flags: u32,
    pub _pad: u32,
    pub pod_mac: [u8; 6],
    pub host_mac: [u8; 6],
    pub _pad2: u32,
}

/// NetworkPolicy verdict key (CNI_POLICY). `peer_identity` 0 = any peer,
/// `port` 0 = any port, `proto` 0 = any protocol.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PolicyKey {
    pub subject_identity: u32,
    pub peer_identity: u32,
    /// 0 = ingress to subject, 1 = egress from subject.
    pub direction: u8,
    pub proto: u8,
    /// Network byte order.
    pub port: [u8; 2],
}

pub const POLICY_INGRESS: u8 = 0;
pub const POLICY_EGRESS: u8 = 1;

/// ClusterIP / NodePort service frontend (CNI_SERVICES / CNI_NODEPORTS).
/// NodePorts use the unspecified address of their family
/// (`::ffff:0.0.0.0` for IPv4, `::` for IPv6).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SvcKey {
    pub addr: [u8; ADDR_LEN],
    /// Network order.
    pub port: [u8; 2],
    pub proto: u8,
    pub _pad: u8,
}

/// Session affinity by client address (`sessionAffinity: ClientIP`).
pub const SVC_F_AFFINITY: u32 = 1 << 0;
/// externalTrafficPolicy=Local: NodePort only uses this node's backends.
pub const SVC_F_LOCAL: u32 = 1 << 1;

/// Maglev lookup table size per service (prime, ≥ 10× the backends of any
/// realistic service so disruption on backend churn stays near 1/N).
pub const MAGLEV_M: u32 = 1021;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SvcVal {
    pub svc_id: u32,
    pub backend_count: u32,
    pub flags: u32,
    pub affinity_secs: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BackendKey {
    pub svc_id: u32,
    pub index: u32,
}

/// Backend lives on another node (NodePort needs SNAT or DSR to reach it).
pub const BE_F_REMOTE: u8 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Backend {
    pub addr: [u8; ADDR_LEN],
    pub port: [u8; 2],
    pub flags: u8,
    pub _pad: u8,
    /// Remote node address hosting the backend (DSR encap target).
    pub node: [u8; ADDR_LEN],
}

/// Maglev slot (CNI_MAGLEV): `slot` in [0, MAGLEV_M) → backend index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MaglevKey {
    pub svc_id: u32,
    pub slot: u32,
}

/// Session affinity entry (CNI_AFFINITY): (service, client) → backend index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AffinityKey {
    pub client: [u8; ADDR_LEN],
    pub svc_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AffinityVal {
    pub index: u32,
    pub _pad: u32,
    pub last_ns: u64,
}

/// UDP reverse translation for socket LB (CNI_UDP_REVNAT).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RevNatKey {
    pub cookie: u64,
    pub addr: [u8; ADDR_LEN],
    pub port: [u8; 2],
    pub _pad: [u8; 6],
}

/// NodePort DNAT conntrack (CNI_NODEPORT_CT): key is the backend-side tuple
/// as seen on the reply path.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct NatCtKey {
    pub backend_addr: [u8; ADDR_LEN],
    pub client_addr: [u8; ADDR_LEN],
    pub backend_port: [u8; 2],
    pub client_port: [u8; 2],
    pub proto: u8,
    pub _pad: [u8; 3],
}

/// The client address was replaced by the node address (eTP=Cluster SNAT).
pub const NAT_F_SNAT: u16 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NatCtVal {
    pub frontend_addr: [u8; ADDR_LEN],
    pub frontend_port: [u8; 2],
    pub flags: u16,
    pub _pad: u32,
    /// Original client address when `NAT_F_SNAT` is set.
    pub client_addr: [u8; ADDR_LEN],
    pub last_ns: u64,
}

/// NodePort to remote backends: 0 = SNAT through this node, 1 = DSR (IPIP).
pub const NODE_F_DSR: u32 = 1 << 0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeCfg {
    /// Node primary IPv4 (IPv4-mapped) for NodePort matching.
    pub node_addr: [u8; ADDR_LEN],
    /// Node primary IPv6, all-zero when the node is single-stack.
    pub node_addr6: [u8; ADDR_LEN],
    pub flags: u32,
    pub uplink_ifindex: u32,
}

// ---------------------------------------------------------------------------
// Uplink XDP dispatcher
// ---------------------------------------------------------------------------

pub const XDP_SLOTS: u32 = 4;
pub const XDP_SLOT_NODEPORT: u32 = 0;

pub const XDP_F_SHIELD: u32 = 1 << 0;
pub const XDP_F_NODEPORT: u32 = 1 << 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct XdpCfg {
    pub flags: u32,
    pub _pad: u32,
}

// ---------------------------------------------------------------------------
// Helpers usable from both sides
// ---------------------------------------------------------------------------

/// IPv4 → IPv4-mapped IPv6 bytes.
#[inline(always)]
pub const fn v4_mapped(o: [u8; 4]) -> [u8; ADDR_LEN] {
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, o[0], o[1], o[2], o[3],
    ]
}

/// FNV-1a 64 over a NUL-terminated byte path (shared by the exec deny map).
#[inline(always)]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 {
            break;
        }
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

#[cfg(all(feature = "user", target_os = "linux"))]
mod pod {
    use super::*;
    macro_rules! pod {
        ($($t:ty),*) => { $(unsafe impl aya::Pod for $t {})* };
    }
    pod!(
        GlobalCfg, IfaceCfg, DenyKey, AllowKey, RuleVal, FlowKey, FlowVal, NetEvent, FileWatch,
        PortKey, CapKey, HealthKey, QosState, Endpoint, PolicyKey, SvcKey, SvcVal, BackendKey, Backend, RevNatKey,
        NatCtKey, NatCtVal, NodeCfg, RateCfg, IfaceStats, MaglevKey, AffinityKey, AffinityVal, XdpCfg
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn layouts_have_no_implicit_padding() {
        assert_eq!(size_of::<GlobalCfg>(), 16);
        assert_eq!(size_of::<IfaceCfg>(), 32);
        assert_eq!(size_of::<DenyKey>(), 20);
        assert_eq!(size_of::<AllowKey>(), 24);
        assert_eq!(size_of::<FlowKey>(), 48);
        assert_eq!(size_of::<FlowVal>(), 56);
        assert_eq!(size_of::<NetEvent>(), 88);
        assert_eq!(size_of::<ProcEvent>(), 64 + 32 + 16 + PATH_LEN);
        assert_eq!(size_of::<Endpoint>(), 32);
        assert_eq!(size_of::<PolicyKey>(), 12);
        assert_eq!(size_of::<NatCtKey>(), 40);
        assert_eq!(size_of::<NatCtVal>(), 48);
        assert_eq!(size_of::<SvcKey>(), 20);
        assert_eq!(size_of::<SvcVal>(), 16);
        assert_eq!(size_of::<Backend>(), 36);
        assert_eq!(size_of::<RevNatKey>(), 32);
        assert_eq!(size_of::<AffinityKey>(), 20);
        assert_eq!(size_of::<AffinityVal>(), 16);
        assert_eq!(size_of::<NodeCfg>(), 40);
        assert_eq!(size_of::<RateCfg>(), 16);
        assert_eq!(size_of::<IfaceStats>(), 40);
        assert_eq!(size_of::<L7Event>(), 24 + 48 + L7_PAYLOAD_LEN);
    }

    #[test]
    fn fnv_stops_at_nul() {
        assert_eq!(fnv1a64(b"/bin/sh\0garbage"), fnv1a64(b"/bin/sh"));
        assert_ne!(fnv1a64(b"/bin/sh"), fnv1a64(b"/bin/bash"));
    }

    #[test]
    fn v4_mapping() {
        assert_eq!(&v4_mapped([10, 0, 0, 1])[10..], &[0xff, 0xff, 10, 0, 0, 1]);
    }
}

// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Wire types for the machina-bpfd Unix-socket API (newline-delimited JSON).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_SOCKET: &str = "/run/machina-bpf/bpfd.sock";
pub const SOCKET_ENV: &str = "MACHINA_BPFD_SOCK";

/// Where a policy applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Scope {
    /// Every VM tap and every registered cgroup on the host.
    #[default]
    Host,
    /// Taps belonging to one libvirt domain.
    Vm { name: String },
    /// A cgroup v2 subtree (containers, systemd units), path relative to /sys/fs/cgroup.
    Cgroup { path: String },
}

impl Scope {
    pub fn key(&self) -> Option<String> {
        match self {
            Scope::Host => None,
            Scope::Vm { name } => Some(format!("vm:{name}")),
            Scope::Cgroup { path } => Some(format!("cgroup:{}", path.trim_matches('/'))),
        }
    }

    pub fn label(&self) -> String {
        self.key().unwrap_or_else(|| "host".into())
    }
}

/// Policy kinds understood by the native datapath.
pub const POLICY_KINDS: &[&str] = &[
    "deny_ip",
    "deny_port",
    "tc_allow",
    "allow_port",
    "deny_process",
    "deny_file",
    "deny_cap",
    "deny_dns",
    "rate_limit",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: String,
    #[serde(rename = "match")]
    pub match_value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scope: Scope,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Observe,
    Enforce,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModeState {
    pub mode: Mode,
    /// RFC 3339 wall-clock expiry of the enforce lease.
    pub lease_expires_at: Option<String>,
    pub lease_remaining_secs: Option<u64>,
    /// True when enforce was requested but the lease has lapsed (datapath failed open).
    #[serde(default)]
    pub lease_expired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryConfig {
    #[serde(default = "default_true")]
    pub exec: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default = "default_true")]
    pub connect: bool,
    #[serde(default = "default_true")]
    pub flows: bool,
    #[serde(default = "default_true")]
    pub dns: bool,
    /// First client payload per TCP flow: TLS SNI/ALPN, HTTP request line, SSH banner.
    #[serde(default = "default_true")]
    pub l7: bool,
    /// Path prefixes reported on open (max 8 including deny_file prefixes).
    #[serde(default = "default_watch")]
    pub file_watch: Vec<String>,
    /// Interface name globs auto-attached as workload taps.
    #[serde(default = "default_patterns")]
    pub iface_patterns: Vec<String>,
}

fn default_watch() -> Vec<String> {
    vec![
        "/etc/shadow".into(),
        "/etc/sudoers".into(),
        "/root/.ssh/".into(),
        "/etc/machina/".into(),
        "/etc/libvirt/".into(),
    ]
}

fn default_patterns() -> Vec<String> {
    vec!["vnet*".into(), "tap*".into()]
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            exec: true,
            fork: false,
            connect: true,
            flows: true,
            dns: true,
            l7: true,
            file_watch: default_watch(),
            iface_patterns: default_patterns(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KernelFeatures {
    pub kernel: String,
    pub btf: bool,
    pub tcx: bool,
    pub lsm_bpf: bool,
    pub tracefs: Option<String>,
    pub cgroup2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IfaceStatus {
    pub name: String,
    pub ifindex: u32,
    pub vm: Option<String>,
    pub mac: Option<String>,
    pub scope: u32,
    pub flags: Vec<String>,
    pub guest_side: bool,
    pub xdp: bool,
    pub qos_egress_bps: u64,
    pub qos_ingress_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Counters {
    pub net_events: u64,
    pub drops: u64,
    pub observed: u64,
    pub flows_opened: u64,
    pub proc_events: u64,
    pub dns_events: u64,
    pub capture_packets: u64,
    pub anomalies: u64,
    #[serde(default)]
    pub l7_events: u64,
    #[serde(default)]
    pub rate_limited: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BpfStatus {
    pub available: bool,
    pub programs_compiled: bool,
    pub version: String,
    pub features: KernelFeatures,
    pub mode: ModeState,
    pub policies_total: usize,
    pub policies_enabled: usize,
    pub interfaces: Vec<IfaceStatus>,
    pub cgroups: Vec<String>,
    pub tracepoints: Vec<String>,
    pub counters: Counters,
    pub telemetry: TelemetryConfig,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FlowRecord {
    pub iface: String,
    pub ifindex: u32,
    pub vm: Option<String>,
    pub proto: String,
    pub local: String,
    pub local_port: u16,
    pub remote: String,
    pub remote_port: u16,
    /// "local" (workload initiated) or "remote".
    pub origin: String,
    pub tx_pkts: u64,
    pub tx_bytes: u64,
    pub rx_pkts: u64,
    pub rx_bytes: u64,
    pub first_seen: String,
    pub last_seen: String,
    /// "pass" | "drop" | "observed"
    pub verdict: String,
    pub tcp_flags: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetEventRecord {
    pub ts: String,
    /// flow_open | flow_close | deny | allow_miss | qos_drop
    pub kind: String,
    /// pass | drop | observed
    pub verdict: String,
    pub policy_id: Option<String>,
    pub iface: Option<String>,
    pub vm: Option<String>,
    pub proto: String,
    pub local: String,
    pub local_port: u16,
    pub remote: String,
    pub remote_port: u16,
    pub pkt_len: u32,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DnsAnswer {
    pub name: String,
    pub rtype: String,
    pub ttl: u32,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DnsRecord {
    pub ts: String,
    pub iface: Option<String>,
    pub vm: Option<String>,
    pub client: String,
    pub server: String,
    pub id: u16,
    pub is_response: bool,
    pub qname: String,
    pub qtype: String,
    pub rcode: String,
    pub answers: Vec<DnsAnswer>,
}

/// First client payload of a TCP flow, classified.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct L7Record {
    pub ts: String,
    pub iface: Option<String>,
    pub vm: Option<String>,
    /// tls | http | ssh
    pub protocol: String,
    /// "outbound" (workload is the client) or "inbound".
    pub direction: String,
    pub client: String,
    pub client_port: u16,
    pub server: String,
    pub server_port: u16,
    /// TLS server name, or the HTTP Host header.
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
    pub tls_version: Option<String>,
    pub method: Option<String>,
    pub path: Option<String>,
    pub user_agent: Option<String>,
    pub banner: Option<String>,
}

/// Traffic totals for one workload (VM, or interface when unattributed).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AccountingRecord {
    pub vm: Option<String>,
    pub interfaces: Vec<String>,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_pkts: u64,
    pub rx_pkts: u64,
    pub drops: u64,
    /// RFC 3339 start of the accounting window (persisted across restarts).
    pub since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcRecord {
    pub ts: String,
    /// exec | exit | fork | file_open | connect | cap_denied
    pub kind: String,
    pub pid: u32,
    pub tgid: u32,
    pub ppid: Option<u32>,
    pub child_pid: Option<u32>,
    pub uid: u32,
    pub gid: u32,
    pub comm: String,
    pub path: Option<String>,
    pub cmdline: Option<String>,
    pub cgroup_id: u64,
    pub cgroup: Option<String>,
    pub unit: Option<String>,
    pub vm: Option<String>,
    pub container: Option<String>,
    pub denied: bool,
    pub killed: bool,
    pub policy_id: Option<String>,
    pub open_flags: Option<u32>,
    pub capability: Option<String>,
    pub proto: Option<String>,
    pub saddr: Option<String>,
    pub sport: Option<u16>,
    pub daddr: Option<String>,
    pub dport: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Anomaly {
    pub id: String,
    pub ts: String,
    /// port_scan | inbound_scan | beaconing | egress_volume_spike |
    /// new_destination | policy_violation_burst | suspicious_exec | dns_tunneling
    pub kind: String,
    /// low | medium | high | critical
    pub severity: String,
    pub summary: String,
    pub vm: Option<String>,
    pub iface: Option<String>,
    pub local: Option<String>,
    pub remote: Option<String>,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DropReason {
    pub reason: u32,
    pub name: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcpHealth {
    /// retransmit | rst_sent | rst_recv
    pub kind: String,
    pub addr: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetHealth {
    pub drop_reasons: Vec<DropReason>,
    pub tcp: Vec<TcpHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptureInfo {
    pub id: String,
    pub iface: String,
    pub vm: Option<String>,
    pub started_at: String,
    pub ends_at: String,
    pub packets: u64,
    pub bytes: u64,
    pub max_packets: usize,
    pub sample: u32,
    pub snaplen: u32,
    pub done: bool,
}

// ---- machina-cni ------------------------------------------------------------

/// A local pod: its IPv4 address and the host side of its veth pair.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CniEndpoint {
    pub ip: String,
    pub host_iface: String,
    pub pod_mac: String,
    pub host_mac: String,
    #[serde(default)]
    pub pod: Option<String>,
}

/// One allowed (subject, peer, direction, proto, port) tuple; peer 0 = any,
/// proto 0 = any, port 0 = any.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CniPolicyEntry {
    pub subject: u32,
    pub peer: u32,
    pub egress: bool,
    pub proto: u8,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CniBackend {
    pub addr: String,
    pub port: u16,
    /// Backend runs on another node (NodePort reaches it by SNAT or DSR).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remote: bool,
    /// Address of the node hosting a remote backend (DSR encap target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}

/// A service frontend. `addr` "0.0.0.0" / "::" = NodePort on this node's
/// IPv4 / IPv6 address.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CniService {
    pub addr: String,
    pub port: u16,
    pub proto: u8,
    pub backends: Vec<CniBackend>,
    #[serde(default)]
    pub name: Option<String>,
    /// `sessionAffinity: ClientIP` timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affinity_secs: Option<u32>,
}

/// Node-level datapath settings for `cni_configure`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CniNodeConfig {
    /// Node primary IPv4 address (NodePort frontend).
    pub node_addr: String,
    #[serde(default)]
    pub node_addr6: Option<String>,
    /// Interface that gets the NodePort classifier (none = no NodePort).
    #[serde(default)]
    pub uplink: Option<String>,
    /// How NodePort reaches remote backends: "snat" (default) or "dsr".
    #[serde(default)]
    pub lb_mode: Option<String>,
    /// Also accelerate NodePort → local backend in XDP on the uplink.
    #[serde(default)]
    pub xdp: bool,
}

/// Identity / isolation for one pod IP (cluster-wide).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CniIdentity {
    pub ip: String,
    pub identity: u32,
    #[serde(default)]
    pub ingress_isolated: bool,
    #[serde(default)]
    pub egress_isolated: bool,
}

/// The CNI state ABI this build speaks (see [`CniState::version`]).
pub const CNI_STATE_VERSION: u32 = machina_bpf_common::CNI_ABI_VERSION;

/// Full desired CNI state; each sync replaces the previous one.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CniState {
    /// Must equal [`CNI_STATE_VERSION`]; bpfd rejects other agents.
    #[serde(default)]
    pub version: u32,
    pub identities: Vec<CniIdentity>,
    pub policy: Vec<CniPolicyEntry>,
    /// NetworkPolicy ipBlock CIDRs → identity.
    pub cidrs: Vec<(String, u32)>,
    pub services: Vec<CniService>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CniStatus {
    pub configured: bool,
    pub version: u32,
    pub node_addr: Option<String>,
    #[serde(default)]
    pub node_addr6: Option<String>,
    pub uplink: Option<String>,
    #[serde(default)]
    pub lb_mode: String,
    #[serde(default)]
    pub xdp: bool,
    /// Services with a Maglev table (two or more backends).
    #[serde(default)]
    pub maglev_services: usize,
    pub endpoints: Vec<CniEndpoint>,
    pub identities: usize,
    pub policy_entries: usize,
    pub services: usize,
    pub last_sync: Option<String>,
}

/// One VM at the edge. `group` (from controller VM labels) picks the policy
/// identity; VMs without a group get their own (`vm:<name>`).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VmEdgeVm {
    pub name: String,
    #[serde(default)]
    pub group: Option<String>,
    /// Guest addresses, so other VMs can match this one as a peer.
    #[serde(default)]
    pub addresses: Vec<String>,
    /// Tap names; empty = discover from libvirt's live domain XML.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taps: Vec<String>,
    #[serde(default)]
    pub isolate_ingress: bool,
    #[serde(default)]
    pub isolate_egress: bool,
    /// Mbit/s from / towards the VM; 0 = unlimited.
    #[serde(default)]
    pub egress_mbps: u32,
    #[serde(default)]
    pub ingress_mbps: u32,
    /// Packets/s each direction; 0 = unlimited.
    #[serde(default)]
    pub pps: u32,
}

/// Allow rule between groups. `peer` None = any peer (including non-VMs),
/// proto 0 = any, port 0 = any.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct VmEdgeRule {
    pub group: String,
    #[serde(default)]
    pub peer: Option<String>,
    #[serde(default)]
    pub egress: bool,
    #[serde(default)]
    pub proto: u8,
    #[serde(default)]
    pub port: u16,
}

/// Desired VM edge state; each `vm_edge_sync` replaces the previous one.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VmEdgeState {
    pub vms: Vec<VmEdgeVm>,
    #[serde(default)]
    pub policy: Vec<VmEdgeRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmEdgeTap {
    pub vm: String,
    pub iface: String,
    pub identity: u32,
    pub flags: Vec<String>,
    pub stats: VmEdgeCounters,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct VmEdgeCounters {
    pub out_pkts: u64,
    pub out_bytes: u64,
    pub in_pkts: u64,
    pub in_bytes: u64,
    pub denied: u64,
    pub observed: u64,
    pub rate_dropped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmEdgeStatus {
    pub vms: usize,
    pub rules: usize,
    pub groups: BTreeMap<String, u32>,
    pub taps: Vec<VmEdgeTap>,
    /// VMs in the state with no tap on this host.
    pub missing: Vec<String>,
    pub enforcing: bool,
}

/// QEMU sandbox settings (device allowlist + egress ports). Enforcement
/// also needs the bpfd enforcement lease.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VmSandboxConfig {
    /// "observe" (default) or "enforce".
    #[serde(default = "default_observe")]
    pub mode: String,
    /// Attach to every running machine-qemu scope automatically.
    #[serde(default)]
    pub auto: bool,
    /// Extra device rules, `c|b MAJOR:MINOR|* [rwm]`.
    #[serde(default)]
    pub extra_devices: Vec<String>,
    /// QEMU egress ports besides loopback (live migration, NBD).
    #[serde(default = "default_qemu_ports")]
    pub egress_ports: Vec<String>,
}

fn default_observe() -> String {
    "observe".into()
}

fn default_qemu_ports() -> Vec<String> {
    vec!["49152-49215".into(), "10809".into()]
}

impl Default for VmSandboxConfig {
    fn default() -> Self {
        Self { mode: default_observe(), auto: false, extra_devices: Vec::new(), egress_ports: default_qemu_ports() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxHit {
    pub vm: Option<String>,
    pub cgroup: Option<String>,
    /// "c 10:232 rw" for devices, "tcp 10.0.0.1:443" for egress.
    pub target: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmSandboxStatus {
    pub config: VmSandboxConfig,
    /// The resolved device allowlist.
    pub devices: Vec<String>,
    /// VM → sandboxed cgroup path.
    pub attached: BTreeMap<String, String>,
    pub enforcing: bool,
    pub device_hits: Vec<SandboxHit>,
    pub egress_hits: Vec<SandboxHit>,
    pub notes: Vec<String>,
}

/// XDP DDoS shield on the uplink (shares the `mn_xdp_uplink` dispatcher
/// with the NodePort fast path, so both must use the same interface).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShieldConfig {
    #[serde(default)]
    pub iface: String,
    /// `off`, `audit` or `enforce` (drops need the enforcement lease).
    #[serde(default = "default_shield_mode")]
    pub mode: String,
    /// Protect every destination instead of `protected`.
    #[serde(default)]
    pub protect_all: bool,
    /// Protected destination addresses.
    #[serde(default)]
    pub protected: Vec<String>,
    /// Per-source packets/s per class; 0 = unlimited.
    #[serde(default = "default_syn_pps")]
    pub syn_pps: u32,
    #[serde(default = "default_udp_pps")]
    pub udp_pps: u32,
    #[serde(default = "default_icmp_pps")]
    pub icmp_pps: u32,
    #[serde(default)]
    pub other_pps: u32,
    #[serde(default = "default_burst_secs")]
    pub burst_secs: u32,
    /// Source CIDRs never limited / always dropped.
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

fn default_shield_mode() -> String {
    "off".into()
}
fn default_syn_pps() -> u32 {
    1000
}
fn default_udp_pps() -> u32 {
    5000
}
fn default_icmp_pps() -> u32 {
    100
}
fn default_burst_secs() -> u32 {
    2
}

impl Default for ShieldConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("shield defaults")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShieldCounters {
    pub checked: u64,
    pub passed: u64,
    pub audited: u64,
    pub dropped: u64,
    pub dropped_bytes: u64,
    pub denied: u64,
    pub malformed: u64,
    pub syn_limited: u64,
    pub udp_limited: u64,
    pub icmp_limited: u64,
    pub other_limited: u64,
}

/// A source over its rate (top talkers by hits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldSource {
    pub addr: String,
    pub class: String,
    pub hits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShieldStatus {
    pub config: ShieldConfig,
    /// Interface carrying the dispatcher with the shield on.
    pub attached: Option<String>,
    pub enforcing: bool,
    pub stats: ShieldCounters,
    pub sources: Vec<ShieldSource>,
    pub tracked_sources: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Status,
    ListPolicies,
    ApplyPolicy {
        policy: Policy,
    },
    RemovePolicy {
        id: String,
    },
    SetMode {
        mode: Mode,
        #[serde(default)]
        lease_secs: Option<u64>,
    },
    ListInterfaces,
    AttachInterface {
        name: String,
        #[serde(default)]
        guest_side: bool,
        #[serde(default)]
        xdp: bool,
    },
    DetachInterface {
        name: String,
    },
    Flows {
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        vm: Option<String>,
    },
    Events {
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        kind: Option<String>,
    },
    Dns {
        #[serde(default)]
        limit: Option<usize>,
    },
    L7 {
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        vm: Option<String>,
        /// tls | http | ssh
        #[serde(default)]
        protocol: Option<String>,
    },
    /// Per-VM traffic totals since `since` (or since the last reset).
    Accounting {
        #[serde(default)]
        vm: Option<String>,
    },
    ResetAccounting {
        #[serde(default)]
        vm: Option<String>,
    },
    ProcEvents {
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        kind: Option<String>,
    },
    Anomalies {
        #[serde(default)]
        limit: Option<usize>,
    },
    NetHealth,
    CaptureStart {
        iface: String,
        #[serde(default)]
        duration_secs: Option<u64>,
        #[serde(default)]
        sample: Option<u32>,
        #[serde(default)]
        snaplen: Option<u32>,
        #[serde(default)]
        max_packets: Option<usize>,
    },
    CaptureList,
    CaptureGet {
        id: String,
    },
    SetQos {
        #[serde(default)]
        iface: Option<String>,
        #[serde(default)]
        vm: Option<String>,
        /// Bits/s sent by the workload; 0 = unlimited.
        #[serde(default)]
        egress_bps: u64,
        /// Bits/s towards the workload; 0 = unlimited.
        #[serde(default)]
        ingress_bps: u64,
    },
    GetTelemetry,
    SetTelemetry {
        telemetry: TelemetryConfig,
    },
    /// Node address (NodePort matching) and the uplink that gets the NodePort
    /// classifier; also attaches socket-level service load balancing.
    CniConfigure {
        #[serde(flatten)]
        config: CniNodeConfig,
    },
    CniAddEndpoint {
        endpoint: CniEndpoint,
    },
    CniDelEndpoint {
        ip: String,
    },
    CniSync {
        state: CniState,
    },
    CniStatus,
    /// VM group identities, policy and rate limits on VM taps.
    VmEdgeSync {
        state: VmEdgeState,
    },
    VmEdgeStatus,
    VmSandboxConfigure {
        config: VmSandboxConfig,
    },
    /// Sandbox one VM's QEMU (`cgroup` relative to /sys/fs/cgroup; default:
    /// its machine-qemu scope).
    VmSandboxAttach {
        vm: String,
        #[serde(default)]
        cgroup: Option<String>,
    },
    VmSandboxDetach {
        vm: String,
    },
    VmSandboxStatus,
    /// Re-follow VM taps and QEMU scopes now (sent on VM start/stop).
    VmRefresh,
    ShieldConfigure {
        config: ShieldConfig,
    },
    ShieldStatus,
    /// Stream events (`net`, `dns`, `l7`, `proc`, `anomaly`) as JSON lines
    /// until the client disconnects.
    Subscribe {
        topics: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub data: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(data: impl Serialize) -> Self {
        Self {
            ok: true,
            data: serde_json::to_value(data).unwrap_or(Value::Null),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(msg.into()),
        }
    }
}

/// One streamed event on a `Subscribe` connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub topic: String,
    pub event: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_format() {
        let r: Request = serde_json::from_str(
            r#"{"op":"apply_policy","policy":{"id":"p1","kind":"deny_ip","match":"10.0.0.0/8","scope":{"kind":"vm","name":"web"}}}"#,
        )
        .unwrap();
        match r {
            Request::ApplyPolicy { policy } => {
                assert!(policy.enabled);
                assert_eq!(policy.scope, Scope::Vm { name: "web".into() });
                assert_eq!(policy.scope.key().as_deref(), Some("vm:web"));
            }
            other => panic!("unexpected {other:?}"),
        }
        let s = serde_json::to_string(&Request::SetMode {
            mode: Mode::Enforce,
            lease_secs: Some(60),
        })
        .unwrap();
        assert_eq!(s, r#"{"op":"set_mode","mode":"enforce","lease_secs":60}"#);
        let r: Request = serde_json::from_str(r#"{"op":"l7","protocol":"tls"}"#).unwrap();
        assert!(matches!(r, Request::L7 { protocol: Some(ref p), .. } if p == "tls"));
        let r: Request = serde_json::from_str(r#"{"op":"accounting"}"#).unwrap();
        assert!(matches!(r, Request::Accounting { vm: None }));
    }

    #[test]
    fn scope_defaults_to_host() {
        let p: Policy =
            serde_json::from_str(r#"{"id":"x","kind":"deny_process","match":"/usr/bin/nc"}"#).unwrap();
        assert_eq!(p.scope, Scope::Host);
        assert_eq!(p.scope.label(), "host");
    }
}

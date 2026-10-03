// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Wire types for the machina-bpfd Unix-socket API (newline-delimited JSON).

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
    /// Stream events (`net`, `flow`, `dns`, `proc`, `anomaly`) as JSON lines
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
    }

    #[test]
    fn scope_defaults_to_host() {
        let p: Policy =
            serde_json::from_str(r#"{"id":"x","kind":"deny_process","match":"/usr/bin/nc"}"#).unwrap();
        assert_eq!(p.scope, Scope::Host);
        assert_eq!(p.scope.label(), "host");
    }
}

// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Policy trace (`cilium policy trace` for VMs): evaluate one connection
//! against the compiled fleet state with the datapath's lookup order —
//! egress at the source VM, then ingress at the destination VM.

use std::collections::HashMap;

use machina_bpf_common::{IDENTITY_HOST, IDENTITY_WORLD};
use serde::{Deserialize, Serialize};

use super::compile::{compile, Inputs, NetpolVm, IDENTITY_REMOTE_NODE};
use super::VmNetworkPolicy;
use crate::api::VmEdgeState;
use crate::policy::parse_prefix;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraceQuery {
    /// VM name, IP address, `world`, `host` or `remote-node`.
    pub from: String,
    pub to: String,
    /// TCP (default), UDP, SCTP, ICMP, ICMPv6 or ANY.
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub port: u16,
    /// For ICMP / ICMPv6.
    #[serde(default)]
    pub icmp_type: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TraceEndpoint {
    pub input: String,
    #[serde(default)]
    pub vm: Option<String>,
    pub identity: u32,
    /// `vm`, `host`, `remote-node`, `cidr` or `world`.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TraceSide {
    /// `egress` (at the source VM) or `ingress` (at the destination VM).
    pub direction: String,
    #[serde(default)]
    pub vm: Option<String>,
    /// The VM is default-deny in this direction.
    pub enforced: bool,
    /// `allowed`, `denied`, `default-deny`, `no-policy` or `not-a-vm`.
    pub verdict: String,
    #[serde(default)]
    pub rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TraceResult {
    pub allowed: bool,
    pub from: TraceEndpoint,
    pub to: TraceEndpoint,
    pub protocol: String,
    pub port: u16,
    pub egress: TraceSide,
    pub ingress: TraceSide,
    pub summary: String,
}

fn proto_num(p: &str) -> Result<u8, String> {
    match p.to_ascii_uppercase().as_str() {
        "" | "TCP" => Ok(6),
        "UDP" => Ok(17),
        "SCTP" => Ok(132),
        "ICMP" => Ok(1),
        "ICMPV6" => Ok(58),
        "ANY" => Ok(0),
        o => Err(format!("unknown protocol `{o}`")),
    }
}

struct Table {
    /// (subject, peer, egress, proto, port) → (deny, source)
    map: super::RuleIndex,
}

impl Table {
    fn new(state: &VmEdgeState) -> Self {
        let mut map = HashMap::new();
        for r in &state.policy {
            let (Some(s), Some(p)) = (r.subject_identity, r.peer_identity) else { continue };
            let end = if r.port_end > r.port { r.port_end } else { r.port };
            for port in r.port..=end {
                let e = map.entry((s, p, r.egress, r.proto, port)).or_insert((r.deny, r.source.clone().unwrap_or_default()));
                if r.deny && !e.0 {
                    *e = (true, r.source.clone().unwrap_or_default());
                }
            }
        }
        Table { map }
    }

    /// Same wildcard order as the kernel; any deny match wins.
    fn eval(&self, subject: u32, peer: u32, egress: bool, proto: u8, port: u16) -> Option<(bool, String)> {
        let combos = [(peer, proto, port), (peer, proto, 0), (0, proto, port), (0, proto, 0), (peer, 0, 0), (0, 0, 0)];
        let hits: Vec<&(bool, String)> =
            combos.iter().filter_map(|(pe, pr, po)| self.map.get(&(subject, *pe, egress, *pr, *po))).collect();
        hits.iter().find(|h| h.0).or_else(|| hits.first()).map(|h| (*h).clone())
    }
}

fn resolve(input: &str, vms: &[NetpolVm], state: &VmEdgeState, compiled_ids: &HashMap<String, u32>) -> TraceEndpoint {
    let ep = |vm: Option<String>, identity, kind: &str| TraceEndpoint { input: input.into(), vm, identity, kind: kind.into() };
    if let Some(id) = compiled_ids.get(input) {
        return ep(Some(input.into()), *id, "vm");
    }
    match input {
        "world" => return ep(None, IDENTITY_WORLD, "world"),
        "host" => return ep(None, IDENTITY_HOST, "host"),
        "remote-node" => return ep(None, IDENTITY_REMOTE_NODE, "remote-node"),
        _ => {}
    }
    let Ok(addr) = parse_prefix(input) else { return ep(None, IDENTITY_WORLD, "world") };
    if let Some(vm) = vms.iter().find(|v| v.addresses.iter().any(|a| parse_prefix(a).ok() == Some(addr))) {
        return ep(Some(vm.name.clone()), compiled_ids.get(&vm.name).copied().unwrap_or(0), "vm");
    }
    let mut best: Option<(u32, u32)> = None;
    for p in &state.peers {
        let Ok(pp) = parse_prefix(&p.cidr) else { continue };
        let inside = addr.bits >= pp.bits && {
            let mut a = addr.addr;
            for (i, b) in a.iter_mut().enumerate() {
                let start = i as u32 * 8;
                if start >= pp.bits {
                    *b = 0;
                } else if start + 8 > pp.bits {
                    *b &= 0xffu8 << (8 - (pp.bits - start));
                }
            }
            a == pp.addr
        };
        if inside && best.is_none_or(|(bits, _)| pp.bits > bits) {
            best = Some((pp.bits, p.identity));
        }
    }
    match best {
        Some((_, IDENTITY_HOST)) => ep(None, IDENTITY_HOST, "host"),
        Some((_, IDENTITY_REMOTE_NODE)) => ep(None, IDENTITY_REMOTE_NODE, "remote-node"),
        Some((_, id)) => ep(None, id, "cidr"),
        None => ep(None, IDENTITY_WORLD, "world"),
    }
}

/// Trace one connection across the fleet.
pub fn trace(
    policies: &[VmNetworkPolicy],
    vms: &[NetpolVm],
    host_addresses: &[String],
    remote_node_addresses: &[String],
    q: &TraceQuery,
) -> Result<TraceResult, String> {
    let proto = proto_num(&q.protocol)?;
    let port = if proto == 1 || proto == 58 { q.icmp_type.map_or(0, |t| t as u16 + 1) } else { q.port };
    let c = compile(&Inputs { policies, vms, host: None, host_addresses, remote_node_addresses });
    let ids: HashMap<String, u32> = c.endpoints.iter().map(|e| (e.name.clone(), e.identity)).collect();
    let iso: HashMap<u32, (bool, bool)> =
        c.endpoints.iter().map(|e| (e.identity, (e.ingress_enforced, e.egress_enforced))).collect();
    let from = resolve(&q.from, vms, &c.state, &ids);
    let to = resolve(&q.to, vms, &c.state, &ids);
    let table = Table::new(&c.state);
    let side = |subject: &TraceEndpoint, peer: &TraceEndpoint, egress: bool| -> TraceSide {
        let direction = if egress { "egress" } else { "ingress" }.to_string();
        if subject.kind != "vm" {
            return TraceSide { direction, verdict: "not-a-vm".into(), ..Default::default() };
        }
        let enforced = iso.get(&subject.identity).is_some_and(|(i, e)| if egress { *e } else { *i });
        let (verdict, rule) = match table.eval(subject.identity, peer.identity, egress, proto, port) {
            Some((true, src)) => ("denied", Some(src)),
            Some((false, src)) => ("allowed", Some(src)),
            None if enforced => ("default-deny", None),
            None => ("no-policy", None),
        };
        TraceSide { direction, vm: subject.vm.clone(), enforced, verdict: verdict.into(), rule }
    };
    let egress = side(&from, &to, true);
    let ingress = side(&to, &from, false);
    let pass = |s: &TraceSide| matches!(s.verdict.as_str(), "allowed" | "no-policy" | "not-a-vm");
    let allowed = pass(&egress) && pass(&ingress);
    let why = |s: &TraceSide| match (&s.verdict[..], &s.rule) {
        ("denied", Some(r)) => format!("denied at {} by {r}", s.direction),
        ("default-deny", _) => format!("no {} rule allows it (default deny)", s.direction),
        _ => String::new(),
    };
    let summary = if allowed {
        "ALLOWED".to_string()
    } else {
        let reasons: Vec<String> = [why(&egress), why(&ingress)].into_iter().filter(|s| !s.is_empty()).collect();
        format!("DENIED: {}", reasons.join("; "))
    };
    Ok(TraceResult {
        allowed,
        from,
        to,
        protocol: if q.protocol.is_empty() { "TCP".into() } else { q.protocol.to_ascii_uppercase() },
        port: q.port,
        egress,
        ingress,
        summary,
    })
}

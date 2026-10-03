// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! VM edge (`mn_vm_edge_in/out` on VM taps) and the QEMU sandbox
//! (`mn_qemu_device` + `mn_qemu_egress` on machine-qemu scopes).
//!
//! Both follow the VMs: the periodic rescan programs taps of VMs in the
//! synced edge state as they appear, and (with `auto`) sandboxes every
//! running QEMU scope. Policy misses and sandbox violations only drop with
//! the enforcement lease live; otherwise they are counted.

use std::collections::{BTreeMap, HashSet};
use std::os::unix::fs::MetadataExt;

use super::cni::addr16;
use super::*;

const EDGE_IN: &str = "mn_vm_edge_in";
const EDGE_OUT: &str = "mn_vm_edge_out";
const MACHINE_SLICE: &str = "machine.slice";

#[derive(Default)]
pub(super) struct VmEdgeRuntime {
    pub state: VmEdgeState,
    groups: BTreeMap<String, u32>,
    /// tap name → (ifindex, vm)
    taps: HashMap<String, (u32, String)>,
    ips: HashSet<[u8; ADDR_LEN]>,
    policy: HashMap<PolicyKey, u32>,
    /// Identities with deny entries: (identity, egress).
    deny: HashSet<(u32, bool)>,
}

/// What the flow reader needs to label events: identity → name/labels and
/// the rule table for attribution.
#[derive(Default, Clone)]
pub(super) struct FlowIndex {
    names: HashMap<u32, (String, BTreeMap<String, String>)>,
    rules: crate::netpol::RuleIndex,
}

impl FlowIndex {
    fn name(&self, id: u32) -> Option<&(String, BTreeMap<String, String>)> {
        self.names.get(&id)
    }

    /// Same lookup order as the kernel; any deny match wins.
    fn attribute(&self, subject: u32, peer: u32, egress: bool, proto: u8, port: u16) -> Option<String> {
        let combos = [(peer, proto, port), (peer, proto, 0), (0, proto, port), (0, proto, 0), (peer, 0, 0), (0, 0, 0)];
        let hits: Vec<&(bool, String)> =
            combos.iter().filter_map(|(pe, pr, po)| self.rules.get(&(subject, *pe, egress, *pr, *po))).collect();
        hits.iter().find(|h| h.0).or_else(|| hits.first()).map(|h| h.1.clone()).filter(|s| !s.is_empty())
    }
}

fn tcp_flag_names(f: u8) -> String {
    let mut out = Vec::new();
    for (bit, n) in [(0x02, "SYN"), (0x10, "ACK"), (0x01, "FIN"), (0x04, "RST"), (0x08, "PSH"), (0x20, "URG")] {
        if f & bit != 0 {
            out.push(n);
        }
    }
    out.join(",")
}

pub(super) fn on_vm_flow(sh: &SharedState, bus: &broadcast::Sender<StreamEvent>, b: &[u8]) {
    if b.len() < std::mem::size_of::<VmFlowEvent>() {
        return;
    }
    let ev = unsafe { std::ptr::read_unaligned(b.as_ptr() as *const VmFlowEvent) };
    let rec = {
        let mut s = lock(sh);
        let (iface, tap_vm) = s.iface(ev.ifindex);
        let idx = &s.vm_flow_index;
        let egress = ev.from_vm != 0;
        let subject = idx.name(ev.subject).cloned();
        let peer = idx.name(ev.peer).cloned();
        let port = if ev.icmp != 0 { ev.icmp as u16 } else { ev.dport };
        let policy = idx.attribute(ev.subject, ev.peer, egress, ev.proto, port);
        let vm = tap_vm.or_else(|| subject.as_ref().map(|s| s.0.clone())).unwrap_or_default();
        let (src_side, dst_side) = if egress { (subject, peer) } else { (peer, subject) };
        let (src_id, dst_id) = if egress { (ev.subject, ev.peer) } else { (ev.peer, ev.subject) };
        let rec = VmFlowRecord {
            ts: mono_to_rfc3339(ev.ts_ns),
            host: None,
            iface: iface.unwrap_or_default(),
            vm,
            direction: if egress { "egress" } else { "ingress" }.into(),
            src: fmt_addr(&ev.src),
            src_port: if ev.icmp != 0 { 0 } else { ev.sport },
            dst: fmt_addr(&ev.dst),
            dst_port: if ev.icmp != 0 { 0 } else { ev.dport },
            src_vm: src_side.as_ref().map(|s| s.0.clone()),
            dst_vm: dst_side.as_ref().map(|s| s.0.clone()),
            src_labels: src_side.map(|s| s.1).unwrap_or_default(),
            dst_labels: dst_side.map(|s| s.1).unwrap_or_default(),
            src_identity: src_id,
            dst_identity: dst_id,
            proto: match ev.proto {
                132 => "sctp".into(),
                p => proto_name(p).into(),
            },
            tcp_flags: if ev.proto == policy::IPPROTO_TCP { tcp_flag_names(ev.tcp_flags) } else { String::new() },
            icmp_type: (ev.icmp != 0).then(|| ev.icmp - 1),
            bytes: ev.len,
            verdict: match ev.verdict {
                VMF_DROPPED => "DROPPED",
                VMF_AUDIT => "AUDIT",
                _ => "FORWARDED",
            }
            .into(),
            drop_reason: match ev.reason {
                VMF_REASON_POLICY_DENY => Some("policy-deny".into()),
                VMF_REASON_DEFAULT_DENY => Some("default-deny".into()),
                _ => None,
            },
            policy,
        };
        Shared::push_capped(&mut s.vm_flows, rec.clone(), VM_FLOW_STORE_CAP);
        rec
    };
    publish(bus, "flow", &rec);
}

#[derive(Default)]
pub(super) struct SandboxRuntime {
    pub config: VmSandboxConfig,
    /// VM → cgroup path relative to the cgroup root.
    attached: BTreeMap<String, String>,
    /// VMs attached by request (kept across restarts, re-attached on start).
    pub pinned: HashSet<String>,
    devices: Vec<String>,
    ports: HashSet<u16>,
    notes: Vec<String>,
    configured: bool,
}

/// Group (or per-VM) identity; never one of the reserved CNI identities.
fn group_identity(name: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    if h < 16 { h + 16 } else { h }
}

fn vm_group(vm: &VmEdgeVm) -> String {
    vm.group.clone().unwrap_or_else(|| format!("vm:{}", vm.name))
}

fn vm_ident(vm: &VmEdgeVm) -> u32 {
    vm.identity.unwrap_or_else(|| group_identity(&vm_group(vm)))
}

fn mbps_to_bytes(mbps: u32) -> u64 {
    mbps as u64 * 1_000_000 / 8
}

fn edge_flag_names(f: u32) -> Vec<String> {
    let mut v = Vec::new();
    if f & VME_ISOLATE_IN != 0 {
        v.push("isolate_ingress".to_string());
    }
    if f & VME_ISOLATE_OUT != 0 {
        v.push("isolate_egress".to_string());
    }
    if f & VME_GUEST_SIDE != 0 {
        v.push("guest_side".to_string());
    }
    if f & VME_DENY_IN != 0 {
        v.push("deny_ingress".to_string());
    }
    if f & VME_DENY_OUT != 0 {
        v.push("deny_egress".to_string());
    }
    if f & VME_FLOW_LOG != 0 {
        v.push("flow_log".to_string());
    }
    v
}

/// `c 10:232 rwm`, `b 8:* r`, `c 136:* rw` → device rule.
pub(crate) fn parse_dev_rule(s: &str) -> Result<QemuDevRule> {
    let mut it = s.split_whitespace();
    let ty = match it.next() {
        Some("c") => DEVCG_DEV_CHAR,
        Some("b") => DEVCG_DEV_BLOCK,
        _ => return Err(anyhow!("device rule `{s}`: expected `c` or `b`")),
    };
    let (maj, min) = it
        .next()
        .and_then(|mm| mm.split_once(':'))
        .ok_or_else(|| anyhow!("device rule `{s}`: expected MAJOR:MINOR"))?;
    let major: u32 = maj.parse().map_err(|_| anyhow!("device rule `{s}`: bad major"))?;
    let minor = if min == "*" { DEV_MINOR_ANY } else { min.parse().map_err(|_| anyhow!("device rule `{s}`: bad minor"))? };
    let mut access = 0;
    for c in it.next().unwrap_or("rwm").chars() {
        access |= match c {
            'r' => DEVCG_ACC_READ,
            'w' => DEVCG_ACC_WRITE,
            'm' => DEVCG_ACC_MKNOD,
            _ => return Err(anyhow!("device rule `{s}`: access is a subset of rwm")),
        };
    }
    Ok(QemuDevRule { major, minor, dev_type: ty, access })
}

pub(super) fn dev_rule_string(r: &QemuDevRule) -> String {
    let ty = if r.dev_type == DEVCG_DEV_BLOCK { 'b' } else { 'c' };
    let minor = if r.minor == DEV_MINOR_ANY { "*".to_string() } else { r.minor.to_string() };
    let mut acc = String::new();
    for (bit, c) in [(DEVCG_ACC_READ, 'r'), (DEVCG_ACC_WRITE, 'w'), (DEVCG_ACC_MKNOD, 'm')] {
        if r.access & bit != 0 {
            acc.push(c);
        }
    }
    format!("{ty} {}:{minor} {acc}", r.major)
}

/// `49152-49215`, `10809` → ports.
pub(crate) fn parse_ports(specs: &[String]) -> Result<HashSet<u16>> {
    let mut out = HashSet::new();
    for s in specs {
        let (a, b) = s.split_once('-').unwrap_or((s, s));
        let (a, b): (u16, u16) = (
            a.trim().parse().map_err(|_| anyhow!("bad port `{s}`"))?,
            b.trim().parse().map_err(|_| anyhow!("bad port `{s}`"))?,
        );
        if a == 0 || b < a || (b - a) > 1024 {
            return Err(anyhow!("bad port range `{s}`"));
        }
        out.extend(a..=b);
    }
    if out.len() > 1024 {
        return Err(anyhow!("at most 1024 egress ports"));
    }
    Ok(out)
}

/// Char device numbers of the nodes QEMU needs (libvirt's default ACL plus
/// vhost/tun/vfio), resolved on this host; absent nodes are skipped.
pub(super) fn default_dev_rules() -> Vec<QemuDevRule> {
    let mut out = Vec::new();
    for node in [
        "/dev/null", "/dev/zero", "/dev/full", "/dev/random", "/dev/urandom", "/dev/ptmx", "/dev/kvm",
        "/dev/vhost-net", "/dev/vhost-vsock", "/dev/net/tun", "/dev/vfio/vfio", "/dev/sev", "/dev/userfaultfd",
    ] {
        if let Ok(m) = std::fs::metadata(node) {
            let rdev = m.rdev();
            let major = (((rdev >> 32) & 0xffff_f000) | ((rdev >> 8) & 0xfff)) as u32;
            let minor = (((rdev >> 12) & 0xffff_ff00) | (rdev & 0xff)) as u32;
            out.push(QemuDevRule { major, minor, dev_type: DEVCG_DEV_CHAR, access: DEVCG_ACC_READ | DEVCG_ACC_WRITE });
        }
    }
    // Pseudo-terminals (serial consoles) and VFIO group nodes.
    out.push(QemuDevRule { major: 136, minor: DEV_MINOR_ANY, dev_type: DEVCG_DEV_CHAR, access: DEVCG_ACC_READ | DEVCG_ACC_WRITE });
    if let Some(m) = char_major("vfio") {
        out.push(QemuDevRule { major: m, minor: DEV_MINOR_ANY, dev_type: DEVCG_DEV_CHAR, access: DEVCG_ACC_READ | DEVCG_ACC_WRITE });
    }
    out
}

fn char_major(name: &str) -> Option<u32> {
    let s = std::fs::read_to_string("/proc/devices").ok()?;
    let chars = s.split("Block devices:").next()?;
    chars.lines().find_map(|l| {
        let (num, n) = l.trim().split_once(' ')?;
        (n.trim() == name).then(|| num.parse().ok()).flatten()
    })
}

/// Running QEMU scopes: VM name → cgroup path relative to the cgroup root.
pub(super) fn qemu_scopes() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(Path::new(attribution::CGROUP_ROOT).join(MACHINE_SLICE)) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("machine-qemu") {
            continue;
        }
        let rel = format!("{MACHINE_SLICE}/{name}");
        if let Some(vm) = attribution::classify_cgroup(&rel).vm {
            out.insert(vm, rel);
        }
    }
    out
}

impl Engine {
    pub(super) fn lease_live(&self) -> bool {
        self.mode == Mode::Enforce && loader::monotonic_ns() < self.lease_deadline_mono
    }

    // ---- VM edge -------------------------------------------------------------

    pub(super) fn vm_edge_sync(&mut self, state: VmEdgeState) -> Result<VmEdgeStatus> {
        let mut names = HashSet::new();
        for vm in &state.vms {
            if vm.name.is_empty() || !names.insert(vm.name.as_str()) {
                return Err(anyhow!("vm_edge_sync: VM names must be unique and non-empty"));
            }
        }
        let mut groups: BTreeMap<String, u32> = BTreeMap::new();
        let mut ips: HashMap<[u8; ADDR_LEN], u32> = HashMap::new();
        let mut index = FlowIndex::default();
        for (id, n) in [(IDENTITY_HOST, "host"), (IDENTITY_WORLD, "world"), (crate::netpol::IDENTITY_REMOTE_NODE, "remote-node")] {
            index.names.insert(id, (n.to_string(), BTreeMap::new()));
        }
        for vm in &state.vms {
            let id = vm_ident(vm);
            groups.insert(vm.identity.map_or_else(|| vm_group(vm), |_| format!("vm:{}", vm.name)), id);
            if let Some(g) = &vm.group {
                groups.entry(g.clone()).or_insert(id);
            }
            index.names.insert(id, (vm.name.clone(), vm.labels.clone()));
            for a in &vm.addresses {
                ips.insert(addr16(a)?, id);
            }
        }
        let mut cidrs: Vec<(Prefix, u32)> = Vec::new();
        for p in &state.peers {
            if p.cidr.contains('/') {
                let pre = policy::parse_prefix(&p.cidr).map_err(|e| anyhow!("peer {}: {e}", p.cidr))?;
                cidrs.push((pre, p.identity));
            } else {
                ips.entry(addr16(&p.cidr)?).or_insert(p.identity);
            }
            if !p.name.is_empty() {
                index.names.entry(p.identity).or_insert_with(|| (p.name.clone(), BTreeMap::new()));
            }
        }
        let mut policy: HashMap<PolicyKey, u32> = HashMap::new();
        let mut deny: HashSet<(u32, bool)> = HashSet::new();
        for r in &state.policy {
            let subject = match r.subject_identity {
                Some(id) => id,
                None => *groups.get(&r.group).ok_or_else(|| anyhow!("rule for unknown group `{}`", r.group))?,
            };
            let peer = match (r.peer_identity, r.peer.as_deref()) {
                (Some(id), _) => id,
                (None, None | Some("") | Some("any")) => 0,
                (None, Some("world")) => IDENTITY_WORLD,
                (None, Some(p)) => *groups.get(p).ok_or_else(|| anyhow!("rule peer `{p}` is not a group"))?,
            };
            let end = if r.port_end > r.port { r.port_end } else { r.port };
            if end - r.port >= crate::netpol::MAX_PORT_RANGE as u16 {
                return Err(anyhow!("rule port range {}-{end} wider than {}", r.port, crate::netpol::MAX_PORT_RANGE));
            }
            let val = if r.deny { VM_POLICY_DENY } else { VM_POLICY_ALLOW };
            if r.deny {
                deny.insert((subject, r.egress));
            }
            for port in r.port..=end {
                let k = PolicyKey {
                    subject_identity: subject,
                    peer_identity: peer,
                    direction: if r.egress { POLICY_EGRESS } else { POLICY_INGRESS },
                    proto: r.proto,
                    port: port.to_be_bytes(),
                };
                let e = policy.entry(k).or_insert(val);
                if r.deny {
                    *e = VM_POLICY_DENY;
                }
                let src = r.source.clone().unwrap_or_default();
                let ie = index.rules.entry((subject, peer, r.egress, r.proto, port)).or_insert((r.deny, src.clone()));
                if r.deny && !ie.0 {
                    *ie = (true, src);
                }
            }
        }
        if policy.len() > 131_072 {
            return Err(anyhow!("{} VM policy entries exceed the map size (131072)", policy.len()));
        }

        for k in self.vm_edge.ips.clone() {
            if !ips.contains_key(&k) {
                self.dp.cni_hash_remove::<[u8; ADDR_LEN], u32>("VM_IPS", &k);
            }
        }
        for (k, id) in &ips {
            self.dp.cni_hash_insert("VM_IPS", *k, *id)?;
        }
        self.dp.addr_lpm_clear::<u32>("VM_CIDR_IDS")?;
        for (p, id) in &cidrs {
            self.dp.addr_lpm_insert("VM_CIDR_IDS", p.addr, p.bits, *id)?;
        }
        for k in self.vm_edge.policy.keys().copied().collect::<Vec<_>>() {
            if !policy.contains_key(&k) {
                self.dp.cni_hash_remove::<PolicyKey, u32>("VM_POLICY", &k);
            }
        }
        for (k, v) in &policy {
            if self.vm_edge.policy.get(k) != Some(v) {
                self.dp.cni_hash_insert("VM_POLICY", *k, *v)?;
            }
        }
        self.vm_edge.ips = ips.into_keys().collect();
        self.vm_edge.policy = policy;
        self.vm_edge.deny = deny;
        self.vm_edge.groups = groups;
        self.vm_edge.state = state;
        lock(&self.shared).vm_flow_index = index;
        // Re-push every tap: limits or isolation may have changed.
        for (tap, (idx, _)) in std::mem::take(&mut self.vm_edge.taps) {
            self.vm_edge_unprogram(&tap, idx);
        }
        self.vm_edge_refresh();
        Ok(self.vm_edge_status())
    }

    fn vm_edge_unprogram(&mut self, tap: &str, idx: u32) {
        self.dp.detach_tc_one(tap, EDGE_IN);
        self.dp.detach_tc_one(tap, EDGE_OUT);
        self.dp.cni_hash_remove::<u32, VmEdgeCfg>("VM_EDGE", &idx);
        for k in [idx << 1, (idx << 1) | 1] {
            self.dp.cni_hash_remove::<u32, VmBucket>("VM_BUCKETS", &k);
        }
        self.dp.percpu_remove::<u32, VmEdgeStats>("VM_EDGE_STATS", &idx);
    }

    /// Program taps of VMs in the edge state; drop taps that went away.
    pub(super) fn vm_edge_refresh(&mut self) {
        if self.vm_edge.state.vms.is_empty() && self.vm_edge.taps.is_empty() {
            return;
        }
        let by_name: HashMap<&str, &VmEdgeVm> = self.vm_edge.state.vms.iter().map(|v| (v.name.as_str(), v)).collect();
        let mut taps: Vec<(String, &VmEdgeVm)> = Vec::new();
        if self.vm_edge.state.vms.iter().any(|v| v.taps.is_empty()) {
            for (tap, info) in attribution::scan_libvirt() {
                if let Some(vm) = by_name.get(info.vm.as_str()).filter(|v| v.taps.is_empty()) {
                    taps.push((tap, vm));
                }
            }
        }
        for vm in &self.vm_edge.state.vms {
            taps.extend(vm.taps.iter().map(|t| (t.clone(), vm)));
        }
        let mut want: HashMap<String, (String, VmEdgeCfg)> = HashMap::new();
        for (tap, vm) in taps {
            let guest_side = if_nametoindex(&tap)
                .and_then(|i| self.ifaces.get(&i))
                .map(|r| r.guest_side)
                .unwrap_or(true);
            let identity = vm_ident(vm);
            let mut flags = 0;
            if vm.isolate_ingress {
                flags |= VME_ISOLATE_IN;
            }
            if vm.isolate_egress {
                flags |= VME_ISOLATE_OUT;
            }
            if guest_side {
                flags |= VME_GUEST_SIDE;
            }
            if self.vm_edge.deny.contains(&(identity, false)) {
                flags |= VME_DENY_IN;
            }
            if self.vm_edge.deny.contains(&(identity, true)) {
                flags |= VME_DENY_OUT;
            }
            if self.vm_edge.state.flow_log {
                flags |= VME_FLOW_LOG;
            }
            let cfg = VmEdgeCfg {
                identity,
                flags,
                out_bps: mbps_to_bytes(vm.egress_mbps),
                in_bps: mbps_to_bytes(vm.ingress_mbps),
                pps: vm.pps,
                _pad: 0,
            };
            want.insert(tap, (vm.name.clone(), cfg));
        }
        let stale: Vec<(String, u32)> = self
            .vm_edge
            .taps
            .iter()
            .filter(|(t, (idx, _))| !want.contains_key(*t) || if_nametoindex(t) != Some(*idx))
            .map(|(t, (idx, _))| (t.clone(), *idx))
            .collect();
        for (tap, idx) in stale {
            if if_nametoindex(&tap) == Some(idx) {
                self.vm_edge_unprogram(&tap, idx);
            } else {
                self.dp.forget_tc(&tap);
                self.dp.cni_hash_remove::<u32, VmEdgeCfg>("VM_EDGE", &idx);
                self.dp.percpu_remove::<u32, VmEdgeStats>("VM_EDGE_STATS", &idx);
            }
            self.vm_edge.taps.remove(&tap);
        }
        for (tap, (vm, cfg)) in want {
            if self.vm_edge.taps.contains_key(&tap) {
                continue;
            }
            let Some(idx) = if_nametoindex(&tap) else { continue };
            let res = self
                .dp
                .cni_hash_insert("VM_EDGE", idx, cfg)
                .and_then(|_| self.dp.attach_tc_one(&tap, EDGE_IN, true))
                .and_then(|_| self.dp.attach_tc_one(&tap, EDGE_OUT, false));
            match res {
                Ok(()) => {
                    self.vm_edge.taps.insert(tap, (idx, vm));
                }
                Err(e) => {
                    tracing::warn!("vm edge on {tap}: {e:#}");
                    self.vm_edge_unprogram(&tap, idx);
                }
            }
        }
    }

    pub(super) fn vm_edge_status(&mut self) -> VmEdgeStatus {
        let stats: HashMap<u32, VmEdgeStats> = self
            .dp
            .percpu_sum::<u32, VmEdgeStats>("VM_EDGE_STATS", |a, b| {
                a.out_pkts += b.out_pkts;
                a.out_bytes += b.out_bytes;
                a.in_pkts += b.in_pkts;
                a.in_bytes += b.in_bytes;
                a.denied += b.denied;
                a.observed += b.observed;
                a.rate_dropped += b.rate_dropped;
            })
            .unwrap_or_default()
            .into_iter()
            .collect();
        let cfgs: HashMap<u32, VmEdgeCfg> =
            self.dp.hash_entries::<u32, VmEdgeCfg>("VM_EDGE").unwrap_or_default().into_iter().collect();
        let mut taps: Vec<VmEdgeTap> = self
            .vm_edge
            .taps
            .iter()
            .map(|(tap, (idx, vm))| {
                let cfg = cfgs.get(idx).copied().unwrap_or_default();
                let s = stats.get(idx).copied().unwrap_or_default();
                VmEdgeTap {
                    vm: vm.clone(),
                    iface: tap.clone(),
                    identity: cfg.identity,
                    flags: edge_flag_names(cfg.flags),
                    stats: VmEdgeCounters {
                        out_pkts: s.out_pkts,
                        out_bytes: s.out_bytes,
                        in_pkts: s.in_pkts,
                        in_bytes: s.in_bytes,
                        denied: s.denied,
                        observed: s.observed,
                        rate_dropped: s.rate_dropped,
                    },
                }
            })
            .collect();
        taps.sort_by(|a, b| a.iface.cmp(&b.iface));
        let covered: HashSet<&str> = self.vm_edge.taps.values().map(|(_, v)| v.as_str()).collect();
        VmEdgeStatus {
            vms: self.vm_edge.state.vms.len(),
            rules: self.vm_edge.policy.len(),
            groups: self.vm_edge.groups.clone(),
            missing: self
                .vm_edge
                .state
                .vms
                .iter()
                .filter(|v| !covered.contains(v.name.as_str()))
                .map(|v| v.name.clone())
                .collect(),
            taps,
            enforcing: self.lease_live(),
            owner: self.vm_edge.state.owner.clone(),
            peers: self.vm_edge.state.peers.len(),
            flow_log: self.vm_edge.state.flow_log,
            cilium: crate::netpol::cilium_present(),
        }
    }

    // ---- QEMU sandbox --------------------------------------------------------

    pub(super) fn vm_sandbox_configure(&mut self, config: VmSandboxConfig) -> Result<VmSandboxStatus> {
        let enforce = match config.mode.as_str() {
            "observe" => false,
            "enforce" => true,
            m => return Err(anyhow!("sandbox mode `{m}`: expected observe or enforce")),
        };
        let mut rules = default_dev_rules();
        for s in &config.extra_devices {
            rules.push(parse_dev_rule(s)?);
        }
        if rules.len() > QEMU_DEV_RULES {
            return Err(anyhow!("at most {QEMU_DEV_RULES} device rules (have {})", rules.len()));
        }
        let ports = parse_ports(&config.egress_ports)?;
        let mut cfg = QemuSandboxCfg { n: rules.len() as u32, flags: if enforce { SANDBOX_ENFORCE } else { 0 }, ..Default::default() };
        cfg.rules[..rules.len()].copy_from_slice(&rules);
        self.dp.array_set("QEMU_SANDBOX", 0, cfg)?;
        for p in self.sandbox.ports.clone() {
            if !ports.contains(&p) {
                self.dp.cni_hash_remove::<u16, u8>("QEMU_PORTS", &p);
            }
        }
        for p in &ports {
            if !self.sandbox.ports.contains(p) {
                self.dp.cni_hash_insert("QEMU_PORTS", *p, 1u8)?;
            }
        }
        self.sandbox.ports = ports;
        self.sandbox.devices = rules.iter().map(dev_rule_string).collect();
        self.sandbox.config = config;
        self.sandbox.configured = true;
        self.sandbox_refresh();
        Ok(self.vm_sandbox_status())
    }

    fn ensure_sandbox_config(&mut self) -> Result<()> {
        if !self.sandbox.configured {
            self.vm_sandbox_configure(self.sandbox.config.clone())?;
        }
        Ok(())
    }

    pub(super) fn vm_sandbox_attach(&mut self, vm: &str, cgroup: Option<&str>) -> Result<VmSandboxStatus> {
        self.ensure_sandbox_config()?;
        let rel = match cgroup {
            Some(c) => c.trim_matches('/').to_string(),
            None => qemu_scopes().remove(vm).ok_or_else(|| anyhow!("no running machine-qemu scope for VM {vm}"))?,
        };
        if rel.split('/').any(|s| s == "..") {
            return Err(anyhow!("cgroup path must stay under {}", attribution::CGROUP_ROOT));
        }
        if let Some(old) = self.sandbox.attached.get(vm).cloned() {
            if old != rel {
                self.dp.detach_sandbox(&format!("{}/{old}", attribution::CGROUP_ROOT));
            }
        }
        self.dp.attach_sandbox(&Path::new(attribution::CGROUP_ROOT).join(&rel))?;
        self.sandbox.attached.insert(vm.to_string(), rel);
        self.sandbox.pinned.insert(vm.to_string());
        Ok(self.vm_sandbox_status())
    }

    pub(super) fn vm_sandbox_detach(&mut self, vm: &str) -> bool {
        self.sandbox.pinned.remove(vm);
        match self.sandbox.attached.remove(vm) {
            Some(rel) => {
                self.dp.detach_sandbox(&format!("{}/{rel}", attribution::CGROUP_ROOT));
                true
            }
            None => false,
        }
    }

    /// Forget scopes that went away; with `auto` (or for pinned VMs that
    /// restarted) sandbox running QEMU scopes.
    pub(super) fn sandbox_refresh(&mut self) {
        if !self.sandbox.config.auto && self.sandbox.attached.is_empty() && self.sandbox.pinned.is_empty() {
            return;
        }
        let root = Path::new(attribution::CGROUP_ROOT);
        let gone: Vec<(String, String)> = self
            .sandbox
            .attached
            .iter()
            .filter(|(_, rel)| !root.join(rel).is_dir())
            .map(|(v, r)| (v.clone(), r.clone()))
            .collect();
        for (vm, rel) in gone {
            self.dp.forget_sandbox(&format!("{}/{rel}", attribution::CGROUP_ROOT));
            self.sandbox.attached.remove(&vm);
        }
        if self.ensure_sandbox_config().is_err() {
            return;
        }
        for (vm, rel) in qemu_scopes() {
            if self.sandbox.attached.contains_key(&vm) || !(self.sandbox.config.auto || self.sandbox.pinned.contains(&vm)) {
                continue;
            }
            match self.dp.attach_sandbox(&root.join(&rel)) {
                Ok(()) => {
                    self.sandbox.attached.insert(vm, rel);
                }
                Err(e) => {
                    let n = format!("sandbox {vm}: {e:#}");
                    if !self.sandbox.notes.contains(&n) {
                        tracing::warn!("{n}");
                        self.sandbox.notes.push(n);
                    }
                }
            }
        }
    }

    pub(super) fn vm_sandbox_status(&mut self) -> VmSandboxStatus {
        let dev = self.dp.hash_entries::<DevHitKey, u64>("QEMU_DEV_HITS").unwrap_or_default();
        let net = self.dp.hash_entries::<NetHitKey, u64>("QEMU_NET_HITS").unwrap_or_default();
        let mut sh = lock(&self.shared);
        let mut who = |id: u64| -> (Option<String>, Option<String>) {
            let path = sh.cgroups.lookup(id);
            let vm = path.as_deref().and_then(|p| attribution::classify_cgroup(p).vm);
            (vm, path)
        };
        let mut device_hits: Vec<SandboxHit> = dev
            .into_iter()
            .map(|(k, count)| {
                let (vm, cgroup) = who(k.cgroup);
                let r = QemuDevRule { major: k.major, minor: k.minor, dev_type: k.dev_type as u32, access: k.access as u32 };
                SandboxHit { vm, cgroup, target: dev_rule_string(&r), count }
            })
            .collect();
        let mut egress_hits: Vec<SandboxHit> = net
            .into_iter()
            .map(|(k, count)| {
                let (vm, cgroup) = who(k.cgroup);
                let addr = fmt_addr(&k.addr);
                let port = u16::from_be_bytes(k.port);
                let target = if addr.contains(':') {
                    format!("{} [{addr}]:{port}", proto_name(k.proto))
                } else {
                    format!("{} {addr}:{port}", proto_name(k.proto))
                };
                SandboxHit { vm, cgroup, target, count }
            })
            .collect();
        drop(sh);
        device_hits.sort_by_key(|h| std::cmp::Reverse(h.count));
        egress_hits.sort_by_key(|h| std::cmp::Reverse(h.count));
        VmSandboxStatus {
            config: self.sandbox.config.clone(),
            devices: self.sandbox.devices.clone(),
            attached: self.sandbox.attached.clone(),
            enforcing: self.sandbox.config.mode == "enforce" && self.lease_live(),
            device_hits,
            egress_hits,
            notes: self.sandbox.notes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_rules_round_trip() {
        let r = parse_dev_rule("c 10:232 rw").unwrap();
        assert_eq!((r.major, r.minor, r.dev_type, r.access), (10, 232, DEVCG_DEV_CHAR, DEVCG_ACC_READ | DEVCG_ACC_WRITE));
        assert_eq!(dev_rule_string(&r), "c 10:232 rw");
        let r = parse_dev_rule("b 8:*").unwrap();
        assert_eq!(r.minor, DEV_MINOR_ANY);
        assert_eq!(dev_rule_string(&r), "b 8:* rwm");
        assert!(parse_dev_rule("x 1:1").is_err());
        assert!(parse_dev_rule("c 1:1 rq").is_err());
    }

    #[test]
    fn ports_and_identities() {
        let p = parse_ports(&["49152-49155".into(), "10809".into()]).unwrap();
        assert_eq!(p.len(), 5);
        assert!(parse_ports(&["0".into()]).is_err());
        assert!(parse_ports(&["2000-1000".into()]).is_err());
        let a = group_identity("web");
        assert_eq!(a, group_identity("web"));
        assert_ne!(a, group_identity("db"));
        assert!(group_identity("") >= 16);
    }
}

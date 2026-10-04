// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Segmentation evidence for audits: what is configured (policies with
//! content hashes, project isolation, egress controls), what is in force
//! (per-host enforcement and sync), what the policy set allows between
//! groups of VMs (a traced reachability matrix), and what was observed
//! (denied flows, alerts, exceptions and approvals). The report carries a
//! SHA-256 digest of its own content so a stored copy can be checked.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::trace::Tracer;
use super::{NetpolService, NetpolVm, TraceQuery, VmNetworkPolicy};
use crate::api::VmFlowEdge;

pub const KIND: &str = "machina.io/segmentation-evidence/v1";
/// Connections each matrix cell is checked on.
pub const PROBES: &[(&str, u16)] = &[
    ("TCP", 22),
    ("TCP", 80),
    ("TCP", 443),
    ("TCP", 3389),
    ("TCP", 5432),
    ("UDP", 53),
];
/// VMs per group tried for each cell.
pub const SAMPLE: usize = 3;
pub const MAX_GROUPS: usize = 40;
pub const TOP_DENIED: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Summary {
    pub policies: usize,
    pub generated_policies: usize,
    pub vms: usize,
    /// VMs selected by at least one policy.
    pub vms_selected: usize,
    pub projects: usize,
    pub projects_isolated: usize,
    pub hosts: usize,
    pub hosts_enforcing: usize,
    pub hosts_in_sync: usize,
    pub denied_connections: usize,
    pub alerts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostEvidence {
    pub hostname: String,
    pub reachable: bool,
    /// Drops are enforced (enforcement lease held), not only audited.
    pub enforcing: bool,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub synced_at: Option<String>,
    #[serde(default)]
    pub in_sync: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyEvidence {
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    /// Generated from project settings.
    pub generated: bool,
    #[serde(default)]
    pub description: Option<String>,
    /// SHA-256 of the policy's canonical YAML.
    pub sha256: String,
    pub selected_vms: Vec<String>,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixCell {
    pub from: String,
    pub to: String,
    /// Probes allowed (`TCP/443`), empty = fully segmented for the probes.
    pub allowed: Vec<String>,
    pub denied: Vec<String>,
    /// VM pairs the cell was traced on.
    pub samples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeniedEdge {
    pub src: String,
    pub dst: String,
    pub proto: String,
    pub port: u16,
    pub flows: u64,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub policy: Option<String>,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Evidence {
    pub kind: String,
    pub generated_at: String,
    pub generated_by: String,
    /// `host` (one daemon) or `fleet` (the controller).
    pub scope: String,
    pub source: String,
    pub summary: Summary,
    pub hosts: Vec<HostEvidence>,
    pub policies: Vec<PolicyEvidence>,
    #[serde(default)]
    pub projects: Vec<Value>,
    /// How groups were formed (`project` or `vm`) and the probes used.
    pub matrix_groups: String,
    pub matrix_probes: Vec<String>,
    pub matrix: Vec<MatrixCell>,
    pub denied: Vec<DeniedEdge>,
    pub alerts: Vec<Value>,
    pub quarantines: Vec<Value>,
    pub temporary_access: Vec<Value>,
    pub threat_feeds: Vec<Value>,
    #[serde(default)]
    pub egress_ips: Vec<Value>,
    #[serde(default)]
    pub approvals: Vec<Value>,
    /// SHA-256 over the report with this field empty.
    pub digest: String,
}

pub fn policy_hash(p: &VmNetworkPolicy) -> String {
    hex(&Sha256::digest(p.to_yaml().as_bytes()))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Groups for the matrix: projects when any VM has one (VMs without a
/// project form `(no project)`), otherwise one group per VM. Capped.
pub fn groups(vms: &[NetpolVm]) -> (String, Vec<(String, Vec<String>)>) {
    let by_project = vms.iter().any(|v| v.project.is_some());
    let mut g: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in vms {
        let key = if by_project {
            v.project.clone().unwrap_or_else(|| "(no project)".into())
        } else {
            v.name.clone()
        };
        g.entry(key).or_default().push(v.name.clone());
    }
    let kind = if by_project { "project" } else { "vm" };
    (kind.into(), g.into_iter().take(MAX_GROUPS).collect())
}

/// Trace every group pair (and the host / internet) on [`PROBES`].
pub fn matrix(
    policies: &[VmNetworkPolicy],
    vms: &[NetpolVm],
    services: &[NetpolService],
    host_addresses: &[String],
    groups: &[(String, Vec<String>)],
) -> Vec<MatrixCell> {
    let t = Tracer::new(policies, vms, services, host_addresses, &[]);
    let mut ends: Vec<(String, Vec<String>)> = vec![
        ("host".into(), vec!["host".into()]),
        ("world".into(), vec!["world".into()]),
    ];
    ends.extend(
        groups
            .iter()
            .map(|(n, m)| (n.clone(), m.iter().take(SAMPLE).cloned().collect())),
    );
    let mut out = Vec::new();
    for (from, fm) in &ends {
        for (to, tm) in &ends {
            let pseudo = |n: &str| n == "host" || n == "world";
            if pseudo(from) && pseudo(to) {
                continue;
            }
            let pairs: Vec<(&String, &String)> = fm
                .iter()
                .flat_map(|a| tm.iter().map(move |b| (a, b)))
                .filter(|(a, b)| a != b)
                .collect();
            if pairs.is_empty() {
                continue;
            }
            let mut cell = MatrixCell {
                from: from.clone(),
                to: to.clone(),
                samples: pairs.iter().map(|(a, b)| format!("{a} → {b}")).collect(),
                ..Default::default()
            };
            for (proto, port) in PROBES {
                let ok = pairs.iter().any(|(a, b)| {
                    t.trace(&TraceQuery {
                        from: (*a).clone(),
                        to: (*b).clone(),
                        protocol: proto.to_string(),
                        port: *port,
                        ..Default::default()
                    })
                    .is_ok_and(|r| r.allowed)
                });
                let label = format!("{proto}/{port}");
                if ok {
                    cell.allowed.push(label);
                } else {
                    cell.denied.push(label);
                }
            }
            out.push(cell);
        }
    }
    out
}

/// Denied connections from the flow history, most flows first.
pub fn denied(edges: &[VmFlowEdge]) -> Vec<DeniedEdge> {
    let mut d: Vec<DeniedEdge> = edges
        .iter()
        .filter(|e| matches!(e.verdict.as_str(), "DROPPED" | "AUDIT" | "DENIED"))
        .map(|e| DeniedEdge {
            src: e.src.clone(),
            dst: e.dst.clone(),
            proto: e.proto.clone(),
            port: e.port,
            flows: e.count,
            reason: e.drop_reason.clone(),
            policy: e.policy.clone(),
            last_seen: e.last_seen.clone(),
        })
        .collect();
    d.sort_by(|a, b| b.flows.cmp(&a.flows).then(a.src.cmp(&b.src)));
    d
}

impl Evidence {
    /// Fill the summary and digest; call last.
    pub fn seal(&mut self, denied_total: usize) {
        self.kind = KIND.into();
        let s = &mut self.summary;
        s.policies = self.policies.iter().filter(|p| !p.generated).count();
        s.generated_policies = self.policies.iter().filter(|p| p.generated).count();
        let mut sel: Vec<&String> = self
            .policies
            .iter()
            .filter(|p| p.enabled)
            .flat_map(|p| &p.selected_vms)
            .collect();
        sel.sort();
        sel.dedup();
        s.vms_selected = sel.len();
        s.projects = self.projects.len();
        s.projects_isolated = self
            .projects
            .iter()
            .filter(|p| p["isolated"].as_bool() == Some(true))
            .count();
        s.hosts = self.hosts.len();
        s.hosts_enforcing = self.hosts.iter().filter(|h| h.enforcing).count();
        s.hosts_in_sync = self.hosts.iter().filter(|h| h.in_sync).count();
        s.denied_connections = denied_total;
        s.alerts = self.alerts.len();
        self.matrix_probes = PROBES.iter().map(|(p, n)| format!("{p}/{n}")).collect();
        self.denied.truncate(TOP_DENIED);
        self.digest = String::new();
        self.digest = self.compute_digest();
    }

    pub fn compute_digest(&self) -> String {
        let mut c = self.clone();
        c.digest = String::new();
        hex(&Sha256::digest(
            serde_json::to_vec(&c).unwrap_or_default().as_slice(),
        ))
    }

    /// The digest matches the content.
    pub fn verify(&self) -> bool {
        !self.digest.is_empty() && self.digest == self.compute_digest()
    }
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn str_of(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(s) => s.clone(),
        Value::Null => "—".into(),
        x => x.to_string(),
    }
}

/// The report as Markdown.
pub fn markdown(e: &Evidence) -> String {
    let s = &e.summary;
    let mut o = String::new();
    o.push_str("# VM network segmentation evidence\n\n");
    o.push_str(&format!(
        "- Generated: {} by {}\n- Scope: {} ({})\n- Digest (SHA-256): `{}`\n\n",
        e.generated_at, e.generated_by, e.scope, e.source, e.digest
    ));
    o.push_str("## Summary\n\n| Item | Value |\n|---|---|\n");
    for (k, v) in [
        ("Policies", s.policies.to_string()),
        (
            "Generated project policies",
            s.generated_policies.to_string(),
        ),
        (
            "VMs",
            format!("{} ({} selected by a policy)", s.vms, s.vms_selected),
        ),
        (
            "Projects",
            format!("{} ({} isolated)", s.projects, s.projects_isolated),
        ),
        (
            "Hosts",
            format!(
                "{} ({} enforcing, {} in sync)",
                s.hosts, s.hosts_enforcing, s.hosts_in_sync
            ),
        ),
        (
            "Denied connections in history",
            s.denied_connections.to_string(),
        ),
        ("Recent alerts", s.alerts.to_string()),
    ] {
        o.push_str(&format!("| {k} | {v} |\n"));
    }
    o.push_str("\n## Enforcement per host\n\n| Host | Reachable | Mode | Owner | In sync | Last sync | Error |\n|---|---|---|---|---|---|---|\n");
    for h in &e.hosts {
        o.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            md_escape(&h.hostname),
            if h.reachable { "yes" } else { "no" },
            if h.enforcing {
                "enforce"
            } else {
                "observe (audit only)"
            },
            if h.owner.is_empty() { "—" } else { &h.owner },
            if h.in_sync { "yes" } else { "no" },
            h.synced_at.as_deref().unwrap_or("—"),
            md_escape(h.error.as_deref().unwrap_or("—")),
        ));
    }
    o.push_str("\n## Policies\n\n| Name | Kind | Enabled | Source | Selects | SHA-256 | Description |\n|---|---|---|---|---|---|---|\n");
    for p in &e.policies {
        o.push_str(&format!(
            "| {} | {} | {} | {} | {} | `{}` | {} |\n",
            md_escape(&p.name),
            p.kind,
            if p.enabled { "yes" } else { "no" },
            if p.generated {
                "project settings"
            } else {
                "stored"
            },
            md_escape(&p.selected_vms.join(", ")),
            &p.sha256[..16.min(p.sha256.len())],
            md_escape(p.description.as_deref().unwrap_or("—")),
        ));
    }
    if !e.projects.is_empty() {
        o.push_str("\n## Projects\n\n| Project | VMs | Isolation | Egress | Egress IPs |\n|---|---|---|---|---|\n");
        for p in &e.projects {
            o.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                md_escape(&str_of(p, "project")),
                p["vms"].as_array().map_or(0, Vec::len),
                str_of(p, "isolation"),
                md_escape(&str_of(p, "egress")),
                md_escape(&str_of(p, "egress_ips")),
            ));
        }
    }
    o.push_str(&format!(
        "\n## Reachability matrix\n\nTraced against the policy set, by {}, on {}. A blank *Allowed* cell means the pair is segmented for every probe.\n\n| From | To | Allowed | Samples |\n|---|---|---|---|\n",
        e.matrix_groups,
        e.matrix_probes.join(", ")
    ));
    for c in &e.matrix {
        o.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            md_escape(&c.from),
            md_escape(&c.to),
            c.allowed.join(", "),
            md_escape(&c.samples.join(", ")),
        ));
    }
    o.push_str("\n## Denied connections (flow history)\n\n");
    if e.denied.is_empty() {
        o.push_str("None recorded.\n");
    } else {
        o.push_str("| Source | Destination | Port | Flows | Reason | Policy | Last seen |\n|---|---|---|---|---|---|---|\n");
        for d in &e.denied {
            o.push_str(&format!(
                "| {} | {} | {}/{} | {} | {} | {} | {} |\n",
                md_escape(&d.src),
                md_escape(&d.dst),
                d.proto.to_ascii_lowercase(),
                d.port,
                d.flows,
                d.reason.as_deref().unwrap_or("—"),
                md_escape(d.policy.as_deref().unwrap_or("—")),
                d.last_seen,
            ));
        }
    }
    let list = |o: &mut String, title: &str, items: &[Value], cols: &[&str]| {
        o.push_str(&format!("\n## {title}\n\n"));
        if items.is_empty() {
            o.push_str("None.\n");
            return;
        }
        o.push_str(&format!(
            "| {} |\n|{}\n",
            cols.join(" | "),
            "---|".repeat(cols.len())
        ));
        for i in items {
            let row: Vec<String> = cols.iter().map(|c| md_escape(&str_of(i, c))).collect();
            o.push_str(&format!("| {} |\n", row.join(" | ")));
        }
    };
    list(
        &mut o,
        "Alerts",
        &e.alerts,
        &["ts", "kind", "severity", "src", "detail"],
    );
    list(
        &mut o,
        "Quarantines",
        &e.quarantines,
        &["vm", "since", "until", "by", "reason"],
    );
    list(
        &mut o,
        "Temporary access",
        &e.temporary_access,
        &[
            "name",
            "from",
            "to",
            "port",
            "expires_at",
            "granted_by",
            "reason",
        ],
    );
    list(
        &mut o,
        "DNS threat feeds",
        &e.threat_feeds,
        &["name", "source", "block", "domains", "updated"],
    );
    if !e.egress_ips.is_empty() {
        list(
            &mut o,
            "Egress IPs",
            &e.egress_ips,
            &["hostname", "project", "egress_ip", "sources"],
        );
    }
    if e.scope == "fleet" {
        list(
            &mut o,
            "Approvals (last 90 days)",
            &e.approvals,
            &[
                "created_at",
                "action_type",
                "label",
                "requested_by",
                "approved_by",
                "status",
            ],
        );
    }
    o
}

#[cfg(test)]
mod tests {
    use super::super::parse_documents;
    use super::*;

    fn vm(name: &str, project: Option<&str>, ip: &str) -> NetpolVm {
        NetpolVm {
            name: name.into(),
            host: Some("h1".into()),
            project: project.map(Into::into),
            labels: BTreeMap::new(),
            addresses: vec![ip.into()],
        }
    }

    #[test]
    fn matrix_shows_isolation_and_digest_detects_tampering() {
        let vms = vec![
            vm("a1", Some("shop"), "10.0.0.5"),
            vm("a2", Some("shop"), "10.0.0.6"),
            vm("b1", Some("lab"), "10.0.0.9"),
            vm("c1", None, "10.0.0.20"),
        ];
        let (p, v) = parse_documents(
            "apiVersion: cilium.io/v2\nkind: CiliumNetworkPolicy\nmetadata:\n  name: iso\nspec:\n  endpointSelector:\n    matchLabels:\n      machina.io/project: shop\n  ingress:\n  - fromEndpoints:\n    - matchLabels:\n        machina.io/project: shop\n    toPorts:\n    - ports:\n      - port: \"5432\"\n",
        );
        assert!(v.ok(), "{:?}", v.errors);
        let (kind, g) = groups(&vms);
        assert_eq!(kind, "project");
        assert_eq!(
            g.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
            ["(no project)", "lab", "shop"]
        );
        let m = matrix(&p, &vms, &[], &["192.168.1.10".into()], &g);
        let cell = |f: &str, t: &str| m.iter().find(|c| c.from == f && c.to == t).unwrap();
        assert!(cell("lab", "shop").allowed.is_empty());
        assert_eq!(cell("shop", "shop").allowed, ["TCP/5432"]);
        assert_eq!(cell("shop", "shop").samples, ["a1 → a2", "a2 → a1"]);
        assert_eq!(cell("shop", "lab").allowed.len(), PROBES.len());
        assert!(cell("world", "shop").allowed.is_empty());
        assert!(
            !m.iter().any(|c| c.from == "lab" && c.to == "lab"),
            "one VM: no pair"
        );

        let mut e = Evidence {
            generated_at: "2026-10-04T10:00:00Z".into(),
            scope: "fleet".into(),
            policies: vec![PolicyEvidence {
                name: "iso".into(),
                enabled: true,
                sha256: policy_hash(&p[0]),
                selected_vms: vec!["a1".into(), "a2".into()],
                ..Default::default()
            }],
            matrix_groups: kind,
            matrix: m,
            ..Default::default()
        };
        e.seal(0);
        assert_eq!(e.summary.vms_selected, 2);
        assert_eq!(e.digest.len(), 64);
        assert!(e.verify());
        let md = markdown(&e);
        assert!(md.contains("| lab | shop |  |"), "{md}");
        assert!(md.contains(&e.digest));
        e.policies[0].enabled = false;
        assert!(!e.verify());
    }
}

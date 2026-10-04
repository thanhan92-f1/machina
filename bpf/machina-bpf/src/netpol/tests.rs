// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

use std::collections::BTreeMap;

use machina_bpf_common::IDENTITY_WORLD;

use super::*;

#[test]
fn cilium_backup_configs_are_not_cilium() {
    assert!(cni_config_name("05-cilium.conflist"));
    assert!(cni_config_name("05-cilium.conf"));
    assert!(!cni_config_name("00-multus.conf.cilium_bak"));
    assert!(!cni_config_name("05-cilium.conflist.bak"));
}

fn vm(name: &str, host: &str, ip: &str, labels: &[(&str, &str)]) -> NetpolVm {
    NetpolVm {
        name: name.into(),
        host: Some(host.into()),
        project: Some("p1".into()),
        labels: labels.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<BTreeMap<_, _>>(),
        addresses: vec![ip.into()],
    }
}

fn fleet() -> Vec<NetpolVm> {
    vec![
        vm("web-1", "h1", "10.0.0.5", &[("app", "web"), ("env", "prod")]),
        vm("web-2", "h2", "10.0.0.6", &[("app", "web"), ("env", "dev")]),
        vm("db-1", "h1", "10.0.0.9", &[("app", "db"), ("env", "prod"), ("machina.io/port.pg", "5432")]),
        vm("lb-1", "h2", "10.0.0.2", &[("app", "lb")]),
    ]
}

fn parse(y: &str) -> Vec<VmNetworkPolicy> {
    let (p, v) = parse_documents(y);
    assert!(v.ok(), "{:?}", v.errors);
    p
}

fn trace_q(p: &[VmNetworkPolicy], from: &str, to: &str, proto: &str, port: u16) -> TraceResult {
    trace(p, &fleet(), &["192.168.1.10".into()], &["192.168.1.11".into()], &TraceQuery {
        from: from.into(),
        to: to.into(),
        protocol: proto.into(),
        port,
        icmp_type: None,
    })
    .unwrap()
}

const WEB_DB: &str = r#"
apiVersion: machina.io/v1
kind: VmNetworkPolicy
metadata: {name: db}
spec:
  description: web may talk to db on pg
  endpointSelector: {matchLabels: {app: db}}
  ingress:
    - fromEndpoints: [{matchLabels: {app: web}}]
      toPorts: [{ports: [{port: pg, protocol: TCP}]}]
    - fromEntities: [host]
      icmps: [{fields: [{type: EchoRequest}]}]
"#;

#[test]
fn parse_multi_doc_and_cilium_kind() {
    let y = format!(
        "{WEB_DB}---\napiVersion: cilium.io/v2\nkind: CiliumNetworkPolicy\nmetadata: {{name: c, namespace: x}}\nspecs:\n  - endpointSelector: {{}}\n    egress: [{{toEntities: [world]}}]\n"
    );
    let (p, v) = parse_documents(&y);
    assert!(v.ok(), "{:?}", v.errors);
    assert_eq!(p.len(), 2);
    assert_eq!(p[1].annotations.get("machina.io/source-kind").map(String::as_str), Some("CiliumNetworkPolicy"));
    assert!(v.warnings.iter().any(|w| w.path.ends_with("metadata.namespace")));
    assert_eq!(p[0].description().as_deref(), Some("web may talk to db on pg"));
    let round = parse(&p[0].to_yaml());
    assert_eq!(round[0], p[0]);
}

#[test]
fn validation_paths() {
    let (_, v) = parse_documents(
        r#"
apiVersion: machina.io/v1
kind: VmNetworkPolicy
metadata: {name: Bad_Name}
spec:
  endpointSelector: {matchExpressions: [{key: a, operator: Maybe}]}
  ingress:
    - fromCIDR: [10.0.0.1]
      toPorts: [{ports: [{port: "80", endPort: 9000, protocol: TCP}]}]
      bogus: 1
  egress:
    - toFQDNs: [{matchName: example.com}, {matchName: "*.bad"}]
      toPorts: [{ports: [{port: "443"}], rules: {http: [{method: GET}]}}]
  egressDeny:
    - toFQDNs: [{matchName: example.com}]
"#,
    );
    let paths: Vec<&str> = v.errors.iter().map(|e| e.path.as_str()).collect();
    for want in [
        "document[0].metadata.name",
        "document[0].spec.endpointSelector.matchExpressions[0].operator",
        "document[0].spec.ingress[0].fromCIDR[0]",
        "document[0].spec.ingress[0].toPorts[0].ports[0].endPort",
        "document[0].spec.ingress[0].bogus",
        "document[0].spec.egress[0].toFQDNs[1].matchName",
        "document[0].spec.egressDeny[0].toFQDNs",
    ] {
        assert!(paths.contains(&want), "missing {want} in {paths:?}");
    }
    assert!(!v.warnings.iter().any(|w| w.path.contains("toFQDNs")));
    assert!(v.warnings.iter().any(|w| w.path.ends_with("toPorts[0].rules")));
}

#[test]
fn compile_per_host_named_ports_and_icmp() {
    let p = parse(WEB_DB);
    let vms = fleet();
    let c = compile(&Inputs { policies: &p, vms: &vms, host: Some("h1"), host_addresses: &[], remote_node_addresses: &[] });
    let names: Vec<&str> = c.state.vms.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["db-1", "web-1"], "only h1 VMs are local");
    let db = vm_identity("db-1");
    let db_vm = c.state.vms.iter().find(|v| v.name == "db-1").unwrap();
    assert!(db_vm.isolate_ingress && !db_vm.isolate_egress);
    assert!(c.state.peers.iter().any(|x| x.cidr == "10.0.0.6" && x.name == "web-2"), "remote VM is a peer");
    let pg: Vec<_> = c.state.policy.iter().filter(|r| r.port == 5432).collect();
    assert_eq!(pg.len(), 2, "both web VMs (local and remote) reach pg");
    assert!(pg.iter().all(|r| r.subject_identity == Some(db) && r.proto == 6 && !r.egress));
    assert!(c.state.policy.iter().any(|r| r.proto == 1 && r.port == 9 && r.peer_identity == Some(machina_bpf_common::IDENTITY_HOST)));
    let c2 = compile(&Inputs { policies: &p, vms: &vms, host: Some("h2"), host_addresses: &[], remote_node_addresses: &[] });
    assert!(c2.state.policy.is_empty(), "no subjects on h2");
    assert!(c2.endpoints.iter().any(|e| e.name == "db-1" && e.ingress_enforced));
}

#[test]
fn trace_allow_default_deny_and_deny_precedence() {
    let mut p = parse(WEB_DB);
    assert!(trace_q(&p, "web-2", "db-1", "TCP", 5432).allowed);
    let t = trace_q(&p, "lb-1", "db-1", "TCP", 5432);
    assert!(!t.allowed);
    assert_eq!(t.ingress.verdict, "default-deny");
    assert_eq!(trace_q(&p, "db-1", "lb-1", "TCP", 80).egress.verdict, "no-policy");

    p.extend(parse(
        r#"
apiVersion: machina.io/v1
kind: VmNetworkPolicy
metadata: {name: no-dev}
spec:
  endpointSelector: {matchLabels: {app: db}}
  enableDefaultDeny: {ingress: false}
  ingressDeny:
    - fromEndpoints: [{matchLabels: {env: dev}}]
"#,
    ));
    let t = trace_q(&p, "web-2", "db-1", "TCP", 5432);
    assert!(!t.allowed, "deny beats the pg allow");
    assert_eq!(t.ingress.rule.as_deref(), Some("no-dev spec.ingressDeny[0]"));
    assert!(trace_q(&p, "web-1", "db-1", "TCP", 5432).allowed);
}

#[test]
fn cidr_except_world_and_requires() {
    let p = parse(
        r#"
apiVersion: machina.io/v1
kind: VmNetworkPolicy
metadata: {name: egress}
specs:
  - endpointSelector: {matchLabels: {app: web}}
    egress:
      - toCIDRSet: [{cidr: 203.0.113.0/24, except: [203.0.113.128/25]}]
        toPorts: [{ports: [{port: "443", endPort: 444, protocol: TCP}]}]
      - toEndpoints: [{}]
      - toEntities: [world]
        toPorts: [{ports: [{port: "53", protocol: UDP}]}]
  - endpointSelector: {matchLabels: {app: db}}
    ingress:
      - fromRequires: [{matchLabels: {env: prod}}]
      - fromEndpoints: [{matchLabels: {app: web}}]
"#,
    );
    assert!(trace_q(&p, "web-1", "203.0.113.10", "TCP", 444).allowed);
    assert!(!trace_q(&p, "web-1", "203.0.113.200", "TCP", 443).allowed, "except carves out the /25");
    assert!(!trace_q(&p, "web-1", "198.51.100.1", "TCP", 443).allowed);
    assert!(trace_q(&p, "web-1", "198.51.100.1", "UDP", 53).allowed);
    assert!(trace_q(&p, "web-1", "203.0.113.200", "UDP", 53).allowed, "world covers CIDR identities");
    assert!(trace_q(&p, "web-1", "lb-1", "TCP", 9999).allowed, "toEndpoints [{{}}] = every VM");
    assert!(trace_q(&p, "web-1", "db-1", "TCP", 1).allowed, "prod web passes fromRequires");
    let t = trace_q(&p, "web-2", "db-1", "TCP", 1);
    assert!(!t.allowed && t.ingress.verdict == "default-deny", "dev web lacks env=prod");
    let w = trace_q(&p, "web-1", "world", "TCP", 443);
    assert_eq!(w.to.identity, IDENTITY_WORLD);
}

#[test]
fn empty_rule_denies_l4_only_allows_any() {
    let p = parse(
        r#"
apiVersion: machina.io/v1
kind: VmNetworkPolicy
metadata: {name: lock}
spec:
  endpointSelector: {matchLabels: {machina.io/vm-name: lb-1}}
  ingress: [{}]
  egress: [{toPorts: [{ports: [{port: "53", protocol: ANY}]}]}]
"#,
    );
    assert!(!trace_q(&p, "web-1", "lb-1", "TCP", 80).allowed);
    assert!(trace_q(&p, "lb-1", "1.1.1.1", "UDP", 53).allowed);
    assert!(trace_q(&p, "lb-1", "1.1.1.1", "SCTP", 53).allowed);
    assert!(!trace_q(&p, "lb-1", "1.1.1.1", "TCP", 80).allowed);
    let vms = fleet();
    let c = compile(&Inputs { policies: &p, vms: &vms, host: None, host_addresses: &[], remote_node_addresses: &[] });
    let sel = c.selectors.iter().find(|s| s.path == "spec.endpointSelector").unwrap();
    assert_eq!(sel.vms, ["lb-1"]);
    assert_eq!(sel.selector, "machina.io/vm-name=lb-1");
}

#[test]
fn fqdn_rules_compile_and_trace() {
    let p = parse(
        r#"
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata: {name: dns-egress}
spec:
  endpointSelector: {matchLabels: {app: web}}
  egress:
    - toEndpoints: [{matchLabels: {app: db}}]
    - toEntities: [world]
      toPorts: [{ports: [{port: "53", protocol: UDP}]}]
    - toFQDNs: [{matchName: API.Example.com.}, {matchPattern: "**.cdn.example.net"}]
      toPorts: [{ports: [{port: "443", protocol: TCP}]}]
"#,
    );
    let vms = fleet();
    let c = compile(&Inputs { policies: &p, vms: &vms, host: Some("h1"), host_addresses: &[], remote_node_addresses: &[] });
    assert!(c.warnings.is_empty(), "{:?}", c.warnings);
    let web1 = vm_identity("web-1");
    let pats: Vec<&str> = c.state.fqdn.iter().map(|r| r.pattern.as_str()).collect();
    assert_eq!(pats, ["**.cdn.example.net", "api.example.com"], "only the local web VM, normalized");
    assert!(c.state.fqdn.iter().all(|r| r.subject_identity == web1 && r.proto == 6 && r.port == 443));
    assert!(c.state.vms.iter().find(|v| v.name == "web-1").unwrap().isolate_egress);

    let t = trace_q(&p, "web-1", "api.example.com", "TCP", 443);
    assert!(t.allowed, "{}", t.summary);
    assert_eq!(t.to.kind, "fqdn");
    assert_eq!(t.egress.rule.as_deref(), Some("dns-egress spec.egress[2]"));
    assert!(trace_q(&p, "web-2", "a.b.cdn.example.net", "TCP", 443).allowed);
    assert!(!trace_q(&p, "web-1", "cdn.example.net", "TCP", 443).allowed);
    assert!(!trace_q(&p, "web-1", "api.example.com", "TCP", 80).allowed);
    assert!(!trace_q(&p, "web-1", "203.0.113.5", "TCP", 443).allowed, "an address is not a name");
}

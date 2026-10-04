// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Per-project egress addresses: VM traffic leaving the host is source-NATed
//! to a project's egress IP by one nftables table, `ip machina_egress`, that
//! is replaced atomically on every change. IPv4 only.

use std::collections::BTreeSet;
use std::net::Ipv4Addr;

use crate::api::{VmEgressSnat, VmEgressSnatRule};

pub const TABLE: &str = "machina_egress";

/// Destinations that stay un-NATed unless the config lists its own.
pub const DEFAULT_EXCLUDE: &[&str] = &[
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "100.64.0.0/10",
    "169.254.0.0/16",
    "127.0.0.0/8",
    "224.0.0.0/4",
];

pub const MAX_RULES: usize = 256;
pub const MAX_SOURCES: usize = 4096;

fn cidr4(s: &str) -> Option<String> {
    let (a, p) = s.split_once('/').unwrap_or((s, "32"));
    let a: Ipv4Addr = a.trim().parse().ok()?;
    let p: u8 = p.trim().parse().ok().filter(|p| *p <= 32)?;
    Some(format!("{a}/{p}"))
}

fn addr4(s: &str) -> Option<Ipv4Addr> {
    s.split('/').next()?.trim().parse().ok()
}

pub fn excludes(cfg: &VmEgressSnat) -> Vec<String> {
    match &cfg.exclude {
        Some(x) => x.clone(),
        None => DEFAULT_EXCLUDE.iter().map(|s| s.to_string()).collect(),
    }
}

/// Reject configs that cannot be rendered at all.
pub fn validate(cfg: &VmEgressSnat) -> Result<(), String> {
    if cfg.rules.len() > MAX_RULES {
        return Err(format!("at most {MAX_RULES} egress rules"));
    }
    let sources: usize = cfg.rules.iter().map(|r| r.sources.len()).sum();
    if sources > MAX_SOURCES {
        return Err(format!("at most {MAX_SOURCES} source addresses"));
    }
    for r in &cfg.rules {
        if r.egress_ip.parse::<Ipv4Addr>().is_err() {
            return Err(format!(
                "egress IP `{}` is not an IPv4 address",
                r.egress_ip
            ));
        }
    }
    for x in excludes(cfg) {
        if cidr4(&x).is_none() {
            return Err(format!("exclude `{x}` is not an IPv4 CIDR"));
        }
    }
    Ok(())
}

/// The nftables script for `cfg` (`None` = no table), plus what was left out:
/// egress IPs not configured on this host, non-IPv4 sources, and sources
/// already claimed by an earlier rule.
pub fn render(cfg: &VmEgressSnat, local: &[Ipv4Addr]) -> (Option<String>, Vec<String>) {
    let mut skipped = Vec::new();
    let mut claimed = BTreeSet::new();
    let mut rules: Vec<(Ipv4Addr, Vec<Ipv4Addr>)> = Vec::new();
    for r in &cfg.rules {
        let label = if r.project.is_empty() {
            r.egress_ip.clone()
        } else {
            format!("project {}", r.project)
        };
        let Ok(ip) = r.egress_ip.parse::<Ipv4Addr>() else {
            skipped.push(format!("{label}: egress IP `{}` is not IPv4", r.egress_ip));
            continue;
        };
        if !local.contains(&ip) {
            skipped.push(format!(
                "{label}: {ip} is not configured on this host; add it to an interface first"
            ));
            continue;
        }
        let mut srcs = Vec::new();
        for s in &r.sources {
            match addr4(s) {
                Some(a) if claimed.insert(a) => srcs.push(a),
                Some(a) => skipped.push(format!("{label}: {a} already uses another egress IP")),
                None => skipped.push(format!("{label}: source `{s}` is not IPv4")),
            }
        }
        if !srcs.is_empty() {
            rules.push((ip, srcs));
        }
    }
    if rules.is_empty() {
        return (None, skipped);
    }
    let excl: Vec<String> = excludes(cfg).iter().filter_map(|x| cidr4(x)).collect();
    let mut out = format!("table ip {TABLE} {{\n");
    if !excl.is_empty() {
        out.push_str(&format!(
            "  set excluded {{\n    type ipv4_addr\n    flags interval\n    elements = {{ {} }}\n  }}\n",
            excl.join(", ")
        ));
    }
    out.push_str("  chain postrouting {\n    type nat hook postrouting priority srcnat - 1; policy accept;\n");
    if !excl.is_empty() {
        out.push_str("    ip daddr @excluded return\n");
    }
    for (ip, srcs) in &rules {
        let list: Vec<String> = srcs.iter().map(|a| a.to_string()).collect();
        out.push_str(&format!(
            "    ip saddr {{ {} }} snat to {ip}\n",
            list.join(", ")
        ));
    }
    out.push_str("  }\n}\n");
    (Some(out), skipped)
}

/// The whole transaction: drop the old table (creating it first so the
/// delete cannot fail), then add the new one.
pub fn transaction(script: Option<&str>) -> String {
    format!(
        "table ip {TABLE} {{}}\ndelete table ip {TABLE}\n{}",
        script.unwrap_or("")
    )
}

/// IPv4 addresses from `ip -o -4 addr show`.
pub fn local_v4(ip_output: &str) -> Vec<Ipv4Addr> {
    ip_output
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            it.find(|w| *w == "inet")?;
            addr4(it.next()?)
        })
        .collect()
}

/// One project's rule from its VMs' addresses (IPv4 only, deduplicated).
pub fn rules_for(
    project: &str,
    egress_ip: &str,
    addresses: impl IntoIterator<Item = String>,
) -> VmEgressSnatRule {
    let mut sources: Vec<String> = addresses
        .into_iter()
        .filter(|a| addr4(a).is_some())
        .collect();
    sources.sort();
    sources.dedup();
    VmEgressSnatRule {
        project: project.to_string(),
        egress_ip: egress_ip.to_string(),
        sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(p: &str, ip: &str, src: &[&str]) -> VmEgressSnatRule {
        VmEgressSnatRule {
            project: p.into(),
            egress_ip: ip.into(),
            sources: src.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn renders_one_table_with_exclusions_first() {
        let cfg = VmEgressSnat {
            rules: vec![
                rule(
                    "shop",
                    "203.0.113.10",
                    &["192.168.122.5", "192.168.122.6/32"],
                ),
                rule(
                    "lab",
                    "203.0.113.11",
                    &["192.168.122.5", "fd00::5", "192.168.122.9"],
                ),
                rule("ghost", "198.51.100.1", &["192.168.122.7"]),
            ],
            exclude: None,
        };
        let local = [
            "203.0.113.10".parse().unwrap(),
            "203.0.113.11".parse().unwrap(),
        ];
        let (s, skipped) = render(&cfg, &local);
        let s = s.unwrap();
        assert!(s.contains("elements = { 10.0.0.0/8, 172.16.0.0/12"), "{s}");
        let ret = s.find("@excluded return").unwrap();
        let first = s
            .find("ip saddr { 192.168.122.5, 192.168.122.6 } snat to 203.0.113.10")
            .unwrap();
        assert!(ret < first, "{s}");
        assert!(
            s.contains("ip saddr { 192.168.122.9 } snat to 203.0.113.11"),
            "{s}"
        );
        assert!(s.contains("priority srcnat - 1"));
        assert_eq!(skipped.len(), 3, "{skipped:?}");
        assert!(skipped
            .iter()
            .any(|x| x.contains("already uses another egress IP")));
        assert!(skipped
            .iter()
            .any(|x| x.contains("not configured on this host")));
        assert!(skipped.iter().any(|x| x.contains("fd00::5")));
    }

    #[test]
    fn empty_or_unusable_rules_mean_no_table() {
        let (s, _) = render(&VmEgressSnat::default(), &[]);
        assert!(s.is_none());
        let t = transaction(None);
        assert_eq!(
            t,
            "table ip machina_egress {}\ndelete table ip machina_egress\n"
        );
        let cfg = VmEgressSnat {
            rules: vec![rule("x", "203.0.113.10", &["192.168.122.5"])],
            exclude: Some(vec![]),
        };
        let (s, skipped) = render(&cfg, &[]);
        assert!(s.is_none() && skipped.len() == 1);
        let (s, _) = render(&cfg, &["203.0.113.10".parse().unwrap()]);
        assert!(!s.unwrap().contains("excluded"));
    }

    #[test]
    fn validation_and_ip_parsing() {
        let bad = VmEgressSnat {
            rules: vec![rule("x", "not-an-ip", &[])],
            exclude: None,
        };
        assert!(validate(&bad).is_err());
        let bad = VmEgressSnat {
            rules: vec![],
            exclude: Some(vec!["10.0.0.0/33".into()]),
        };
        assert!(validate(&bad).is_err());
        let out = "2: eth0    inet 203.0.113.10/24 brd 203.0.113.255 scope global eth0\\       valid_lft forever\n3: virbr0    inet 192.168.122.1/24 scope global virbr0\n";
        assert_eq!(
            local_v4(out),
            vec![
                "203.0.113.10".parse::<Ipv4Addr>().unwrap(),
                "192.168.122.1".parse().unwrap()
            ]
        );
        let r = rules_for(
            "p",
            "1.2.3.4",
            ["10.0.0.2".into(), "fd00::1".into(), "10.0.0.2".into()],
        );
        assert_eq!(r.sources, ["10.0.0.2"]);
    }
}

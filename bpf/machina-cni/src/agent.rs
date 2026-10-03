// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Per-node reconciler. Polls the API server through `kubectl` (k3s ships
//! one; honours `KUBECONFIG`), so there is no client-go/kube-rs dependency:
//!
//! 1. writes `/etc/cni/net.d/05-machina.conflist` from this node's podCIDR
//!    (the kubelet keeps the node NotReady until it exists);
//! 2. routes other nodes' podCIDRs via their InternalIP (direct routing) and
//!    masquerades pod traffic leaving the cluster CIDR;
//! 3. compiles NetworkPolicies / Services and syncs them into machina-bpfd.

use std::collections::BTreeSet;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use machina_bpf::api::Request;
use machina_bpf::BpfdClient;
use serde_json::{json, Value};

use crate::compile::{self, Inputs};

/// Routes we own carry this protocol number so stale ones can be found.
const ROUTE_PROTO: &str = "233";
const NFT_TABLE: &str = "machina_cni";
const SYSCTL_OVERRIDE: &str = "/etc/sysctl.d/99-zzz-machina-cni.conf";
/// NodePort replies leave a pod veth with the node address as source, which
/// the kernel drops as martian unless reverse-path filtering is off and local
/// sources are accepted. systemd-sysctl re-applies `*.rp_filter=2` to every
/// new link after the plugin ran, so the override has to live in sysctl.d.
const VETH_SYSCTLS: &[(&str, &str)] = &[("rp_filter", "0"), ("accept_local", "1")];
const RESYNC_EVERY: Duration = Duration::from_secs(30);

pub struct Config {
    pub node: String,
    pub kubectl: String,
    pub cluster_cidr: String,
    /// CNI config dirs; existing parents only, the first is always created.
    /// k3s reads its own dir, other runtimes /etc/cni/net.d.
    pub conf_dirs: Vec<String>,
    /// Where the plugin binary is installed (same rule as `conf_dirs`).
    pub bin_dirs: Vec<String>,
    pub mtu: u32,
    pub interval: Duration,
    /// Also enforce CiliumNetworkPolicy / CiliumClusterwideNetworkPolicy.
    pub cilium_policies: bool,
}

const CILIUM_RESOURCES: &str = "ciliumnetworkpolicies.cilium.io,ciliumclusterwidenetworkpolicies.cilium.io";

fn list(v: String) -> Vec<String> {
    v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

/// The first dir is always used; the rest only when their parent exists.
fn usable_dirs(dirs: &[String]) -> Vec<String> {
    dirs.iter()
        .enumerate()
        .filter(|(i, d)| *i == 0 || std::path::Path::new(d).parent().is_some_and(|p| p.is_dir()))
        .map(|(_, d)| d.clone())
        .collect()
}

/// Copy this executable into each CNI bin dir when missing or different.
fn install_plugin(bin_dirs: &[String]) {
    let Ok(me) = std::env::current_exe() else {
        return;
    };
    let Ok(bytes) = std::fs::read(&me) else {
        return;
    };
    for dir in usable_dirs(bin_dirs) {
        let dst = std::path::Path::new(&dir).join("machina-cni");
        if dst == me || std::fs::read(&dst).ok().as_deref() == Some(bytes.as_slice()) {
            continue;
        }
        let tmp = dst.with_extension("tmp");
        let res = std::fs::create_dir_all(&dir)
            .and_then(|_| std::fs::write(&tmp, &bytes))
            .and_then(|_| {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
            })
            .and_then(|_| std::fs::rename(&tmp, &dst));
        match res {
            Ok(()) => tracing::info!("installed CNI plugin {}", dst.display()),
            Err(e) => tracing::warn!("install {}: {e}", dst.display()),
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let env = |k: &str, d: &str| std::env::var(k).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| d.to_string());
        // kubelet registers the node under the lowercased hostname.
        let host = std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap_or_default().trim().to_lowercase();
        Self {
            node: env("NODE_NAME", &host),
            kubectl: env("MACHINA_CNI_KUBECTL", "kubectl"),
            cluster_cidr: env("MACHINA_CNI_CLUSTER_CIDR", "10.42.0.0/16"),
            conf_dirs: list(env(
                "MACHINA_CNI_CONF_DIRS",
                "/etc/cni/net.d,/var/lib/rancher/k3s/agent/etc/cni/net.d",
            )),
            bin_dirs: list(env(
                "MACHINA_CNI_BIN_DIRS",
                "/opt/cni/bin,/var/lib/rancher/k3s/data/current/bin",
            )),
            mtu: env("MACHINA_CNI_MTU", "1500").parse().unwrap_or(1500),
            interval: Duration::from_secs(env("MACHINA_CNI_INTERVAL_SECS", "3").parse().unwrap_or(3).max(1)),
            cilium_policies: matches!(
                env("MACHINA_CNI_CILIUM_POLICIES", "0").to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
        }
    }
}

fn kubectl(cfg: &Config, args: &[&str]) -> Result<Value> {
    let out = Command::new(&cfg.kubectl)
        .args(args)
        .output()
        .with_context(|| format!("run {}", cfg.kubectl))?;
    if !out.status.success() {
        bail!("kubectl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

fn items(v: &Value) -> Vec<Value> {
    v["items"].as_array().cloned().unwrap_or_default()
}

fn run_cmd(prog: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(prog).args(args).output().with_context(|| format!("run {prog}"))?;
    if !out.status.success() {
        bail!("{prog} {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn internal_ip(node: &Value) -> Option<String> {
    node["status"]["addresses"]
        .as_array()?
        .iter()
        .find(|a| a["type"] == "InternalIP" && a["address"].as_str().is_some_and(|s| !s.contains(':')))
        .and_then(|a| a["address"].as_str().map(String::from))
}

fn pod_cidr(node: &Value) -> Option<String> {
    node["spec"]["podCIDR"]
        .as_str()
        .map(String::from)
        .or_else(|| {
            node["spec"]["podCIDRs"]
                .as_array()?
                .iter()
                .filter_map(|c| c.as_str())
                .find(|c| !c.contains(':'))
                .map(String::from)
        })
}

/// Interface that holds `addr` (the uplink used for NodePort DNAT).
fn iface_with_addr(addr: &str) -> Option<String> {
    let out = run_cmd("ip", &["-j", "-4", "addr", "show"]).ok()?;
    let v: Value = serde_json::from_str(&out).ok()?;
    v.as_array()?.iter().find_map(|l| {
        let has = l["addr_info"].as_array()?.iter().any(|a| a["local"].as_str() == Some(addr));
        has.then(|| l["ifname"].as_str().map(String::from)).flatten()
    })
}

pub fn conflist(subnet: &str, mtu: u32) -> String {
    serde_json::to_string_pretty(&json!({
        "cniVersion": "1.0.0",
        "name": "machina",
        "plugins": [
            { "type": "machina-cni", "subnet": subnet, "mtu": mtu }
        ]
    }))
    .unwrap_or_default()
}

fn write_if_changed(path: &str, body: &str) -> Result<bool> {
    if std::fs::read_to_string(path).ok().as_deref() == Some(body) {
        return Ok(false);
    }
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(true)
}

/// Routes to other nodes' podCIDRs; removes ours that no longer apply.
fn sync_routes(want: &BTreeSet<(String, String)>) -> Result<()> {
    let out = run_cmd("ip", &["-j", "-4", "route", "show", "proto", ROUTE_PROTO])?;
    let have: Vec<Value> = serde_json::from_str(&out).unwrap_or_default();
    for r in &have {
        let dst = r["dst"].as_str().unwrap_or("");
        let gw = r["gateway"].as_str().unwrap_or("");
        if !want.contains(&(dst.to_string(), gw.to_string())) {
            let _ = run_cmd("ip", &["route", "del", dst, "proto", ROUTE_PROTO]);
        }
    }
    for (cidr, gw) in want {
        if let Err(e) = run_cmd("ip", &["route", "replace", cidr, "via", gw, "proto", ROUTE_PROTO]) {
            tracing::warn!("route {cidr} via {gw}: {e:#}");
        }
    }
    Ok(())
}

pub fn nft_rules(cluster_cidr: &str) -> String {
    format!(
        "table ip {NFT_TABLE} {{\n  chain postrouting {{\n    type nat hook postrouting priority srcnat; policy accept;\n    \
         ip saddr {cluster_cidr} ip daddr != {cluster_cidr} masquerade\n  }}\n  \
         chain forward {{\n    type filter hook forward priority filter; policy accept;\n    \
         ip saddr {cluster_cidr} accept\n    ip daddr {cluster_cidr} accept\n  }}\n}}\n"
    )
}

pub fn sysctl_override() -> String {
    let mut s = String::from("# Written by machina-cni agent.\n");
    for (k, v) in VETH_SYSCTLS {
        s.push_str(&format!("net.ipv4.conf.mc*.{k} = {v}\n"));
    }
    s
}

/// Pin the veth sysctls on existing pod veths (new ones get the sysctl.d override).
fn pin_veth_sysctls() {
    let Ok(dir) = std::fs::read_dir("/proc/sys/net/ipv4/conf") else { return };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("mc") {
            continue;
        }
        for (k, v) in VETH_SYSCTLS {
            let path = e.path().join(k);
            if std::fs::read_to_string(&path).map(|c| c.trim() != *v).unwrap_or(false) {
                let _ = std::fs::write(&path, v);
            }
        }
    }
}

fn ensure_masquerade(cluster_cidr: &str) -> Result<()> {
    let _ = std::fs::write("/proc/sys/net/ipv4/ip_forward", "1");
    let _ = run_cmd("nft", &["delete", "table", "ip", NFT_TABLE]);
    let path = std::env::temp_dir().join("machina-cni.nft");
    std::fs::write(&path, nft_rules(cluster_cidr))?;
    run_cmd("nft", &["-f", &path.display().to_string()]).map(|_| ())
}

struct NodeSetup {
    pod_cidr: String,
    node_addr: String,
}

pub async fn run(cfg: Config) -> Result<()> {
    tracing::info!(node = %cfg.node, cluster_cidr = %cfg.cluster_cidr, "machina-cni agent starting");
    let bpfd = BpfdClient::from_env();
    let mut setup: Option<NodeSetup> = None;
    let mut last_pushed: Option<(machina_bpf::api::CniState, Instant)> = None;
    let mut last_warnings: Vec<String> = Vec::new();
    let mut tick = tokio::time::interval(cfg.interval);
    loop {
        tick.tick().await;
        if let Err(e) = reconcile(&cfg, &bpfd, &mut setup, &mut last_pushed, &mut last_warnings).await {
            tracing::warn!("reconcile: {e:#}");
        }
    }
}

async fn reconcile(
    cfg: &Config,
    bpfd: &BpfdClient,
    setup: &mut Option<NodeSetup>,
    last_pushed: &mut Option<(machina_bpf::api::CniState, Instant)>,
    last_warnings: &mut Vec<String>,
) -> Result<()> {
    let list = tokio::task::block_in_place(|| {
        kubectl(
            cfg,
            &["get", "nodes,namespaces,pods,networkpolicies,services,endpointslices", "-A", "-o", "json"],
        )
    })?;
    let mut cilium_warning = None;
    let cilium_policies = if cfg.cilium_policies {
        match tokio::task::block_in_place(|| kubectl(cfg, &["get", CILIUM_RESOURCES, "-A", "-o", "json"])) {
            Ok(v) => items(&v),
            Err(e) if format!("{e:#}").contains("doesn't have a resource type") => {
                cilium_warning = Some("MACHINA_CNI_CILIUM_POLICIES is set but the cilium.io CRDs are not installed".into());
                Vec::new()
            }
            Err(e) => return Err(e.context("list Cilium policies")),
        }
    } else {
        Vec::new()
    };
    let all = items(&list);
    let of_kind = |k: &str| -> Vec<Value> { all.iter().filter(|i| i["kind"] == k).cloned().collect() };
    let nodes = of_kind("Node");
    let me = nodes
        .iter()
        .find(|n| n["metadata"]["name"].as_str() == Some(cfg.node.as_str()))
        .ok_or_else(|| anyhow!("node {} not registered yet", cfg.node))?;

    // Node bring-up: CNI config, bpfd node config, masquerade.
    let cidr = pod_cidr(me).ok_or_else(|| anyhow!("node {} has no podCIDR yet", cfg.node))?;
    let addr = internal_ip(me).ok_or_else(|| anyhow!("node {} has no InternalIP", cfg.node))?;
    let fresh = setup.as_ref().is_none_or(|s| s.pod_cidr != cidr || s.node_addr != addr);
    if fresh {
        install_plugin(&cfg.bin_dirs);
        for dir in usable_dirs(&cfg.conf_dirs) {
            let path = format!("{dir}/05-machina.conflist");
            if write_if_changed(&path, &conflist(&cidr, cfg.mtu))? {
                tracing::info!("wrote {path} (podCIDR {cidr})");
            }
        }
        ensure_masquerade(&cfg.cluster_cidr)?;
        if let Err(e) = write_if_changed(SYSCTL_OVERRIDE, &sysctl_override()) {
            tracing::warn!("write {SYSCTL_OVERRIDE}: {e:#}");
        }
        let uplink = iface_with_addr(&addr);
        bpfd.call(&Request::CniConfigure { node_addr: addr.clone(), uplink: uplink.clone() })
            .await
            .context("machina-bpfd cni_configure")?;
        tracing::info!(node_addr = %addr, uplink = ?uplink, "node datapath configured");
        *setup = Some(NodeSetup { pod_cidr: cidr.clone(), node_addr: addr.clone() });
    }

    let routes: BTreeSet<(String, String)> = nodes
        .iter()
        .filter(|n| n["metadata"]["name"].as_str() != Some(cfg.node.as_str()))
        .filter_map(|n| Some((pod_cidr(n)?, internal_ip(n)?)))
        .collect();
    tokio::task::block_in_place(|| sync_routes(&routes))?;
    pin_veth_sysctls();

    let pods = compile::pods(&of_kind("Pod"));
    let mut compiled = compile::compile(&Inputs {
        pods: &pods,
        namespaces: &of_kind("Namespace"),
        policies: &of_kind("NetworkPolicy"),
        services: &of_kind("Service"),
        endpoint_slices: &of_kind("EndpointSlice"),
        cilium_policies: &cilium_policies,
        node: &cfg.node,
    });
    compiled.warnings.extend(cilium_warning);
    if compiled.warnings != *last_warnings {
        for w in &compiled.warnings {
            tracing::warn!("{w}");
        }
        *last_warnings = compiled.warnings.clone();
    }
    let due = match last_pushed {
        Some((st, at)) => *st != compiled.state || at.elapsed() >= RESYNC_EVERY,
        None => true,
    };
    if due {
        bpfd.call(&Request::CniSync { state: compiled.state.clone() })
            .await
            .context("machina-bpfd cni_sync")?;
        tracing::debug!(
            identities = compiled.state.identities.len(),
            policy = compiled.state.policy.len(),
            services = compiled.state.services.len(),
            "synced"
        );
        *last_pushed = Some((compiled.state, Instant::now()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_fields() {
        let n = json!({
            "spec": {"podCIDRs": ["fd00::/64", "10.42.3.0/24"]},
            "status": {"addresses": [{"type": "Hostname", "address": "n3"}, {"type": "InternalIP", "address": "192.168.1.13"}]}
        });
        assert_eq!(pod_cidr(&n).as_deref(), Some("10.42.3.0/24"));
        assert_eq!(internal_ip(&n).as_deref(), Some("192.168.1.13"));
    }

    #[test]
    fn generated_config() {
        let c: Value = serde_json::from_str(&conflist("10.42.0.0/24", 1450)).unwrap();
        assert_eq!(c["plugins"][0]["type"], "machina-cni");
        assert_eq!(c["plugins"][0]["subnet"], "10.42.0.0/24");
        assert!(nft_rules("10.42.0.0/16").contains("ip saddr 10.42.0.0/16 ip daddr != 10.42.0.0/16 masquerade"));
        let sysctl = sysctl_override();
        assert!(sysctl.contains("net.ipv4.conf.mc*.rp_filter = 0"));
        assert!(sysctl.contains("net.ipv4.conf.mc*.accept_local = 1"));
    }
}

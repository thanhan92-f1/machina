// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! CNI 1.0 plugin. Pods get a veth pair, a /32 address and a default route
//! via the link-local gateway 169.254.1.1 (a permanent neighbour entry for the
//! host-side MAC), so there is no bridge and no per-node gateway address.

use std::io::Read;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use machina_bpf::api::{CniEndpoint, Request};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ipam::{Ipam, Subnet};

pub const GATEWAY: &str = "169.254.1.1";
/// Fixed host-side veth MAC. Set explicitly so udev (MACAddressPolicy=persistent)
/// does not re-randomise it after the pod's permanent gateway neighbour entry
/// was written; every veth is point-to-point, so sharing one MAC is fine.
pub const HOST_MAC: &str = "ee:ee:ee:ee:ee:ee";
const IPAM_ROOT: &str = "/var/lib/machina-cni/ipam";
const SUPPORTED: &[&str] = &["0.3.1", "0.4.0", "1.0.0"];

#[derive(Debug, Deserialize)]
struct NetConf {
    #[serde(rename = "cniVersion", default = "default_version")]
    cni_version: String,
    #[serde(default)]
    mtu: Option<u32>,
    /// Node podCIDR (written by `machina-cni agent`).
    subnet: String,
    #[serde(default)]
    ipam_dir: Option<String>,
}

fn default_version() -> String {
    "1.0.0".into()
}

struct Args {
    command: String,
    container_id: String,
    netns: String,
    ifname: String,
    pod: Option<String>,
}

impl Args {
    fn from_env() -> Result<Self> {
        let get = |k: &str| std::env::var(k).unwrap_or_default();
        let pod = get("CNI_ARGS")
            .split(';')
            .filter_map(|kv| kv.split_once('='))
            .fold((None, None), |(ns, name), (k, v)| match k {
                "K8S_POD_NAMESPACE" => (Some(v.to_string()), name),
                "K8S_POD_NAME" => (ns, Some(v.to_string())),
                _ => (ns, name),
            });
        Ok(Self {
            command: get("CNI_COMMAND"),
            container_id: get("CNI_CONTAINERID"),
            netns: get("CNI_NETNS"),
            ifname: {
                let i = get("CNI_IFNAME");
                if i.is_empty() { "eth0".into() } else { i }
            },
            pod: match pod {
                (Some(ns), Some(n)) => Some(format!("{ns}/{n}")),
                _ => None,
            },
        })
    }
}

/// Host-side veth name: `mc` + 12 hex digits of FNV-1a(container id, ifname).
pub fn host_veth(container_id: &str, ifname: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in container_id.bytes().chain([0]).chain(ifname.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("mc{:012x}", h & 0xffff_ffff_ffff)
}

fn sh(prog: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(prog)
        .args(args)
        .output()
        .with_context(|| format!("run {prog}"))?;
    if !out.status.success() {
        bail!(
            "{prog} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn ip(args: &[&str]) -> Result<String> {
    sh("ip", args)
}

fn ns_ip(netns: &str, args: &[&str]) -> Result<String> {
    let net = format!("--net={netns}");
    let mut full = vec![net.as_str(), "ip"];
    full.extend_from_slice(args);
    sh("nsenter", &full)
}

fn link_mac(json_out: &str) -> Option<String> {
    let v: Value = serde_json::from_str(json_out).ok()?;
    v.get(0)?.get("address")?.as_str().map(String::from)
}

fn emit_error(version: &str, code: u32, msg: &str) -> i32 {
    println!(
        "{}",
        json!({ "cniVersion": version, "code": code, "msg": msg })
    );
    1
}

pub fn run() -> i32 {
    let args = match Args::from_env() {
        Ok(a) => a,
        Err(e) => return emit_error("1.0.0", 4, &format!("{e:#}")),
    };
    if args.command == "VERSION" {
        println!("{}", json!({ "cniVersion": "1.0.0", "supportedVersions": SUPPORTED }));
        return 0;
    }
    let mut stdin = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut stdin) {
        return emit_error("1.0.0", 4, &format!("read config: {e}"));
    }
    let conf: NetConf = match serde_json::from_str(&stdin) {
        Ok(c) => c,
        Err(e) => return emit_error("1.0.0", 6, &format!("decode network config: {e}")),
    };
    let version = conf.cni_version.clone();
    let res = match args.command.as_str() {
        "ADD" => add(&conf, &args).map(|v| println!("{v}")),
        "DEL" => del(&conf, &args),
        "CHECK" => check(&args),
        other => Err(anyhow!("unsupported CNI_COMMAND {other}")),
    };
    match res {
        Ok(()) => 0,
        Err(e) => emit_error(&version, 999, &format!("{e:#}")),
    }
}

fn ipam(conf: &NetConf) -> Result<Ipam> {
    let root = conf.ipam_dir.as_deref().unwrap_or(IPAM_ROOT);
    Ipam::new(Path::new(root), Subnet::parse(&conf.subnet)?)
}

fn bpfd(req: Request) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    rt.block_on(machina_bpf::BpfdClient::from_env().call(&req))?;
    Ok(())
}

fn add(conf: &NetConf, a: &Args) -> Result<Value> {
    if a.netns.is_empty() || a.container_id.is_empty() {
        bail!("CNI_NETNS and CNI_CONTAINERID are required for ADD");
    }
    let ipam = ipam(conf)?;
    let addr: Ipv4Addr = ipam.allocate(&a.container_id, &a.ifname)?;
    let host = host_veth(&a.container_id, &a.ifname);
    let res = plumb(conf, a, &host, addr);
    if res.is_err() {
        let _ = ip(&["link", "del", &host]);
        ipam.release(&a.container_id, &a.ifname);
    }
    let (pod_mac, host_mac) = res?;

    // The pod is reachable through the kernel route even if bpfd is down;
    // bpfd adds same-node redirect and NetworkPolicy on top.
    if let Err(e) = bpfd(Request::CniAddEndpoint {
        endpoint: CniEndpoint {
            ip: addr.to_string(),
            host_iface: host.clone(),
            pod_mac: pod_mac.clone(),
            host_mac: host_mac.clone(),
            pod: a.pod.clone(),
        },
    }) {
        eprintln!("machina-cni: machina-bpfd registration failed (policy not enforced): {e:#}");
    }

    Ok(json!({
        "cniVersion": conf.cni_version,
        "interfaces": [
            { "name": host, "mac": host_mac },
            { "name": a.ifname, "mac": pod_mac, "sandbox": a.netns },
        ],
        "ips": [{ "address": format!("{addr}/32"), "gateway": GATEWAY, "interface": 1 }],
        "routes": [{ "dst": "0.0.0.0/0", "gw": GATEWAY }],
        "dns": {},
    }))
}

/// Create the veth pair and configure both ends. Returns (pod MAC, host MAC).
fn plumb(conf: &NetConf, a: &Args, host: &str, addr: Ipv4Addr) -> Result<(String, String)> {
    let mtu = conf.mtu.unwrap_or(1500).to_string();
    // Created inside the pod netns with the peer moved to PID 1's (host) netns.
    if ip(&["link", "show", host]).is_err() {
        ns_ip(
            &a.netns,
            &[
                "link", "add", &a.ifname, "mtu", &mtu, "type", "veth", "peer", "name", host, "address", HOST_MAC, "mtu",
                &mtu, "netns", "1",
            ],
        )?;
    }
    ip(&["link", "set", host, "address", HOST_MAC])?;
    let host_mac = link_mac(&ip(&["-j", "link", "show", host])?).ok_or_else(|| anyhow!("no MAC on {host}"))?;
    let cidr = format!("{addr}/32");
    ns_ip(&a.netns, &["link", "set", "lo", "up"])?;
    ns_ip(&a.netns, &["addr", "replace", &cidr, "dev", &a.ifname])?;
    ns_ip(&a.netns, &["link", "set", &a.ifname, "up"])?;
    ns_ip(&a.netns, &["route", "replace", GATEWAY, "dev", &a.ifname, "scope", "link"])?;
    ns_ip(&a.netns, &["route", "replace", "default", "via", GATEWAY, "dev", &a.ifname])?;
    ns_ip(
        &a.netns,
        &["neigh", "replace", GATEWAY, "lladdr", &host_mac, "dev", &a.ifname, "nud", "permanent"],
    )?;
    let pod_mac = link_mac(&ns_ip(&a.netns, &["-j", "link", "show", &a.ifname])?)
        .ok_or_else(|| anyhow!("no MAC on pod {}", a.ifname))?;

    ip(&["link", "set", host, "up"])?;
    ip(&["route", "replace", &cidr, "dev", host, "scope", "link", "proto", "static"])?;
    let _ = std::fs::write(format!("/proc/sys/net/ipv4/conf/{host}/rp_filter"), "0");
    let _ = std::fs::write(format!("/proc/sys/net/ipv4/conf/{host}/accept_local"), "1");
    Ok((pod_mac, host_mac))
}

fn del(conf: &NetConf, a: &Args) -> Result<()> {
    let host = host_veth(&a.container_id, &a.ifname);
    let released = ipam(conf).ok().and_then(|i| i.release(&a.container_id, &a.ifname));
    if let Some(addr) = released {
        let _ = ip(&["route", "del", &format!("{addr}/32"), "dev", &host]);
        if let Err(e) = bpfd(Request::CniDelEndpoint { ip: addr.to_string() }) {
            eprintln!("machina-cni: machina-bpfd unregister failed: {e:#}");
        }
    }
    // Deleting the host end removes the pair; already-gone is fine (DEL is idempotent).
    let _ = ip(&["link", "del", &host]);
    Ok(())
}

fn check(a: &Args) -> Result<()> {
    let host = host_veth(&a.container_id, &a.ifname);
    ip(&["link", "show", &host]).map(|_| ()).context("host veth missing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veth_names_fit_ifnamsiz() {
        let n = host_veth("4f1c0b5e9a7d6c3b2a1908f7e6d5c4b3a291807f6e5d4c3b2a19081726354453", "eth0");
        assert!(n.len() <= 15, "{n}");
        assert!(n.starts_with("mc"));
        assert_ne!(n, host_veth("other", "eth0"));
        assert_ne!(host_veth("c", "eth0"), host_veth("c", "net1"));
    }

    #[test]
    fn mac_from_ip_json() {
        assert_eq!(
            link_mac(r#"[{"ifindex":5,"ifname":"mc1","address":"aa:bb:cc:dd:ee:ff"}]"#).as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
        assert_eq!(link_mac("[]"), None);
    }
}

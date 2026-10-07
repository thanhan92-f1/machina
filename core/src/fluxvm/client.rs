// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Async REST client for `fluxvm-api` (`/v1/vms…`). FluxVM is UUID-keyed;
//! Machina is name-keyed, so every per-VM call goes through [`FluxvmClient::resolve`].

use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::types::{FluxCreate, FluxList, FluxMetrics, FluxRecord, FLUXVM_HYPERVISORS};
use crate::{CreateVmRequest, FluxvmConfig, LibvirtError, VmMetrics};

#[derive(Clone)]
pub struct FluxvmClient {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
    default_backend: String,
}

fn read_token(cfg: &FluxvmConfig) -> Option<String> {
    let from_file = cfg.token_file.trim();
    let tok = if from_file.is_empty() {
        cfg.token.trim().to_string()
    } else {
        std::fs::read_to_string(from_file)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    };
    Some(tok).filter(|t| !t.is_empty())
}

impl FluxvmClient {
    /// Build a client from `[fluxvm]`; `Forbidden` when the backend is disabled.
    pub fn from_config(cfg: &FluxvmConfig) -> Result<Self, LibvirtError> {
        if !cfg.enabled {
            return Err(LibvirtError::Forbidden(
                "FluxVM backend is disabled ([fluxvm] enabled = false)".into(),
            ));
        }
        let base = cfg.base_url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err(LibvirtError::Invalid("fluxvm.base_url is empty".into()));
        }
        let mut b = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(300));
        if cfg.insecure_tls {
            b = b.danger_accept_invalid_certs(true);
        }
        let http = b
            .build()
            .map_err(|e| LibvirtError::Internal(format!("fluxvm http client: {e}")))?;
        Ok(Self {
            http,
            base,
            token: read_token(cfg),
            default_backend: cfg.default_backend.trim().to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    /// `ws(s)://…/v1/vms/{id}/serial` plus the bearer token for the upgrade request.
    pub fn serial_ws(&self, id: &str) -> (String, Option<String>) {
        let ws_base = if let Some(rest) = self.base.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = self.base.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            self.base.clone()
        };
        (format!("{ws_base}/v1/vms/{id}/serial"), self.token.clone())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<(StatusCode, String), LibvirtError> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.http.request(method.clone(), &url);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(|e| {
            LibvirtError::Connection(format!("fluxvm-api {method} {path} unreachable: {e}"))
        })?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            return Ok((status, text));
        }
        Err(error_from_status(status, path, &text))
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, LibvirtError> {
        let (_, text) = self.send(method, path, body).await?;
        let text = if text.trim().is_empty() {
            "null"
        } else {
            &text
        };
        serde_json::from_str(text)
            .map_err(|e| LibvirtError::Internal(format!("fluxvm-api {path}: bad JSON: {e}")))
    }

    pub async fn health(&self) -> Result<Value, LibvirtError> {
        self.call(Method::GET, "/healthz", None).await
    }

    pub async fn capabilities(&self) -> Result<Value, LibvirtError> {
        self.call(Method::GET, "/v1/runtime/capabilities", None)
            .await
    }

    pub async fn list(&self) -> Result<Vec<FluxRecord>, LibvirtError> {
        let l: FluxList = self.call(Method::GET, "/v1/vms", None).await?;
        Ok(l.items)
    }

    /// Name → record, via the server-side `?name=` filter.
    pub async fn resolve(&self, name: &str) -> Result<FluxRecord, LibvirtError> {
        let path = format!("/v1/vms?name={}", urlencode(name));
        let l: FluxList = self.call(Method::GET, &path, None).await?;
        l.items
            .into_iter()
            .find(|r| r.name == name)
            .ok_or_else(|| LibvirtError::NotFound(format!("FluxVM VM '{name}' not found")))
    }

    pub async fn get(&self, name: &str) -> Result<FluxRecord, LibvirtError> {
        let id = self.resolve(name).await?.id;
        self.call(Method::GET, &format!("/v1/vms/{id}"), None).await
    }

    /// `start` / `stop` / `pause` / `resume` / `restart`.
    pub async fn action(&self, name: &str, verb: &str) -> Result<(), LibvirtError> {
        let id = self.resolve(name).await?.id;
        self.send(Method::POST, &format!("/v1/vms/{id}/{verb}"), None)
            .await
            .map(|_| ())
    }

    pub async fn delete(&self, name: &str) -> Result<(), LibvirtError> {
        let id = self.resolve(name).await?.id;
        self.send(Method::DELETE, &format!("/v1/vms/{id}"), None)
            .await
            .map(|_| ())
    }

    pub async fn metrics(&self, name: &str) -> Result<VmMetrics, LibvirtError> {
        let rec = self.resolve(name).await?;
        let m: FluxMetrics = self
            .call(Method::GET, &format!("/v1/vms/{}/stats", rec.id), None)
            .await?;
        Ok(m.to_vm_metrics(&rec))
    }

    pub async fn create(&self, req: &CreateVmRequest) -> Result<FluxRecord, LibvirtError> {
        let body = self.build_create(req)?;
        let body = serde_json::to_value(body)
            .map_err(|e| LibvirtError::Internal(format!("fluxvm create body: {e}")))?;
        self.call(Method::POST, "/v1/vms", Some(body)).await
    }

    pub fn build_create(&self, req: &CreateVmRequest) -> Result<FluxCreate, LibvirtError> {
        let hv = match req.fluxvm_backend.trim() {
            "" => self.default_backend.as_str(),
            other => other,
        };
        let hv = if hv.is_empty() { "auto" } else { hv };
        if !FLUXVM_HYPERVISORS.contains(&hv) {
            return Err(LibvirtError::Invalid(format!(
                "fluxvm_backend '{hv}' is not one of {}",
                FLUXVM_HYPERVISORS.join(", ")
            )));
        }
        let image = match req.fluxvm_image.trim() {
            "" => req.existing_disk.trim(),
            other => other,
        };
        if image.is_empty() {
            return Err(LibvirtError::Invalid(
                "FluxVM create needs a base image (fluxvm_image or existing_disk)".into(),
            ));
        }
        let network = build_network(req, hv)?;
        let user = req.cloud_init_user.trim();
        let key = req.cloud_init_ssh_pubkey.trim();
        // Always send a NoCloud seed: without one cloud-init never runs its default
        // network config and the guest's DHCP client never comes up.
        let cloud_init = Some(json!({
            "hostname": req.name,
            "user": Some(user).filter(|u| !u.is_empty()),
            "ssh_authorized_keys": if key.is_empty() { vec![] } else { vec![key.to_string()] },
        }));
        let vcpus = req.vcpus.clamp(1, u8::MAX as u32);
        Ok(FluxCreate {
            name: req.name.clone(),
            backend: hv.to_string(),
            image: image.to_string(),
            vcpus,
            memory_mib: req.memory_mb,
            disk_size_gib: Some(req.disk_gb).filter(|g| *g > 0),
            network,
            cloud_init,
        })
    }
}

/// Every mode except `user`/`none` gives the guest a host-visible tap, which is what
/// FluxVM's eBPF VM edge (TCX policy, identity, accounting) attaches to.
fn build_network(req: &CreateVmRequest, hv: &str) -> Result<Value, LibvirtError> {
    let invalid = |m: String| Err(LibvirtError::Invalid(m));
    let bridge = req.fluxvm_bridge.trim();
    let uplink = req.fluxvm_direct_uplink.trim();
    let mode = req.fluxvm_direct_mode.trim();
    let guest_ips = &req.fluxvm_direct_guest_ips;
    let net = req.fluxvm_network.trim();

    if uplink.is_empty() {
        if !mode.is_empty() || !guest_ips.is_empty() {
            return invalid(
                "fluxvm_direct_mode and fluxvm_direct_guest_ips require fluxvm_direct_uplink"
                    .into(),
            );
        }
    } else {
        if !bridge.is_empty() {
            return invalid("fluxvm_direct_uplink is mutually exclusive with fluxvm_bridge".into());
        }
        if !matches!(net, "" | "netns") {
            return invalid(format!(
                "fluxvm_direct_uplink cannot be combined with fluxvm_network '{net}'"
            ));
        }
        if !(1..=15).contains(&uplink.len()) || uplink.contains('/') || uplink.contains(' ') {
            return invalid(format!(
                "fluxvm_direct_uplink '{uplink}' must be an interface name (1-15 characters, no slash or space)"
            ));
        }
        let mode = if mode.is_empty() { "l2-uplink" } else { mode };
        if !matches!(mode, "l2-uplink" | "peer-veth") {
            return invalid(format!(
                "fluxvm_direct_mode must be 'l2-uplink' or 'peer-veth', got '{mode}'"
            ));
        }
        if mode != "l2-uplink" && !guest_ips.is_empty() {
            return invalid(
                "fluxvm_direct_guest_ips are only valid for fluxvm_direct_mode l2-uplink".into(),
            );
        }
        if guest_ips.len() > 8 {
            return invalid(format!(
                "fluxvm_direct_guest_ips accepts at most 8 addresses, got {}",
                guest_ips.len()
            ));
        }
        if let Some(bad) = guest_ips
            .iter()
            .find(|ip| ip.trim().parse::<std::net::Ipv4Addr>().is_err())
        {
            return invalid(format!(
                "fluxvm_direct_guest_ips entry '{bad}' is not an IPv4 address"
            ));
        }
        let ips: Vec<&str> = guest_ips.iter().map(|ip| ip.trim()).collect();
        return Ok(json!({
            "mode": "tap",
            "mac": random_mac(),
            "netns": false,
            "direct": {"outer": uplink, "mode": mode, "guest_ips": ips},
        }));
    }

    if !bridge.is_empty() {
        if !matches!(net, "" | "netns") {
            return invalid(format!(
                "fluxvm_bridge cannot be combined with fluxvm_network '{net}'"
            ));
        }
        return Ok(json!({"mode": "tap", "bridge": bridge, "mac": random_mac()}));
    }

    match net {
        // FluxVM never persists an auto-generated MAC, and netns networking refuses to
        // start without one.
        "" | "netns" => Ok(json!({"mode": "tap", "netns": true, "mac": random_mac()})),
        "user" if matches!(hv, "qemu" | "auto") => Ok(json!({"mode": "user"})),
        "user" => invalid(format!(
            "fluxvm_network 'user' is QEMU-only; '{hv}' needs netns, a bridge or a direct uplink"
        )),
        "none" => Ok(json!({"mode": "none"})),
        other => invalid(format!(
            "fluxvm_network must be 'netns', 'user' or 'none', got '{other}'"
        )),
    }
}

fn random_mac() -> String {
    let b: [u8; 3] = rand::random();
    format!("52:54:00:{:02x}:{:02x}:{:02x}", b[0], b[1], b[2])
}

fn error_from_status(status: StatusCode, path: &str, body: &str) -> LibvirtError {
    let msg = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| body.chars().take(300).collect());
    let msg = format!("fluxvm-api {path}: HTTP {}: {msg}", status.as_u16());
    match status {
        StatusCode::NOT_FOUND => LibvirtError::NotFound(msg),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LibvirtError::Forbidden(msg),
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY => {
            LibvirtError::Invalid(msg)
        }
        _ => LibvirtError::Operation(msg),
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> FluxvmClient {
        FluxvmClient::from_config(&FluxvmConfig {
            enabled: true,
            base_url: "https://flux.local:7788/".into(),
            token: "t0k".into(),
            ..FluxvmConfig::default()
        })
        .unwrap()
    }

    #[test]
    fn disabled_config_is_forbidden() {
        assert!(matches!(
            FluxvmClient::from_config(&FluxvmConfig::default()),
            Err(LibvirtError::Forbidden(_))
        ));
    }

    #[test]
    fn serial_url_uses_ws_scheme() {
        let (url, tok) = client().serial_ws("abc");
        assert_eq!(url, "wss://flux.local:7788/v1/vms/abc/serial");
        assert_eq!(tok.as_deref(), Some("t0k"));
    }

    #[test]
    fn direct_uplink_network() {
        let c = client();
        let mut req = CreateVmRequest {
            name: "vm1".into(),
            fluxvm_image: "/images/base.raw".into(),
            fluxvm_direct_uplink: "eno1".into(),
            fluxvm_direct_guest_ips: vec!["10.0.0.9".into()],
            ..Default::default()
        };
        let n = c.build_create(&req).unwrap().network;
        assert_eq!(n["mode"], "tap");
        assert_eq!(n["netns"], false);
        assert_eq!(n["direct"]["outer"], "eno1");
        assert_eq!(n["direct"]["mode"], "l2-uplink");
        assert_eq!(n["direct"]["guest_ips"][0], "10.0.0.9");

        req.fluxvm_direct_mode = "peer-veth".into();
        assert!(c.build_create(&req).is_err());
        req.fluxvm_direct_guest_ips.clear();
        assert_eq!(
            c.build_create(&req).unwrap().network["direct"]["mode"],
            "peer-veth"
        );
        req.fluxvm_bridge = "br0".into();
        assert!(c.build_create(&req).is_err());
        req.fluxvm_bridge.clear();
        req.fluxvm_direct_uplink = "bad/name".into();
        assert!(c.build_create(&req).is_err());
        req.fluxvm_direct_uplink.clear();
        assert!(c.build_create(&req).is_err());
    }

    #[test]
    fn create_body_defaults_and_validation() {
        let c = client();
        let mut req = CreateVmRequest {
            name: "vm1".into(),
            vcpus: 2,
            memory_mb: 1024,
            disk_gb: 10,
            existing_disk: "/images/base.qcow2".into(),
            ..Default::default()
        };
        let b = c.build_create(&req).unwrap();
        assert_eq!(b.backend, "auto");
        assert_eq!(b.image, "/images/base.qcow2");
        assert_eq!(b.network["mode"], "tap");
        assert_eq!(b.network["netns"], true);
        assert!(b.network["mac"].as_str().unwrap().starts_with("52:54:00:"));
        assert_eq!(b.cloud_init.as_ref().unwrap()["hostname"], "vm1");

        req.fluxvm_backend = "firecracker".into();
        req.fluxvm_bridge = "vmbr0".into();
        req.cloud_init_ssh_pubkey = "ssh-ed25519 AAAA".into();
        let b = c.build_create(&req).unwrap();
        assert_eq!(b.backend, "firecracker");
        assert_eq!(b.network["bridge"], "vmbr0");
        assert_eq!(
            b.cloud_init.unwrap()["ssh_authorized_keys"][0],
            "ssh-ed25519 AAAA"
        );

        req.fluxvm_backend = "flux-vm".into();
        req.fluxvm_bridge.clear();
        req.fluxvm_network = "user".into();
        assert!(c.build_create(&req).is_err());
        req.fluxvm_network = "none".into();
        assert_eq!(c.build_create(&req).unwrap().network["mode"], "none");
        req.fluxvm_network.clear();

        req.fluxvm_backend = "xen".into();
        assert!(c.build_create(&req).is_err());
        req.fluxvm_backend.clear();
        req.existing_disk.clear();
        assert!(c.build_create(&req).is_err());
    }

    #[test]
    fn error_mapping() {
        assert!(matches!(
            error_from_status(StatusCode::NOT_FOUND, "/v1/vms/x", r#"{"error":"no such vm"}"#),
            LibvirtError::NotFound(m) if m.contains("no such vm")
        ));
        assert!(matches!(
            error_from_status(StatusCode::UNAUTHORIZED, "/v1/vms", ""),
            LibvirtError::Forbidden(_)
        ));
        assert!(matches!(
            error_from_status(StatusCode::INTERNAL_SERVER_ERROR, "/v1/vms", "boom"),
            LibvirtError::Operation(_)
        ));
    }

    #[test]
    fn urlencode_names() {
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
        assert_eq!(urlencode("vm-1_x.y"), "vm-1_x.y");
    }
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

use anyhow::Result;
use machina_core::format_http_error_body;
use machina_core::libvirt::extras::BrowseDirResponse;
use machina_core::{
    BackupInfo, BackupRequest, CloneVmRequest, CreateNetworkRequest, CreateSnapshotRequest,
    NetworkInfo, NodeInfo, RenameVmRequest, RestoreRequest, SnapshotInfo, StoragePoolInfo,
    VmDetails, VmInfo, VmMetrics,
};

pub struct DaemonClient {
    base_url: String,
    client: reqwest::Client,
}

// Bounded timeouts (see `DaemonClient::new`) so the TUI's single-threaded event loop
// can't hang forever waiting on a request; connect_timeout is shorter since a dead
// host should fail fast, while an in-flight request gets more slack to complete.
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;

impl DaemonClient {
    pub fn new(base_url: &str) -> Self {
        // Self-signed certs from install.sh are normal; trust for local admin tool (same as curl -k).
        // A bounded timeout keeps the TUI's single-threaded event loop from hanging forever
        // (freezing rendering and input) if the daemon becomes unreachable mid-request.
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .build()
            .expect("reqwest client");
        Self {
            base_url: base_url.to_string(),
            client,
        }
    }

    // ── Unified HTTP helpers ────────────────────────────────────────────

    fn http_error(status: reqwest::StatusCode, body: &str) -> anyhow::Error {
        let reason = status.canonical_reason().unwrap_or("");
        anyhow::anyhow!(format_http_error_body(status.as_u16(), reason, body,))
    }

    // Sends an already-built request and turns a non-2xx response into a formatted
    // error (reusing the body text). Every verb helper below funnels through this so
    // the success/error split stays identical across GET/POST/PUT/DELETE call sites.
    async fn send_checked(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let resp = req.send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Self::http_error(status, &body));
        }
        Ok(resp)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.send_checked(self.client.get(&url)).await?;
        Ok(resp.json().await?)
    }

    async fn get_text(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.send_checked(self.client.get(&url)).await?;
        Ok(resp.text().await?)
    }

    async fn post_action(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.send_checked(self.client.post(&url)).await?;
        Ok(())
    }

    async fn post_json<T: serde::Serialize>(&self, path: &str, body: &T) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.send_checked(self.client.post(&url).json(body)).await?;
        Ok(())
    }

    async fn delete_action(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.send_checked(self.client.delete(&url)).await?;
        Ok(())
    }

    async fn put_json<T: serde::Serialize>(&self, path: &str, body: &T) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.send_checked(self.client.put(&url).json(body)).await?;
        Ok(())
    }

    fn error_suggests_nvram_undefine_needed(msg: &str) -> bool {
        let m = msg.to_lowercase();
        m.contains("nvram") && (m.contains("undefine") || m.contains("cannot remove domain"))
    }

    // ── VMs ─────────────────────────────────────────────────────────────

    pub async fn fetch_vms(&self) -> Result<Vec<VmInfo>> {
        self.get_json("/api/v1/vms").await
    }

    pub async fn get_vm_details(&self, name: &str) -> Result<VmDetails> {
        self.get_json(&format!("/api/v1/vms/{name}")).await
    }

    pub async fn get_vm_xml(&self, name: &str) -> Result<String> {
        self.get_text(&format!("/api/v1/vms/{name}/xml")).await
    }

    pub async fn start_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/start")).await
    }

    pub async fn stop_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/stop")).await
    }

    pub async fn shutdown_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/shutdown"))
            .await
    }

    pub async fn reboot_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/reboot"))
            .await
    }

    pub async fn pause_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/pause")).await
    }

    pub async fn resume_vm(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/resume"))
            .await
    }

    pub async fn delete_vm(&self, name: &str) -> Result<()> {
        let path = format!("/api/v1/vms/{name}");
        match self.delete_action(&path).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                if Self::error_suggests_nvram_undefine_needed(&msg) {
                    let retry = format!("{path}?undefine_nvram=true");
                    self.delete_action(&retry).await
                } else {
                    Err(e)
                }
            }
        }
    }

    pub async fn clone_vm(&self, source: &str, new_name: &str) -> Result<()> {
        let req = CloneVmRequest {
            new_name: new_name.to_string(),
            clone_mode: "linked".into(),
        };
        self.post_json(&format!("/api/v1/vms/{source}/clone"), &req)
            .await
    }

    pub async fn rename_vm(&self, name: &str, new_name: &str) -> Result<()> {
        let req = RenameVmRequest {
            new_name: new_name.to_string(),
        };
        self.post_json(&format!("/api/v1/vms/{name}/rename"), &req)
            .await
    }

    pub async fn set_autostart(&self, name: &str, enabled: bool) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/autostart/{enabled}"))
            .await
    }

    pub async fn set_vcpus(&self, name: &str, count: u32) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/vcpus/{count}"))
            .await
    }

    pub async fn set_memory(&self, name: &str, mb: u64) -> Result<()> {
        self.post_action(&format!("/api/v1/vms/{name}/memory/{mb}"))
            .await
    }

    // ── Networks ────────────────────────────────────────────────────────

    pub async fn fetch_networks(&self) -> Result<Vec<NetworkInfo>> {
        self.get_json("/api/v1/networks").await
    }

    pub async fn start_network(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/networks/{name}/start"))
            .await
    }

    pub async fn stop_network(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/networks/{name}/stop"))
            .await
    }

    pub async fn create_network(&self, req: &CreateNetworkRequest) -> Result<()> {
        self.post_json("/api/v1/networks", req).await
    }

    pub async fn delete_network(&self, name: &str) -> Result<()> {
        self.delete_action(&format!("/api/v1/networks/{name}"))
            .await
    }

    pub async fn set_network_autostart(&self, name: &str, enabled: bool) -> Result<()> {
        self.post_action(&format!("/api/v1/networks/{name}/autostart/{enabled}"))
            .await
    }

    pub async fn get_network_xml(&self, name: &str) -> Result<String> {
        self.get_text(&format!("/api/v1/networks/{name}/xml")).await
    }

    // ── Storage ─────────────────────────────────────────────────────────

    pub async fn fetch_storage_pools(&self) -> Result<Vec<StoragePoolInfo>> {
        self.get_json("/api/v1/storage/pools").await
    }

    pub async fn start_pool(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/storage/pools/{name}/start"))
            .await
    }

    pub async fn stop_pool(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/storage/pools/{name}/stop"))
            .await
    }

    pub async fn fetch_volumes(&self, pool: &str) -> Result<Vec<machina_core::StorageVolumeInfo>> {
        self.get_json(&format!("/api/v1/storage/pools/{pool}/volumes"))
            .await
    }

    pub async fn set_pool_autostart(&self, name: &str, enabled: bool) -> Result<()> {
        self.post_action(&format!("/api/v1/storage/pools/{name}/autostart/{enabled}"))
            .await
    }

    pub async fn refresh_pool(&self, name: &str) -> Result<()> {
        self.post_action(&format!("/api/v1/storage/pools/{name}/refresh"))
            .await
    }

    pub async fn delete_volume(&self, pool: &str, vol: &str) -> Result<()> {
        self.delete_action(&format!("/api/v1/storage/pools/{pool}/volumes/{vol}"))
            .await
    }

    // ── Snapshots ───────────────────────────────────────────────────────

    pub async fn fetch_all_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        self.get_json("/api/v1/snapshots").await
    }

    pub async fn create_snapshot(&self, vm_name: &str, snap_name: &str, desc: &str) -> Result<()> {
        let req = CreateSnapshotRequest {
            name: snap_name.to_string(),
            description: desc.to_string(),
            disk_only: false,
            storage_mode: String::new(),
            memory_snapshot: String::new(),
            memory_file: String::new(),
            external_disk_dir: String::new(),
            external_memory_dir: String::new(),
            disks: Vec::new(),
            atomic: true,
            reuse_external: false,
            quiesce: false,
        };
        self.post_json(&format!("/api/v1/vms/{vm_name}/snapshots"), &req)
            .await
    }

    pub async fn delete_snapshot(&self, vm_name: &str, snap_name: &str) -> Result<()> {
        self.delete_action(&format!("/api/v1/vms/{vm_name}/snapshots/{snap_name}"))
            .await
    }

    pub async fn revert_snapshot(&self, vm_name: &str, snap_name: &str) -> Result<()> {
        self.post_action(&format!(
            "/api/v1/vms/{vm_name}/snapshots/{snap_name}/revert"
        ))
        .await
    }

    // ── Node / Metrics ──────────────────────────────────────────────────

    pub async fn fetch_node_info(&self) -> Result<NodeInfo> {
        self.get_json("/api/v1/node").await
    }

    pub async fn fetch_metrics(&self) -> Result<Vec<VmMetrics>> {
        self.get_json("/api/v1/metrics").await
    }

    // ── Console ─────────────────────────────────────────────────────────

    pub async fn get_console_info(&self, name: &str) -> Result<serde_json::Value> {
        self.get_json(&format!("/api/v1/vms/console-info/{name}"))
            .await
    }

    // ── Backups ─────────────────────────────────────────────────────────

    pub async fn fetch_backups(&self) -> Result<Vec<BackupInfo>> {
        self.get_json("/api/v1/backups").await
    }

    pub async fn trigger_backup(&self, req: &BackupRequest) -> Result<()> {
        self.post_json("/api/v1/backups", req).await
    }

    pub async fn restore_backup(&self, backup_id: &str) -> Result<()> {
        let req = RestoreRequest {
            backup_id: backup_id.to_string(),
        };
        self.post_json("/api/v1/backups/restore", &req).await
    }

    pub async fn delete_backup(&self, id: &str) -> Result<()> {
        self.delete_action(&format!("/api/v1/backups/{id}")).await
    }

    // ── Host browse / KubeVirt ─────────────────────────────────────────

    pub async fn browse_directory(&self, path: &str) -> Result<BrowseDirResponse> {
        let url = format!("{}/api/v1/browse/dir", self.base_url);
        let req = if path.trim().is_empty() {
            self.client.get(&url)
        } else {
            self.client.get(&url).query(&[("path", path)])
        };
        let resp = self.send_checked(req).await?;
        Ok(resp.json().await?)
    }

    pub async fn get_kubevirt_bundle_yaml(&self, vm: &str) -> Result<String> {
        let v: serde_json::Value = self
            .get_json(&format!("/api/v1/vms/{vm}/kubevirt-bundle"))
            .await?;
        v.get("yaml")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("kubevirt-bundle response missing yaml"))
    }

    /// `op`: `apply` | `upload` | `start` — POST body is JSON overrides (same keys as kubevirt-bundle query).
    pub async fn kubevirt_cluster_exec(
        &self,
        vm: &str,
        op: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let path = match op {
            "apply" => "apply",
            "upload" => "upload",
            "start" => "start",
            _ => anyhow::bail!("unknown kubevirt op '{op}' (expected apply, upload, start)"),
        };
        let url = format!("{}/api/v1/vms/{}/kubevirt/{}", self.base_url, vm, path);
        let resp = self.send_checked(self.client.post(&url).json(body)).await?;
        let text = resp.text().await.unwrap_or_default();
        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("invalid JSON from daemon: {e}; body: {text}"))
    }
}

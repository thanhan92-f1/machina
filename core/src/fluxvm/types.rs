// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Minimal serde mirrors of the `fluxvm-api` wire types. Every field is
//! defaulted and unknown fields are ignored so FluxVM schema additions never
//! break listing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{DiskInfo, InterfaceInfo, VmDetails, VmInfo, VmMetrics};

pub const BACKEND_NAME: &str = "fluxvm";

/// FluxVM hypervisors accepted in `CreateVmRequest.backend` (kebab-case on the wire).
pub const FLUXVM_HYPERVISORS: &[&str] =
    &["auto", "qemu", "cloud-hypervisor", "firecracker", "flux-vm"];

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FluxRecord {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub disk: String,
    #[serde(default)]
    pub tap_name: Option<String>,
    #[serde(default)]
    pub guest_ip: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub request: FluxRequestView,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FluxRequestView {
    #[serde(default)]
    pub vcpus: u32,
    #[serde(default)]
    pub memory_mib: u64,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub network: Value,
    #[serde(default)]
    pub data_disks: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FluxList {
    #[serde(default)]
    pub items: Vec<FluxRecord>,
}

/// Body for `POST /v1/vms`. Only the fields Machina exposes; FluxVM defaults the rest.
#[derive(Debug, Clone, Serialize)]
pub struct FluxCreate {
    pub name: String,
    pub backend: String,
    pub image: String,
    pub vcpus: u32,
    pub memory_mib: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_size_gib: Option<u64>,
    pub network: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cloud_init: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FluxMetrics {
    #[serde(default)]
    pub cpu_usage_percent: f64,
    #[serde(default)]
    pub memory_usage_bytes: u64,
    #[serde(default)]
    pub disk_read_bytes: u64,
    #[serde(default)]
    pub disk_write_bytes: u64,
}

impl FluxMetrics {
    /// Map onto Machina's per-VM metrics shape. FluxVM reports lifetime-average
    /// CPU % rather than cumulative CPU time, so `cpu_time_ns` stays 0.
    pub fn to_vm_metrics(&self, rec: &FluxRecord) -> VmMetrics {
        let total = rec.request.memory_mib;
        let used = self.memory_usage_bytes / (1024 * 1024);
        let state = map_status(&rec.status);
        VmMetrics {
            name: rec.name.clone(),
            running: state == "running",
            state,
            cpu_time_ns: 0,
            vcpus: rec.request.vcpus,
            memory_total_mb: total,
            memory_used_mb: used,
            memory_pct: if total > 0 {
                (used as f64 / total as f64 * 100.0).min(100.0)
            } else {
                0.0
            },
            disk_rd_bytes: self.disk_read_bytes,
            disk_wr_bytes: self.disk_write_bytes,
            disk_rd_ops: 0,
            disk_wr_ops: 0,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            vcpus_detail: Vec::new(),
            disks: Vec::new(),
            nets: Vec::new(),
            cgroup: None,
            libvirt_connection: None,
        }
    }
}

/// FluxVM `VmStatus` (lowercase) → the libvirt state strings the UI already understands.
pub fn map_status(status: &str) -> String {
    match status {
        "running" => "running",
        "paused" => "paused",
        "stopped" => "shutoff",
        "creating" => "creating",
        "failed" => "crashed",
        "" => "no state",
        other => other,
    }
    .to_string()
}

fn vcpus(r: &FluxRecord) -> u32 {
    r.request.vcpus
}

impl FluxRecord {
    pub fn to_vm_info(&self) -> VmInfo {
        let guest_ips: Vec<String> = self.guest_ip.iter().cloned().collect();
        VmInfo {
            name: self.name.clone(),
            state: map_status(&self.status),
            vcpus: vcpus(self),
            memory_mb: self.request.memory_mib,
            libvirt_connection: None,
            guest_ip: self.guest_ip.clone(),
            guest_ips,
            backend: Some(BACKEND_NAME.to_string()),
            fluxvm_backend: Some(self.backend.clone()).filter(|b| !b.is_empty()),
        }
    }

    pub fn to_vm_details(&self) -> VmDetails {
        let mut disks = Vec::new();
        if !self.disk.is_empty() {
            disks.push(DiskInfo {
                device: "disk".into(),
                source: self.disk.clone(),
                driver: if self.disk.ends_with(".qcow2") {
                    "qcow2".into()
                } else {
                    "raw".into()
                },
                target: "vda".into(),
                bus: "virtio".into(),
                cache: String::new(),
                readonly: false,
                shareable: false,
                capacity_bytes: None,
                allocation_bytes: None,
                physical_bytes: None,
            });
        }
        let mode = self
            .request
            .network
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_string();
        let mut interfaces = Vec::new();
        if mode != "none" {
            let net = &self.request.network;
            let tap = self.tap_name.as_deref().unwrap_or("tap");
            let source = if let Some(outer) = net.pointer("/direct/outer").and_then(Value::as_str) {
                format!("direct {outer} → {tap} (eBPF)")
            } else if net.get("netns").and_then(Value::as_bool) == Some(true) {
                format!("netns {tap} (eBPF)")
            } else if let Some(bridge) = net.get("bridge").and_then(Value::as_str) {
                format!("{bridge} · {tap}")
            } else {
                mode.clone()
            };
            interfaces.push(InterfaceInfo {
                mac_address: self
                    .request
                    .network
                    .get("mac")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                source,
                model: "virtio".into(),
                ip: self.guest_ip.clone(),
            });
        }
        VmDetails {
            name: self.name.clone(),
            uuid: self.id.clone(),
            state: map_status(&self.status),
            vcpus: vcpus(self),
            memory_mb: self.request.memory_mib,
            os_type: "hvm".into(),
            arch: std::env::consts::ARCH.into(),
            autostart: false,
            persistent: true,
            interfaces,
            disks,
            filesystems: Vec::new(),
            libvirt_connection: None,
            guest_ip: self.guest_ip.clone(),
            backend: Some(BACKEND_NAME.to_string()),
            fluxvm_backend: Some(self.backend.clone()).filter(|b| !b.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> FluxRecord {
        serde_json::from_value(json!({
            "id": "5f0c1c3e-8a52-4a59-9d7c-0d6f1a1b2c3d",
            "name": "edge-1",
            "backend": "cloud-hypervisor",
            "status": "running",
            "pid": 1234,
            "disk": "/var/lib/fluxvm/vms/x/disk.qcow2",
            "tap_name": "fvtap0",
            "guest_ip": "10.0.0.5",
            "some_future_field": {"nested": true},
            "request": {
                "name": "edge-1",
                "backend": "cloud-hypervisor",
                "image": "/images/ubuntu.qcow2",
                "vcpus": 2,
                "memory_mib": 2048,
                "network": {"mode": "tap", "bridge": "vmbr0", "mac": "52:54:00:aa:bb:cc"}
            }
        }))
        .unwrap()
    }

    #[test]
    fn status_maps_to_libvirt_strings() {
        assert_eq!(map_status("running"), "running");
        assert_eq!(map_status("paused"), "paused");
        assert_eq!(map_status("stopped"), "shutoff");
        assert_eq!(map_status("failed"), "crashed");
        assert_eq!(map_status("creating"), "creating");
    }

    #[test]
    fn record_maps_to_vm_info() {
        let info = sample().to_vm_info();
        assert_eq!(info.name, "edge-1");
        assert_eq!(info.state, "running");
        assert_eq!(info.vcpus, 2);
        assert_eq!(info.memory_mb, 2048);
        assert_eq!(info.backend.as_deref(), Some("fluxvm"));
        assert_eq!(info.fluxvm_backend.as_deref(), Some("cloud-hypervisor"));
        assert_eq!(info.guest_ips, vec!["10.0.0.5".to_string()]);
    }

    #[test]
    fn record_maps_to_vm_details() {
        let d = sample().to_vm_details();
        assert_eq!(d.uuid, "5f0c1c3e-8a52-4a59-9d7c-0d6f1a1b2c3d");
        assert_eq!(d.disks.len(), 1);
        assert_eq!(d.disks[0].driver, "qcow2");
        assert_eq!(d.interfaces.len(), 1);
        assert_eq!(d.interfaces[0].source, "vmbr0 · fvtap0");
        assert_eq!(d.interfaces[0].mac_address, "52:54:00:aa:bb:cc");
    }

    #[test]
    fn metrics_map_memory_pct() {
        let m = FluxMetrics {
            cpu_usage_percent: 12.5,
            memory_usage_bytes: 1024 * 1024 * 1024,
            disk_read_bytes: 7,
            disk_write_bytes: 9,
        };
        let vm = m.to_vm_metrics(&sample());
        assert!(vm.running);
        assert_eq!(vm.memory_used_mb, 1024);
        assert!((vm.memory_pct - 50.0).abs() < 1e-9);
        assert_eq!(vm.disk_wr_bytes, 9);
    }

    #[test]
    fn minimal_record_parses() {
        let r: FluxRecord =
            serde_json::from_value(json!({"id": "x", "name": "y", "status": "stopped"})).unwrap();
        let info = r.to_vm_info();
        assert_eq!(info.state, "shutoff");
        assert_eq!(info.fluxvm_backend, None);
        assert!(r.to_vm_details().interfaces.is_empty());
    }
}

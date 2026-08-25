// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Resolve libvirt guest disks for OpenStack Glance upload.

use crate::kubevirt::pick_root_boot_disk;
use crate::state::{DiskInfo, VmDetails};
use crate::LibvirtError;

use super::glance::GlanceUploadPreview;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibvirtOpenStackPushPreview {
    pub vm_name: String,
    pub vm_state: String,
    pub root_disk: String,
    pub glance_preview: GlanceUploadPreview,
    /// True when the guest is running — stop before upload to avoid qcow2 write locks.
    pub vm_running: bool,
}

/// Build push preview from libvirt VM details and on-disk root path.
pub fn libvirt_openstack_push_preview(
    vm_name: &str,
    details: &VmDetails,
) -> Result<LibvirtOpenStackPushPreview, LibvirtError> {
    let disk = pick_root_boot_disk(details)?;
    let root = disk.source.trim().to_string();
    if !root.starts_with('/') {
        return Err(LibvirtError::Invalid(format!(
            "OpenStack upload needs an absolute disk path on the hypervisor, got: {root}"
        )));
    }
    let preview = super::glance::preview_qcow2_upload(&root)?;
    let running = details.state.eq_ignore_ascii_case("running")
        || details.state.eq_ignore_ascii_case("paused");
    Ok(LibvirtOpenStackPushPreview {
        vm_name: vm_name.to_string(),
        vm_state: details.state.clone(),
        root_disk: root,
        glance_preview: preview,
        vm_running: running,
    })
}

/// Exported for tests / callers that need only the path.
pub fn libvirt_root_disk_path(details: &VmDetails) -> Result<String, LibvirtError> {
    Ok(pick_root_boot_disk(details)?.source.trim().to_string())
}

pub fn is_supported_upload_disk(disk: &DiskInfo) -> bool {
    let s = disk.source.to_lowercase();
    s.ends_with(".qcow2") || s.ends_with(".raw") || s.ends_with(".img")
}

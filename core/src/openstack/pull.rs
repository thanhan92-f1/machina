// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Download Glance images to on-host qcow2 paths (OpenStack → libvirt pull).

use std::path::Path;
use std::time::Duration;

use osauth::services::IMAGE;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

use crate::config::OpenStackConfig;
use crate::LibvirtError;

use super::auth::{connect_session, map_osauth_err};
use super::glance::{GlancePullRequest, GlancePullResult};
use serde::Deserialize;

#[derive(Deserialize)]
struct ImageStatus {
    status: String,
    #[serde(default)]
    name: String,
}

fn validate_dest(dest: &str, allowed_prefixes: &[String]) -> Result<(), LibvirtError> {
    let dest = dest.trim();
    if dest.is_empty() {
        return Err(LibvirtError::Invalid("dest_path is required".into()));
    }
    if !dest.starts_with('/') {
        return Err(LibvirtError::Invalid("dest_path must be absolute".into()));
    }
    if dest.contains("..") {
        return Err(LibvirtError::Invalid("path traversal not allowed".into()));
    }
    let lower = dest.to_lowercase();
    if !(lower.ends_with(".qcow2") || lower.ends_with(".raw") || lower.ends_with(".img")) {
        return Err(LibvirtError::Invalid(
            "dest_path must end with .qcow2, .raw, or .img".into(),
        ));
    }
    if !allowed_prefixes.iter().any(|p| dest.starts_with(p)) {
        return Err(LibvirtError::Invalid(format!(
            "dest_path not in an allowed images directory: {dest}"
        )));
    }
    Ok(())
}

async fn wait_glance_active(
    session: &osauth::Session,
    image_id: &str,
    timeout: Duration,
) -> Result<ImageStatus, LibvirtError> {
    let start = std::time::Instant::now();
    loop {
        let img: ImageStatus = session
            .get(IMAGE, ["images", image_id])
            .fetch()
            .await
            .map_err(map_osauth_err)?;
        let st = img.status.to_lowercase();
        if st == "active" {
            return Ok(img);
        }
        if st == "killed" || st == "deleted" {
            return Err(LibvirtError::Operation(format!(
                "Glance image {image_id} entered status {st}"
            )));
        }
        if start.elapsed() > timeout {
            return Err(LibvirtError::Operation(format!(
                "timed out waiting for Glance image {image_id} to become active (last: {st})"
            )));
        }
        sleep(Duration::from_secs(5)).await;
    }
}

/// Stream a Glance image to a local file (OpenStack → hypervisor disk path).
pub async fn pull_glance_image_to_disk(
    cfg: &OpenStackConfig,
    image_id: &str,
    req: &GlancePullRequest,
    allowed_prefixes: &[String],
) -> Result<GlancePullResult, LibvirtError> {
    let id = image_id.trim();
    if id.is_empty() {
        return Err(LibvirtError::Invalid("image_id is required".into()));
    }
    let dest = req.dest_path.trim();
    validate_dest(dest, allowed_prefixes)?;

    if let Some(parent) = Path::new(dest).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| LibvirtError::Operation(format!("create parent dir: {e}")))?;
    }

    let session = connect_session(cfg).await?;
    let wait = req.wait_for_active.unwrap_or(true);
    let meta = if wait {
        wait_glance_active(
            &session,
            id,
            Duration::from_secs(cfg.upload_timeout_secs.max(60)),
        )
        .await?
    } else {
        session
            .get(IMAGE, ["images", id])
            .fetch()
            .await
            .map_err(map_osauth_err)?
    };

    let mut resp = session
        .get(IMAGE, ["images", id, "file"])
        .send()
        .await
        .map_err(map_osauth_err)?;

    if !resp.status().is_success() {
        return Err(LibvirtError::Operation(format!(
            "Glance image download failed: HTTP {}",
            resp.status()
        )));
    }

    let mut file = File::create(dest)
        .await
        .map_err(|e| LibvirtError::Operation(format!("create {dest}: {e}")))?;
    let mut bytes_written: u64 = 0;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| LibvirtError::Operation(format!("download chunk: {e}")))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| LibvirtError::Operation(format!("write {dest}: {e}")))?;
        bytes_written += chunk.len() as u64;
    }
    file.flush()
        .await
        .map_err(|e| LibvirtError::Operation(format!("flush {dest}: {e}")))?;

    Ok(GlancePullResult {
        image_id: id.to_string(),
        image_name: meta.name,
        dest_path: dest.to_string(),
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::validate_dest;

    #[test]
    fn dest_must_be_under_allowed_prefix() {
        let ok = validate_dest(
            "/var/lib/libvirt/images/vm.qcow2",
            &["/var/lib/libvirt/images".into()],
        );
        assert!(ok.is_ok());
        let bad = validate_dest("/tmp/evil.qcow2", &["/var/lib/libvirt/images".into()]);
        assert!(bad.is_err());
    }
}

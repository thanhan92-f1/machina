// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! FluxVM backend glue: client from `[fluxvm]`, fail-soft listing for the merged
//! VM list, and `GET /api/v1/fluxvm/status` for the UI.

use axum::routing::get;
use axum::{Json, Router};
use machina_core::fluxvm::FluxvmClient;
use machina_core::{LibvirtManager, MachinaConfig, VmInfo};
use serde_json::{json, Value};

use crate::error::AppError;

pub(crate) fn client() -> Result<FluxvmClient, AppError> {
    FluxvmClient::from_config(&MachinaConfig::load().fluxvm).map_err(AppError::from)
}

/// FluxVM VMs for the merged list; empty (with a warning) when disabled or unreachable.
pub(crate) async fn list_vm_infos() -> Vec<VmInfo> {
    let cfg = MachinaConfig::load().fluxvm;
    if !cfg.enabled {
        return Vec::new();
    }
    let c = match FluxvmClient::from_config(&cfg) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("fluxvm: {e}");
            return Vec::new();
        }
    };
    match c.list().await {
        Ok(items) => items.iter().map(|r| r.to_vm_info()).collect(),
        Err(e) => {
            tracing::warn!("fluxvm: list failed, showing libvirt VMs only: {e}");
            Vec::new()
        }
    }
}

/// Merge FluxVM VMs into a libvirt list; libvirt wins on a name clash.
pub(crate) async fn merge_into(mut vms: Vec<VmInfo>) -> Vec<VmInfo> {
    let flux = list_vm_infos().await;
    for f in flux {
        if !vms.iter().any(|v| v.name == f.name) {
            vms.push(f);
        }
    }
    vms
}

async fn fluxvm_status() -> Json<Value> {
    let cfg = MachinaConfig::load().fluxvm;
    let mut out = json!({
        "enabled": cfg.enabled,
        "base_url": cfg.base_url,
        "default_backend": cfg.default_backend,
        "reachable": false,
    });
    if !cfg.enabled {
        return Json(out);
    }
    let c = match FluxvmClient::from_config(&cfg) {
        Ok(c) => c,
        Err(e) => {
            out["last_error"] = e.to_string().into();
            return Json(out);
        }
    };
    match c.health().await {
        Ok(_) => {
            out["reachable"] = true.into();
            if let Ok(caps) = c.capabilities().await {
                out["capabilities"] = caps;
            }
        }
        Err(e) => out["last_error"] = e.to_string().into(),
    }
    Json(out)
}

pub fn fluxvm_routes() -> Router<LibvirtManager> {
    Router::new().route("/fluxvm/status", get(fluxvm_status))
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Runtime OpenStack cloud override (session-only, not persisted).

use std::sync::RwLock;

use machina_core::config::OpenStackConfig;
use machina_core::MachinaConfig;

static CLOUD_OVERRIDE: RwLock<Option<String>> = RwLock::new(None);

pub fn set_cloud_override(name: Option<String>) {
    let mut g = CLOUD_OVERRIDE.write().unwrap_or_else(|e| e.into_inner());
    *g = name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
}

pub fn cloud_override() -> Option<String> {
    CLOUD_OVERRIDE.read().ok().and_then(|g| g.clone())
}

pub fn openstack_cfg() -> OpenStackConfig {
    let mut cfg = MachinaConfig::load().openstack;
    if let Some(name) = cloud_override() {
        cfg.cloud_name = name;
        cfg.auth_url.clear();
    }
    cfg
}

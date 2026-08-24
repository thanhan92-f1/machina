// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

mod apply;
mod install;
mod types;

pub use apply::{apply_security_bundle, security_fabric_status};
pub use install::{render_install_script, run_tetragon_install};
pub use types::{
    SecurityBundleApplyResult, SecurityFabricStatus, TetragonInstallResult, TetragonInstallSpec,
};

/// Root directory for rendered install scripts, policy JSON, and export state.
/// Shared by `apply` and `install` (was duplicated identically in both). Not
/// `pub`: child modules already see private ancestor items in Rust, so this
/// only needs to be reachable as `super::policy_dir()` from `apply`/`install`.
fn policy_dir() -> std::path::PathBuf {
    std::env::var("MACHINA_TETRAGON_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/var/lib/machina/tetragon"))
}

/// `systemctl is-active --quiet <unit>` check, shared by `apply` and `install`
/// (was duplicated identically in both).
fn is_service_active(unit: &str) -> bool {
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

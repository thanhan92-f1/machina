// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Periodic Vault provider health sync (Phase 32).

use crate::state::AppState;

/// Vault provider health/secret sync cadence (30 minutes) — infrequent
/// because it's a background staleness check, not a live dependency.
const SYNC_INTERVAL_SECS: u64 = 1800;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(SYNC_INTERVAL_SECS));
        loop {
            interval.tick().await;
            if let Err(e) =
                crate::engine::enterprise_security::sync_all_vault_providers(&state.pool).await
            {
                tracing::warn!("vault sync scheduler: {e:#}");
            }
        }
    });
}

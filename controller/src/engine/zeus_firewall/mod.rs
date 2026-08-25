// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Zeus Firewall — fleet orchestration for machine protection.

pub mod approvals;
pub mod checkpoint;
pub mod cloud;
pub mod compliance_pdf;
pub mod drift;
pub mod finops;
pub mod gitops;
pub mod guest_ports;
pub mod inventory;
pub mod k8s;
pub mod lockdown;
pub mod metal;
pub mod multisite;
pub mod operator;
pub mod policy_operator;
pub mod profiles;
pub mod siem;
pub mod sync;
pub mod temporary;
pub mod worker;

pub use approvals::*;
pub use checkpoint::*;
pub use gitops::*;
pub use inventory::*;
pub use lockdown::*;
pub use metal::*;
pub use temporary::*;

/// Records a `firewall_timeline` audit-trail row. Every call site historically
/// wrote this INSERT inline and swallowed the error with `let _ = ...`: the
/// timeline entry is a best-effort audit record alongside a firewall mutation
/// (or scan) that has already happened, so a transient DB error here must
/// never fail or roll back the operation it's describing. Centralized here so
/// that "ignore the error, but still try" policy is expressed once instead of
/// once per call site.
pub(crate) async fn record_timeline(
    pool: &sqlx::SqlitePool,
    target_kind: &str,
    target_id: uuid::Uuid,
    kind: &str,
    summary: &str,
    detail: &serde_json::Value,
    actor: &str,
) {
    let _ = sqlx::query(
        "INSERT INTO firewall_timeline (id, target_kind, target_id, kind, summary, detail_json, actor) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(target_kind)
    .bind(target_id)
    .bind(kind)
    .bind(summary)
    .bind(detail)
    .bind(actor)
    .execute(pool)
    .await;
}

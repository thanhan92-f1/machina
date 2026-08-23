// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Cinder v3-compatible block storage endpoints.
//!
//! A `volumes` row is a standalone, project-owned resource created UNATTACHED — the
//! key generalization Cinder needs over Machina's existing `vm_disks`, which is always
//! VM-scoped from creation.
//!
//! Two storage backends, selected per volume at create time:
//! - **Atlas** (`ATLAS_ENABLED=1`, the existing Zyvor storage control plane — Ceph/RBD
//!   volumes with real snapshot/backup/restore, `engine::atlas_bridge`/`atlas_vm`) —
//!   used when enabled, giving Cinder real snapshots instead of a bare record. Attach
//!   resolves the RBD backend-native id and builds a `rbd:` libvirt network-disk source
//!   via `atlas_vm::rbd_source`, then delegates to the existing `vms::attach_vm_disk`
//!   exactly like the native `atlas_root_disk` VM-create path already does.
//! - **Local pool** fallback — the existing pool-volume agent RPC in
//!   `api::storage::create_storage_pool_volume`, for deployments without Atlas.
//!
//! Either way, attach/detach/extend delegate to the existing
//! `vms::{attach_vm_disk, detach_vm_disk, resize_vm_disk}` handlers — no new libvirt
//! plumbing is added here. `attach_vm_disk` already supports attaching an existing
//! path without allocating a new disk when `size_gib` is omitted, which is exactly the
//! "attach a pre-existing volume" mode Cinder needs.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::storage::{self, CreateStorageVolumeBody, StoragePoolHostQuery};
use crate::api::vms;
use crate::api::ApiError;
use crate::auth::{require_admin, require_operator, AuthUser};
use crate::engine::atlas_bridge::{self, AtlasOwner};
use crate::engine::atlas_vm;
use crate::state::AppState;

/// `volumes.project_id` is a real FK to `projects.id` (unlike `vms.project`, which
/// joins by free-text name — see Phase 0's backward-compat note), so the path segment
/// only needs parsing and an existence check, no name round-trip.
async fn parse_project_id(pool: &sqlx::SqlitePool, project_id: &str) -> Result<Uuid, ApiError> {
    let id = Uuid::parse_str(project_id).map_err(|_| ApiError::bad_request("invalid project_id"))?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?)")
        .bind(id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found("project not found"));
    }
    Ok(id)
}

#[derive(Debug, sqlx::FromRow)]
struct VolumeRow {
    id: Uuid,
    name: String,
    size_gib: i64,
    volume_type: String,
    status: String,
    attached_vm_id: Option<Uuid>,
    attached_device: Option<String>,
    bootable: bool,
    atlas_volume_id: Option<String>,
}

impl VolumeRow {
    fn to_cinder(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "name": self.name,
            "size": self.size_gib,
            "volume_type": self.volume_type,
            "status": self.status,
            "bootable": if self.bootable { "true" } else { "false" },
            "attachments": self.attached_vm_id.map(|vm| serde_json::json!([{
                "server_id": vm.to_string(),
                "device": self.attached_device,
            }])).unwrap_or(serde_json::json!([])),
        })
    }
}

const VOLUME_SELECT: &str = "SELECT id, name, size_gib, volume_type, status, attached_vm_id, \
    attached_device, bootable, atlas_volume_id FROM volumes";

pub async fn list_volumes(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project_uuid = parse_project_id(&state.pool, &project_id).await?;
    let rows = sqlx::query_as::<_, VolumeRow>(&format!(
        "{VOLUME_SELECT} WHERE project_id = ? ORDER BY created_at"
    ))
    .bind(project_uuid)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "volumes": rows.iter().map(VolumeRow::to_cinder).collect::<Vec<_>>() })))
}

pub async fn get_volume(
    State(state): State<AppState>,
    Path((_project_id, id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, VolumeRow>(&format!("{VOLUME_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "volume": row.to_cinder() })))
}

#[derive(Debug, Deserialize)]
pub struct VolumeCreateRequest {
    pub volume: VolumeCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct VolumeCreateBody {
    #[serde(default)]
    pub name: Option<String>,
    pub size: i64,
    #[serde(default)]
    pub volume_type: Option<String>,
}

/// `POST /v3/{project_id}/volumes` — creates the `volumes` row unattached, then
/// provisions the backing storage. Uses Atlas when enabled (`volume_type`, if given,
/// is passed straight through as the Atlas policy name — e.g. `database`/`general`);
/// otherwise falls back to the local storage-pool RPC, matched by `volume_type` against
/// `volume_types.storage_class`, falling back to any available pool.
pub async fn create_volume(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(req): Json<VolumeCreateRequest>,
) -> Result<Response, ApiError> {
    // The local-pool backend used to get this for free via the delegated
    // `storage::create_storage_pool_volume` call — the Atlas backend never delegates
    // to a handler that checks it, so it must be enforced explicitly here for both.
    require_operator(&actor)?;
    let project_uuid = parse_project_id(&state.pool, &project_id).await?;
    let volume_type = req.volume.volume_type.clone().unwrap_or_else(|| "default".into());
    let id = Uuid::new_v4();
    let name = req.volume.name.clone().unwrap_or_else(|| format!("cinder-{id}"));

    sqlx::query(
        "INSERT INTO volumes (id, project_id, name, size_gib, volume_type, status)
         VALUES (?, ?, ?, ?, ?, 'creating')",
    )
    .bind(id)
    .bind(project_uuid)
    .bind(&name)
    .bind(req.volume.size)
    .bind(&volume_type)
    .execute(&state.pool)
    .await?;

    let result = if state.config.atlas_enabled {
        create_volume_atlas(&state, id, &name, req.volume.size, &volume_type).await
    } else {
        create_volume_local(&state, actor, id, req.volume.size, &volume_type).await
    };

    if let Err(e) = result {
        sqlx::query("UPDATE volumes SET status = 'error' WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await?;
        return Err(e);
    }

    let row = sqlx::query_as::<_, VolumeRow>(&format!("{VOLUME_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(serde_json::json!({ "volume": row.to_cinder() }))).into_response())
}

async fn create_volume_atlas(
    state: &AppState,
    id: Uuid,
    name: &str,
    size_gib: i64,
    volume_type: &str,
) -> Result<(), ApiError> {
    let cfg = &state.config;
    let client = atlas_bridge::require_client(cfg).map_err(|e| ApiError::internal(e.to_string()))?;
    let policy = if volume_type == "default" { cfg.atlas_default_policy.as_str() } else { volume_type };
    let owner = AtlasOwner {
        product: "machina".into(),
        resource_type: "cinder_volume".into(),
        resource_id: id.to_string(),
        role: "cinder".into(),
    };
    let job = client
        .create_volume(
            &cfg.atlas_tenant_id,
            &atlas_vm_safe_name(name),
            size_gib.max(1) * 1024 * 1024 * 1024,
            policy,
            Some(&owner),
            None,
            None,
        )
        .await
        .map_err(|e| ApiError::internal(format!("Atlas volume create failed: {e}")))?;
    let atlas_volume_id = job
        .resource_volume_id()
        .ok_or_else(|| ApiError::internal("Atlas create-volume returned no volume_id"))?;
    if let Some(jid) = job.job_id() {
        let _ = client.wait_for_job(jid, Duration::from_secs(60)).await;
    }
    sqlx::query("UPDATE volumes SET status = 'available', atlas_volume_id = ? WHERE id = ?")
        .bind(&atlas_volume_id)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

/// Atlas uses the volume name as the Kubernetes PVC name (RFC 1123 label) — mirrors
/// `engine::atlas_vm::dns_safe`, duplicated locally since that helper is private to
/// its module.
fn atlas_vm_safe_name(s: &str) -> String {
    let lowered: String = s
        .chars()
        .map(|c| {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }
        })
        .collect();
    let trimmed = lowered.trim_matches('-');
    if trimmed.is_empty() { "vol".to_string() } else { trimmed.chars().take(63).collect() }
}

async fn create_volume_local(
    state: &AppState,
    actor: AuthUser,
    id: Uuid,
    size_gib: i64,
    volume_type: &str,
) -> Result<(), ApiError> {
    let storage_class: Option<String> =
        sqlx::query_scalar("SELECT storage_class FROM volume_types WHERE name = ?")
            .bind(volume_type)
            .fetch_optional(&state.pool)
            .await?;
    let pool_row: Option<(Uuid, String)> = if let Some(class) = &storage_class {
        sqlx::query_as("SELECT id, name FROM storage_pools WHERE storage_class = ? LIMIT 1")
            .bind(class)
            .fetch_optional(&state.pool)
            .await?
    } else {
        None
    };
    let (pool_id, _pool_name) = match pool_row {
        Some(p) => p,
        None => sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM storage_pools LIMIT 1")
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::bad_request("no storage pool configured"))?,
    };
    sqlx::query("UPDATE volumes SET storage_pool_id = ? WHERE id = ?")
        .bind(pool_id)
        .bind(id)
        .execute(&state.pool)
        .await?;

    let vol_file_name = format!("cinder-{id}");
    let agent_result = storage::create_storage_pool_volume(
        State(state.clone()),
        Extension(actor),
        Path(pool_id),
        Query(StoragePoolHostQuery { host_id: None }),
        Json(CreateStorageVolumeBody { name: vol_file_name, capacity_gb: size_gib.max(1) as u64, format: "qcow2".into() }),
    )
    .await?;
    let path = agent_result.0.get("path").and_then(|v| v.as_str()).map(str::to_string);
    sqlx::query("UPDATE volumes SET status = 'available', path = ? WHERE id = ?")
        .bind(path)
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn delete_volume(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path((_project_id, id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
    require_operator(&actor)?;
    let row: Option<(Option<Uuid>, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT attached_vm_id, atlas_volume_id, storage_pool_id FROM volumes WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((attached, atlas_volume_id, pool_id)) = row else {
        return Err(ApiError::not_found("volume not found"));
    };
    if attached.is_some() {
        return Err(ApiError::bad_request("volume is still attached — detach it first"));
    }

    if let Some(atlas_id) = atlas_volume_id {
        let client = atlas_bridge::require_client(&state.config).map_err(|e| ApiError::internal(e.to_string()))?;
        client
            .delete_volume(&atlas_id)
            .await
            .map_err(|e| ApiError::internal(format!("Atlas volume delete failed: {e}")))?;
    } else if let Some(pool_id) = pool_id {
        let vol_file_name = format!("cinder-{id}");
        let _ = storage::delete_storage_pool_volume(
            State(state.clone()),
            Extension(actor),
            Path((pool_id, vol_file_name)),
            Query(StoragePoolHostQuery { host_id: None }),
        )
        .await;
    }

    sqlx::query("DELETE FROM volumes WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Polls Atlas for the RBD backend-native id of a just-created volume and builds its
/// libvirt network-disk source — the same wait-and-resolve step
/// `engine::atlas_vm::provision_vm_volume` performs inline for VM-create-time volumes,
/// needed here too since Cinder volumes are created before any attach is requested.
async fn resolve_atlas_rbd_source(
    client: &atlas_bridge::AtlasClient,
    cfg: &crate::config::ControllerConfig,
    atlas_volume_id: &str,
) -> Result<String, ApiError> {
    for attempt in 0..8 {
        if let Ok(v) = client.get_volume(atlas_volume_id).await {
            if let Some(native) = v.backend_native_id {
                return Ok(atlas_vm::rbd_source(cfg, &native));
            }
        }
        if attempt < 7 {
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    Err(ApiError::internal("Atlas volume has no backend-native id yet — try again shortly"))
}

/// `POST /v3/{project_id}/volumes/{id}/action` — `os-attach`/`os-detach`/`os-extend`.
pub async fn volume_action(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path((_project_id, id)): Path<(String, Uuid)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    require_operator(&actor)?;
    let obj = body.as_object().ok_or_else(|| ApiError::bad_request("expected an action object"))?;
    let (action, params) = obj.iter().next().ok_or_else(|| ApiError::bad_request("empty action body"))?;

    match action.as_str() {
        "os-attach" => {
            let instance_uuid = params
                .get("instance_uuid")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| ApiError::bad_request("os-attach requires instance_uuid"))?;
            let row: Option<(Option<String>, Option<String>)> =
                sqlx::query_as("SELECT path, atlas_volume_id FROM volumes WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&state.pool)
                    .await?;
            let Some((path, atlas_volume_id)) = row else {
                return Err(ApiError::not_found("volume not found"));
            };
            let disk_source = if let Some(atlas_id) = &atlas_volume_id {
                let client = atlas_bridge::require_client(&state.config).map_err(|e| ApiError::internal(e.to_string()))?;
                resolve_atlas_rbd_source(&client, &state.config, atlas_id).await?
            } else {
                path.ok_or_else(|| ApiError::internal("volume has no backing path"))?
            };
            let target_dev = params.get("mountpoint").and_then(|v| v.as_str()).unwrap_or("vdb").to_string();

            let _ = vms::attach_vm_disk(
                State(state.clone()),
                Extension(actor),
                Path(instance_uuid),
                Json(vms::AttachDiskBody { disk_path: disk_source, target_dev: target_dev.clone(), size_gib: None }),
            )
            .await?;

            if let Some(atlas_id) = &atlas_volume_id {
                // Register with Machina's native per-VM Atlas bookkeeping too, so the
                // existing "snapshot/back up this VM" tooling (engine::atlas_vm::
                // snapshot_vm/backup_vm, which enumerates vm_atlas_volumes by vm_id)
                // also covers a volume that arrived via the Cinder-compat API.
                let _ = sqlx::query(
                    "INSERT INTO vm_atlas_volumes (id, vm_id, volume_id, role, size_bytes, policy, state)
                     SELECT ?, ?, ?, ?, size_gib * 1024 * 1024 * 1024, volume_type, 'attached'
                     FROM volumes WHERE id = ?",
                )
                .bind(Uuid::new_v4())
                .bind(instance_uuid)
                .bind(atlas_id)
                .bind(format!("cinder-{target_dev}"))
                .bind(id)
                .execute(&state.pool)
                .await;
            }

            sqlx::query(
                "UPDATE volumes SET status = 'in-use', attached_vm_id = ?, attached_device = ? WHERE id = ?",
            )
            .bind(instance_uuid)
            .bind(&target_dev)
            .bind(id)
            .execute(&state.pool)
            .await?;
        }
        "os-detach" => {
            let row: Option<(Option<Uuid>, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT attached_vm_id, attached_device, atlas_volume_id FROM volumes WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
            if let Some((Some(vm_id), Some(target_dev), atlas_volume_id)) = row {
                let _ = vms::detach_vm_disk(State(state.clone()), Extension(actor), Path((vm_id, target_dev))).await?;
                if let Some(atlas_id) = atlas_volume_id {
                    let _ = sqlx::query(
                        "DELETE FROM vm_atlas_volumes WHERE vm_id = ? AND volume_id = ?",
                    )
                    .bind(vm_id)
                    .bind(atlas_id)
                    .execute(&state.pool)
                    .await;
                }
            }
            sqlx::query(
                "UPDATE volumes SET status = 'available', attached_vm_id = NULL, attached_device = NULL WHERE id = ?",
            )
            .bind(id)
            .execute(&state.pool)
            .await?;
        }
        "os-extend" => {
            let new_size = params
                .get("new_size")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| ApiError::bad_request("os-extend requires new_size"))?;
            let row: Option<(Option<Uuid>, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT attached_vm_id, attached_device, atlas_volume_id FROM volumes WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
            let Some((attached_vm_id, attached_device, atlas_volume_id)) = row else {
                return Err(ApiError::not_found("volume not found"));
            };
            if let Some(atlas_id) = &atlas_volume_id {
                let client = atlas_bridge::require_client(&state.config).map_err(|e| ApiError::internal(e.to_string()))?;
                client
                    .expand_volume(atlas_id, new_size.max(1) * 1024 * 1024 * 1024)
                    .await
                    .map_err(|e| ApiError::internal(format!("Atlas volume expand failed: {e}")))?;
            }
            // Best-effort guest-visible resize when attached; the backend (pool file or
            // Atlas/RBD) is already extended above/via the local-pool path below.
            if let (Some(vm_id), Some(target_dev)) = (attached_vm_id, attached_device) {
                let _ = vms::resize_vm_disk(
                    State(state.clone()),
                    Extension(actor),
                    Path((vm_id, target_dev)),
                    Json(vms::ResizeVmDiskBody { size_gb: new_size.max(1) as u64 }),
                )
                .await;
            }
            sqlx::query("UPDATE volumes SET size_gib = ? WHERE id = ?")
                .bind(new_size)
                .bind(id)
                .execute(&state.pool)
                .await?;
        }
        other => return Err(ApiError::bad_request(format!("unsupported volume action: {other}"))),
    }
    Ok(StatusCode::ACCEPTED.into_response())
}

// ---------------------------------------------------------------------------
// Volume types
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct VolumeTypeRow {
    id: Uuid,
    name: String,
    storage_class: String,
}

pub async fn list_volume_types(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, VolumeTypeRow>("SELECT id, name, storage_class FROM volume_types ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    let types: Vec<_> = rows
        .iter()
        .map(|r| serde_json::json!({ "id": r.id.to_string(), "name": r.name, "extra_specs": { "storage_class": r.storage_class } }))
        .collect();
    Ok(Json(serde_json::json!({ "volume_types": types })))
}

#[derive(Debug, Deserialize)]
pub struct VolumeTypeCreateRequest {
    pub volume_type: VolumeTypeCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct VolumeTypeCreateBody {
    pub name: String,
    #[serde(default = "default_storage_class")]
    pub storage_class: String,
}

fn default_storage_class() -> String {
    "silver".into()
}

pub async fn create_volume_type(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<VolumeTypeCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&actor)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO volume_types (id, name, storage_class) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&req.volume_type.name)
        .bind(&req.volume_type.storage_class)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({
        "volume_type": { "id": id.to_string(), "name": req.volume_type.name }
    })))
}

// ---------------------------------------------------------------------------
// Snapshots — real Atlas snapshots when the source volume is Atlas-backed,
// record-only otherwise (no libvirt-level snapshot mechanism for local-pool
// volumes exists to wire up here).
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct SnapshotRow {
    id: Uuid,
    volume_id: Uuid,
    name: String,
    status: String,
}

pub async fn list_snapshots(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, SnapshotRow>(
        "SELECT id, volume_id, name, status FROM volume_snapshots ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;
    let snaps: Vec<_> = rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id.to_string(), "volume_id": r.volume_id.to_string(), "name": r.name, "status": r.status,
        }))
        .collect();
    Ok(Json(serde_json::json!({ "snapshots": snaps })))
}

#[derive(Debug, Deserialize)]
pub struct SnapshotCreateRequest {
    pub snapshot: SnapshotCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotCreateBody {
    pub volume_id: Uuid,
    #[serde(default)]
    pub name: String,
}

pub async fn create_snapshot(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<SnapshotCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_operator(&actor)?;
    let atlas_volume_id: Option<String> =
        sqlx::query_scalar("SELECT atlas_volume_id FROM volumes WHERE id = ?")
            .bind(req.snapshot.volume_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();

    let id = Uuid::new_v4();
    let (status, atlas_snapshot_id) = if let Some(vol_atlas_id) = &atlas_volume_id {
        let client = atlas_bridge::require_client(&state.config).map_err(|e| ApiError::internal(e.to_string()))?;
        let job = client
            .snapshot_volume(vol_atlas_id, Some(&req.snapshot.name))
            .await
            .map_err(|e| ApiError::internal(format!("Atlas snapshot failed: {e}")))?;
        let snap_id = job.resource_snapshot_id();
        ("available", snap_id)
    } else {
        ("available", None)
    };

    sqlx::query(
        "INSERT INTO volume_snapshots (id, volume_id, name, status, atlas_snapshot_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(req.snapshot.volume_id)
    .bind(&req.snapshot.name)
    .bind(status)
    .bind(&atlas_snapshot_id)
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({
        "snapshot": {
            "id": id.to_string(),
            "volume_id": req.snapshot.volume_id.to_string(),
            "name": req.snapshot.name,
            "status": status,
        }
    })))
}

pub async fn delete_snapshot(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path((_project_id, id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
    require_operator(&actor)?;
    let atlas_snapshot_id: Option<String> =
        sqlx::query_scalar("SELECT atlas_snapshot_id FROM volume_snapshots WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
    if let Some(snap_id) = atlas_snapshot_id {
        let client = atlas_bridge::require_client(&state.config).map_err(|e| ApiError::internal(e.to_string()))?;
        let _ = client.delete_snapshot(&snap_id, false).await;
    }
    sqlx::query("DELETE FROM volume_snapshots WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

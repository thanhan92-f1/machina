// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Nova v2.1-compatible compute endpoints. Server create/list/get/delete and power
//! actions all translate JSON and then delegate to the SAME handlers the native
//! Machina VM API uses (`api::vms::{create_vm, delete_vm, start_vm, ...}`) — no VM
//! lifecycle logic is duplicated here.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use machina_spec::VirtualMachine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::vms::{self, CreateVmBody, VmRow};
use crate::api::ApiError;
use crate::auth::{require_admin, AuthUser};
use crate::state::AppState;

/// Resolves a Nova `{project_id}` path segment (a `projects.id`) to the free-text
/// project name that `vms.project` / `create_vm` actually key on (see Phase 0's
/// backward-compat note: `projects` and the legacy free-text column are joined by
/// name, not FK).
async fn resolve_project_name(pool: &sqlx::SqlitePool, project_id: &str) -> Result<String, ApiError> {
    let id = Uuid::parse_str(project_id)
        .map_err(|_| ApiError::bad_request("invalid project_id"))?;
    sqlx::query_scalar("SELECT name FROM projects WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| ApiError::not_found("project not found"))
}

/// Nova's server `status` vocabulary, derived from Machina's own VM state columns —
/// a translation, not a new state machine.
fn nova_status(row: &VmRow) -> &'static str {
    match row.lifecycle_phase.as_str() {
        "creating" => "BUILD",
        "error" => "ERROR",
        "deleting" => "DELETED",
        _ => match row.observed_state.as_str() {
            "running" => "ACTIVE",
            "shutoff" | "stopped" => "SHUTOFF",
            "paused" => "PAUSED",
            "missing" => "ERROR",
            _ => "BUILD",
        },
    }
}

fn server_json(row: &VmRow, flavor_id: Option<&str>, image_id: Option<&str>) -> serde_json::Value {
    let mut addresses = serde_json::Map::new();
    if let Some(ip) = &row.guest_ip {
        addresses.insert(
            "private".into(),
            serde_json::json!([{ "addr": ip, "version": 4, "OS-EXT-IPS:type": "fixed" }]),
        );
    }
    serde_json::json!({
        "id": row.id.to_string(),
        "name": row.name,
        "status": nova_status(row),
        "tenant_id": row.project.clone().unwrap_or_default(),
        "user_id": "",
        "flavor": { "id": flavor_id.unwrap_or("") },
        "image": image_id.map(|i| serde_json::json!({ "id": i })).unwrap_or(serde_json::json!("")),
        "addresses": addresses,
        "metadata": {},
        "OS-EXT-STS:vm_state": row.lifecycle_phase,
        "OS-EXT-SRV-ATTR:host": row.host_id.map(|h| h.to_string()),
        "links": [],
    })
}

// ---------------------------------------------------------------------------
// Flavors
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct FlavorRow {
    pub id: Uuid,
    pub name: String,
    pub vcpus: i32,
    pub ram_mib: i64,
    pub disk_gib: i64,
    pub is_public: bool,
}

impl FlavorRow {
    fn to_nova(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "name": self.name,
            "vcpus": self.vcpus,
            "ram": self.ram_mib,
            "disk": self.disk_gib,
            "swap": "",
            "os-flavor-access:is_public": self.is_public,
            "links": [],
        })
    }
}

pub async fn list_flavors(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, FlavorRow>(
        "SELECT id, name, vcpus, ram_mib, disk_gib, is_public FROM flavors ORDER BY ram_mib",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "flavors": rows.iter().map(FlavorRow::to_nova).collect::<Vec<_>>() })))
}

#[derive(Debug, Deserialize)]
pub struct CreateFlavorRequest {
    pub flavor: CreateFlavorBody,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlavorBody {
    pub name: String,
    pub vcpus: i32,
    pub ram: i64,
    #[serde(default)]
    pub disk: i64,
    #[serde(default = "default_true")]
    pub is_public: bool,
}

fn default_true() -> bool {
    true
}

pub async fn create_flavor(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<CreateFlavorRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&actor)?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO flavors (id, name, vcpus, ram_mib, disk_gib, is_public) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(&req.flavor.name)
    .bind(req.flavor.vcpus)
    .bind(req.flavor.ram)
    .bind(req.flavor.disk)
    .bind(req.flavor.is_public)
    .execute(&state.pool)
    .await?;
    let row = FlavorRow {
        id,
        name: req.flavor.name,
        vcpus: req.flavor.vcpus,
        ram_mib: req.flavor.ram,
        disk_gib: req.flavor.disk,
        is_public: req.flavor.is_public,
    };
    Ok(Json(serde_json::json!({ "flavor": row.to_nova() })))
}

// ---------------------------------------------------------------------------
// Servers
// ---------------------------------------------------------------------------

pub async fn list_servers(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = resolve_project_name(&state.pool, &project_id).await?;
    let rows = sqlx::query_as::<_, VmRow>(
        "SELECT v.id, v.name, v.host_id, v.desired_state, v.observed_state,
                COALESCE(v.lifecycle_phase, 'idle') AS lifecycle_phase,
                COALESCE(v.last_error, '') AS last_error,
                COALESCE(v.managed, TRUE) AS managed,
                v.uuid, v.vcpus, v.memory_mib,
                FALSE AS ha_enabled, v.project, COALESCE(v.tags, '[]') AS tags,
                COALESCE(v.inventory_source, 'libvirt') AS inventory_source,
                v.k8s_namespace, v.last_seen_at, v.guest_ip, v.guest_tools_status
         FROM vms v WHERE v.project = ? ORDER BY v.name",
    )
    .bind(&project)
    .fetch_all(&state.pool)
    .await?;
    let servers: Vec<_> = rows.iter().map(|r| server_json(r, None, None)).collect();
    Ok(Json(serde_json::json!({ "servers": servers })))
}

pub async fn get_server(
    State(state): State<AppState>,
    Path((_project_id, id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, VmRow>(
        "SELECT v.id, v.name, v.host_id, v.desired_state, v.observed_state,
                COALESCE(v.lifecycle_phase, 'idle') AS lifecycle_phase,
                COALESCE(v.last_error, '') AS last_error,
                COALESCE(v.managed, TRUE) AS managed,
                v.uuid, v.vcpus, v.memory_mib,
                FALSE AS ha_enabled, v.project, COALESCE(v.tags, '[]') AS tags,
                COALESCE(v.inventory_source, 'libvirt') AS inventory_source,
                v.k8s_namespace, v.last_seen_at, v.guest_ip, v.guest_tools_status
         FROM vms v WHERE v.id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "server": server_json(&row, None, None) })))
}

#[derive(Debug, Deserialize)]
pub struct ServerCreateRequest {
    pub server: ServerCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct ServerCreateBody {
    pub name: String,
    #[serde(rename = "imageRef", default)]
    pub image_ref: Option<String>,
    #[serde(rename = "flavorRef")]
    pub flavor_ref: String,
    #[serde(default)]
    pub networks: Vec<ServerNetworkRef>,
}

#[derive(Debug, Deserialize)]
pub struct ServerNetworkRef {
    #[serde(default)]
    pub uuid: Option<String>,
}

/// `POST /v2.1/{project_id}/servers` — translates Nova's create body into
/// `machina_spec::VirtualMachine` and calls the existing `vms::create_vm` handler
/// directly, the same way `vms::create_from_iso` already does internally. The VM's id
/// isn't in `create_vm`'s `TaskResponse`, so it's recovered from the enqueued task's
/// `resource_id` (set by `enqueue_task` to the new vm_id) rather than duplicating any
/// of `create_vm`'s insert logic here.
pub async fn create_server(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(req): Json<ServerCreateRequest>,
) -> Result<Response, ApiError> {
    let project = resolve_project_name(&state.pool, &project_id).await?;

    let flavor_id = Uuid::parse_str(&req.server.flavor_ref)
        .map_err(|_| ApiError::bad_request("invalid flavorRef"))?;
    let flavor = sqlx::query_as::<_, FlavorRow>(
        "SELECT id, name, vcpus, ram_mib, disk_gib, is_public FROM flavors WHERE id = ?",
    )
    .bind(flavor_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ApiError::bad_request("flavor not found"))?;

    let mut image_source: Option<String> = None;
    if let Some(image_ref) = &req.server.image_ref {
        if let Ok(image_id) = Uuid::parse_str(image_ref) {
            image_source = sqlx::query_scalar("SELECT path FROM content_images WHERE id = ?")
                .bind(image_id)
                .fetch_optional(&state.pool)
                .await?;
        }
    }

    let network_name: String = if let Some(net_ref) = req.server.networks.first().and_then(|n| n.uuid.clone()) {
        if let Ok(net_id) = Uuid::parse_str(&net_ref) {
            let found: Option<String> = sqlx::query_scalar("SELECT name FROM networks WHERE id = ?")
                .bind(net_id)
                .fetch_optional(&state.pool)
                .await?;
            found.unwrap_or_else(|| "default".to_string())
        } else {
            "default".to_string()
        }
    } else {
        "default".to_string()
    };

    let mut vm = VirtualMachine::new(&req.server.name, format!("{}Mi", flavor.ram_mib));
    vm.metadata.project = Some(project);
    vm.spec.cpu.sockets = 1;
    vm.spec.cpu.cores = flavor.vcpus.max(1) as u32;
    if let Some(vol) = vm.spec.storage.first_mut() {
        vol.size = format!("{}Gi", flavor.disk_gib.max(1));
        vol.source = image_source;
    }
    if let Some(net) = vm.spec.network.first_mut() {
        net.network = network_name;
    }

    let create_body = CreateVmBody {
        vm,
        host_id: None,
        tags: vec!["openstack-compat".into()],
        desired_state: "running".into(),
        atlas_root_disk: false,
        atlas_policy: None,
    };

    let task = vms::create_vm(State(state.clone()), Extension(actor), Json(create_body)).await?;
    // task_id is the canonical hyphenated string form (`Uuid::to_string()` in
    // create_vm); parse it back to a `Uuid` before binding so sqlx encodes it the
    // same way it was stored (raw bytes, not the hyphenated string — see the
    // `CHECK(length(id) = 16)` convention on these tables).
    let task_uuid = Uuid::parse_str(&task.0.task_id).map_err(|e| ApiError::internal(e.to_string()))?;
    let vm_id: Uuid = sqlx::query_scalar("SELECT resource_id FROM tasks WHERE id = ?")
        .bind(task_uuid)
        .fetch_one(&state.pool)
        .await?;

    let body = serde_json::json!({
        "server": {
            "id": vm_id.to_string(),
            "name": req.server.name,
            "status": "BUILD",
            "flavor": { "id": flavor.id.to_string() },
            "adminPass": "",
            "links": [],
        }
    });
    Ok((StatusCode::ACCEPTED, Json(body)).into_response())
}

pub async fn delete_server(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path((_project_id, id)): Path<(String, Uuid)>,
) -> Result<Response, ApiError> {
    let _ = vms::delete_vm(State(state), Extension(actor), Path(id), None).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// `POST /v2.1/{project_id}/servers/{id}/action` — Nova's single action-dispatch
/// endpoint (`{"reboot": {...}}`, `{"os-stop": {}}`, ...). Delegates every action to
/// the corresponding existing `vms::*` power/resize handler; never touches libvirt
/// directly.
pub async fn server_action(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path((_project_id, id)): Path<(String, Uuid)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Response, ApiError> {
    let obj = body
        .as_object()
        .ok_or_else(|| ApiError::bad_request("expected a single-key action object"))?;
    let (action, params) = obj
        .iter()
        .next()
        .ok_or_else(|| ApiError::bad_request("empty action body"))?;

    match action.as_str() {
        "reboot" => {
            let _ = vms::reboot_vm(State(state), Extension(actor), Path(id), None).await?;
        }
        "os-start" | "start" => {
            let _ = vms::start_vm(State(state), Extension(actor), Path(id)).await?;
        }
        "os-stop" | "stop" => {
            let _ = vms::stop_vm(State(state), Extension(actor), Path(id)).await?;
        }
        "pause" => {
            let _ = vms::pause_vm(State(state), Extension(actor), Path(id)).await?;
        }
        "unpause" | "resume" => {
            let _ = vms::resume_vm(State(state), Extension(actor), Path(id)).await?;
        }
        "os-reset" | "reset" => {
            let _ = vms::reset_vm(State(state), Extension(actor), Path(id)).await?;
        }
        "resize" => {
            let flavor_ref = params
                .get("flavorRef")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("resize requires flavorRef"))?;
            let flavor_id = Uuid::parse_str(flavor_ref)
                .map_err(|_| ApiError::bad_request("invalid flavorRef"))?;
            let flavor = sqlx::query_as::<_, FlavorRow>(
                "SELECT id, name, vcpus, ram_mib, disk_gib, is_public FROM flavors WHERE id = ?",
            )
            .bind(flavor_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::bad_request("flavor not found"))?;
            let _ = vms::set_vm_vcpus(
                State(state.clone()),
                Extension(actor.clone()),
                Path(id),
                Json(vms::SetVcpusBody { count: flavor.vcpus.max(1) as u32 }),
            )
            .await?;
            let _ = vms::set_vm_memory(
                State(state),
                Extension(actor),
                Path(id),
                Json(vms::SetMemoryBody { memory_mb: flavor.ram_mib.max(1) as u64 }),
            )
            .await?;
        }
        other => {
            return Err(ApiError::bad_request(format!("unsupported server action: {other}")));
        }
    }
    Ok(StatusCode::ACCEPTED.into_response())
}

/// `GET /v2.1/{project_id}/limits` — translated from the existing `project_quotas`
/// table (matched by the resolved project name, same as every other legacy
/// project-scoped query).
pub async fn limits(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let project = resolve_project_name(&state.pool, &project_id).await?;
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT max_vms, max_vcpu, max_memory_mib, max_storage_gib FROM project_quotas WHERE project = ?",
    )
    .bind(&project)
    .fetch_optional(&state.pool)
    .await?;
    let (max_vms, max_vcpu, max_memory_mib, _max_storage_gib) = row.unwrap_or((0, 0, 0, 0));
    let used_vcpu: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(vcpus), 0) FROM vms WHERE project = ?")
        .bind(&project)
        .fetch_one(&state.pool)
        .await?;
    let used_ram: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(memory_mib), 0) FROM vms WHERE project = ?")
        .bind(&project)
        .fetch_one(&state.pool)
        .await?;
    let used_instances: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vms WHERE project = ?")
        .bind(&project)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({
        "limits": {
            "absolute": {
                "maxTotalCores": max_vcpu,
                "maxTotalRAMSize": max_memory_mib,
                "maxTotalInstances": max_vms,
                "totalCoresUsed": used_vcpu,
                "totalRAMUsed": used_ram,
                "totalInstancesUsed": used_instances,
            }
        }
    })))
}

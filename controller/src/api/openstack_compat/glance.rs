// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Glance v2-compatible image endpoints, extending the existing `content_images` table
//! (see migration 023) rather than replacing it — `api::content`'s submitted/approved
//! governance workflow is untouched; these handlers write `glance_status` (a separate,
//! new column) and go straight to `'active'`, since an OpenStack-client upload is a
//! different actor/workflow from that manual approval queue.
//!
//! Upload is Glance's real two-step protocol: `POST /v2/images` registers metadata
//! (`glance_status='queued'`, no file yet), then `PUT /v2/images/{id}/file` streams the
//! raw disk image body to `disk_image_dir` on the controller's own filesystem.
//!
//! Open design question (not resolved here, flagged per the implementation plan): in a
//! multi-host deployment the uploaded file lands on the CONTROLLER's filesystem, which
//! may not be the hypervisor host `create_vm` ultimately schedules onto. Single-host /
//! co-located deployments work as-is; multi-host needs either a shared/NFS-backed
//! `disk_image_dir` or an agent-side fetch step before Phase 2's server-create can boot
//! from an uploaded image on a remote host.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::api::ApiError;
use crate::state::AppState;

#[derive(Debug, sqlx::FromRow)]
struct ImageRow {
    id: Uuid,
    name: String,
    path: String,
    size_gib: i64,
    disk_format: String,
    container_format: String,
    visibility: String,
    min_disk_gib: i64,
    min_ram_mib: i64,
    glance_status: String,
    checksum: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl ImageRow {
    fn to_glance(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "name": self.name,
            "status": self.glance_status,
            "disk_format": self.disk_format,
            "container_format": self.container_format,
            "visibility": self.visibility,
            "min_disk": self.min_disk_gib,
            "min_ram": self.min_ram_mib,
            "size": self.size_gib * 1024 * 1024 * 1024,
            "checksum": self.checksum,
            "created_at": self.created_at.to_rfc3339(),
            "tags": [],
        })
    }
}

const IMAGE_SELECT: &str = "SELECT id, name, path, size_gib, disk_format, container_format, \
    visibility, min_disk_gib, min_ram_mib, glance_status, checksum, created_at FROM content_images";

#[derive(Debug, Deserialize)]
pub struct CreateImageRequest {
    pub name: String,
    #[serde(default = "default_disk_format")]
    pub disk_format: String,
    #[serde(default = "default_container_format")]
    pub container_format: String,
    #[serde(default = "default_visibility")]
    pub visibility: String,
    #[serde(default)]
    pub min_disk: i64,
    #[serde(default)]
    pub min_ram: i64,
}

fn default_disk_format() -> String {
    "qcow2".into()
}
fn default_container_format() -> String {
    "bare".into()
}
fn default_visibility() -> String {
    "private".into()
}

/// `POST /v2/images` — registers metadata only; `glance_status` starts `'queued'` until
/// the file is uploaded via `upload_image_file`.
pub async fn create_image(
    State(state): State<AppState>,
    Json(req): Json<CreateImageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    machina_spec::validate_name(&req.name).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let cluster_id: Uuid = sqlx::query_scalar("SELECT id FROM clusters LIMIT 1")
        .fetch_one(&state.pool)
        .await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO content_images
            (id, cluster_id, name, kind, path, size_gib, status, disk_format,
             container_format, visibility, min_disk_gib, min_ram_mib, glance_status)
         VALUES (?, ?, ?, 'glance', '', 0, 'available', ?, ?, ?, ?, ?, 'queued')",
    )
    .bind(id)
    .bind(cluster_id)
    .bind(&req.name)
    .bind(&req.disk_format)
    .bind(&req.container_format)
    .bind(&req.visibility)
    .bind(req.min_disk)
    .bind(req.min_ram)
    .execute(&state.pool)
    .await?;
    let row = sqlx::query_as::<_, ImageRow>(&format!("{IMAGE_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.to_glance()))
}

/// `PUT /v2/images/{id}/file` — raw `application/octet-stream` body (Glance's upload is
/// a plain PUT, not multipart), streamed to disk in chunks rather than buffered in
/// memory (disk images are routinely multi-gigabyte).
pub async fn upload_image_file(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Body,
) -> Result<Response, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM content_images WHERE id = ?)")
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::not_found("image not found"));
    }

    let dir = state.config.disk_image_dir.join("glance");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ApiError::internal(format!("failed to create image directory: {e}")))?;
    let path = dir.join(format!("{id}.img"));

    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| ApiError::internal(format!("failed to create image file: {e}")))?;
    let mut stream = body.into_data_stream();
    let mut total_bytes: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ApiError::bad_request(format!("upload stream error: {e}")))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| ApiError::internal(format!("failed to write image file: {e}")))?;
        total_bytes += chunk.len() as u64;
    }
    file.flush().await.map_err(|e| ApiError::internal(e.to_string()))?;

    let size_gib = ((total_bytes as f64) / (1024.0 * 1024.0 * 1024.0)).ceil() as i64;
    sqlx::query(
        "UPDATE content_images SET path = ?, size_gib = ?, glance_status = 'active' WHERE id = ?",
    )
    .bind(path.to_string_lossy().to_string())
    .bind(size_gib)
    .bind(id)
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn list_images(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, ImageRow>(&format!("{IMAGE_SELECT} WHERE kind = 'glance' ORDER BY name"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "images": rows.iter().map(ImageRow::to_glance).collect::<Vec<_>>() })))
}

pub async fn get_image(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, ImageRow>(&format!("{IMAGE_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.to_glance()))
}

pub async fn delete_image(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let path: Option<String> = sqlx::query_scalar("SELECT path FROM content_images WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(path) = path else {
        return Err(ApiError::not_found("image not found"));
    };
    sqlx::query("DELETE FROM content_images WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if !path.is_empty() {
        let _ = tokio::fs::remove_file(&path).await;
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

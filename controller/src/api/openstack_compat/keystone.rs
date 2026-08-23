// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Keystone v3-compatible identity endpoints: password auth, project-scoped opaque
//! tokens, and a self-referential service catalog pointing back at Machina's own
//! openstack_compat routes (no real OpenStack endpoint is ever involved).
//!
//! Tokens are opaque, server-side-validated strings (not JWT-in-the-clear) — real
//! `python-openstackclient`/Terraform's `openstack` provider treat the `X-Subject-Token`
//! value as opaque and never decode it, so this matches actual Keystone behavior and
//! avoids retrofitting Fernet crypto. Validation is a single indexed hash lookup against
//! `keystone_tokens`, wired into `auth::auth_middleware` alongside the existing
//! Basic-auth/API-key/Bearer-JWT branches (see `authenticate_keystone_token`).

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::{require_admin, AuthUser};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request wire types (Keystone v3 password-auth request body)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub auth: AuthBlock,
}

#[derive(Debug, Deserialize)]
pub struct AuthBlock {
    pub identity: IdentityBlock,
    #[serde(default)]
    pub scope: Option<ScopeBlock>,
}

#[derive(Debug, Deserialize)]
pub struct IdentityBlock {
    pub password: PasswordBlock,
}

#[derive(Debug, Deserialize)]
pub struct PasswordBlock {
    pub user: PasswordUser,
}

#[derive(Debug, Deserialize)]
pub struct PasswordUser {
    #[serde(default)]
    pub name: Option<String>,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ScopeBlock {
    #[serde(default)]
    pub project: Option<ProjectScope>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectScope {
    #[serde(default)]
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// Response wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DomainRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct UserRef {
    pub id: String,
    pub name: String,
    pub domain: DomainRef,
}

#[derive(Debug, Serialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub domain: DomainRef,
}

#[derive(Debug, Serialize)]
pub struct RoleRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct EndpointRef {
    pub id: String,
    pub interface: &'static str,
    pub region: &'static str,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct CatalogEntry {
    pub r#type: &'static str,
    pub id: &'static str,
    pub name: &'static str,
    pub endpoints: Vec<EndpointRef>,
}

#[derive(Debug, Serialize)]
pub struct TokenBody {
    pub methods: Vec<&'static str>,
    pub user: UserRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectRef>,
    pub roles: Vec<RoleRef>,
    pub catalog: Vec<CatalogEntry>,
    pub expires_at: String,
    pub issued_at: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponseEnvelope {
    pub token: TokenBody,
}

/// Self-referential service catalog: every entry points back at Machina's own
/// openstack_compat routes (see the phase-by-phase path prefixes below), never at a
/// real external OpenStack. `project_id` is the empty string for an unscoped token —
/// callers must rescope (pass `scope.project`) before Nova/Neutron/Cinder calls, which
/// mirrors real Keystone's behavior for tenant-scoped service endpoint templates.
fn build_catalog(base_url: &str, project_id: &str) -> Vec<CatalogEntry> {
    let base = base_url.trim_end_matches('/');
    vec![
        CatalogEntry {
            r#type: "identity",
            id: "identity",
            name: "keystone",
            endpoints: vec![EndpointRef {
                id: "identity-public".into(),
                interface: "public",
                region: "RegionOne",
                url: format!("{base}/v3"),
            }],
        },
        CatalogEntry {
            r#type: "compute",
            id: "compute",
            name: "nova",
            endpoints: vec![EndpointRef {
                id: "compute-public".into(),
                interface: "public",
                region: "RegionOne",
                url: format!("{base}/v2.1/{project_id}"),
            }],
        },
        CatalogEntry {
            r#type: "image",
            id: "image",
            name: "glance",
            endpoints: vec![EndpointRef {
                id: "image-public".into(),
                interface: "public",
                region: "RegionOne",
                url: format!("{base}/v2"),
            }],
        },
        CatalogEntry {
            r#type: "network",
            id: "network",
            name: "neutron",
            endpoints: vec![EndpointRef {
                id: "network-public".into(),
                interface: "public",
                region: "RegionOne",
                url: format!("{base}/v2.0"),
            }],
        },
        CatalogEntry {
            r#type: "volumev3",
            id: "volumev3",
            name: "cinder",
            endpoints: vec![EndpointRef {
                id: "volume-public".into(),
                interface: "public",
                region: "RegionOne",
                url: format!("{base}/v3/{project_id}"),
            }],
        },
    ]
}

fn generate_opaque_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Validates an `X-Auth-Token` value against `keystone_tokens`, wired into
/// `auth::auth_middleware` the same way `apikeys::authenticate_api_key` validates
/// API keys — one indexed hash lookup, no crypto to verify.
pub async fn authenticate_keystone_token(
    pool: &sqlx::SqlitePool,
    token: &str,
) -> anyhow::Result<Option<AuthUser>> {
    let hash = hash_token(token);
    let row: Option<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "SELECT kt.user_id, kt.project_id, u.role
         FROM keystone_tokens kt
         JOIN users u ON u.id = kt.user_id
         WHERE kt.token_hash = ? AND kt.expires_at > datetime('now')",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let Some((user_id, project_id, role)) = row else {
        return Ok(None);
    };
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(Some(AuthUser {
        username,
        role,
        auth_source: Some("keystone".into()),
        project_id: project_id.map(|id| id.to_string()),
    }))
}

/// `POST /v3/auth/tokens` — public, rate-limited alongside `auth::login`. Password auth
/// via the existing `authenticate()`; unscoped or project-scoped depending on whether
/// `auth.scope.project` is present.
pub async fn token_issue(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> Result<Response, ApiError> {
    let username = req
        .auth
        .identity
        .password
        .user
        .name
        .clone()
        .ok_or_else(|| ApiError::bad_request("auth.identity.password.user.name is required"))?;
    let password = req.auth.identity.password.user.password.clone();

    let user = crate::auth::authenticate(&state.pool, &username, &password)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .ok_or_else(|| ApiError::unauthorized("invalid username or password"))?;

    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_one(&state.pool)
        .await?;

    // Resolve project scope, if requested. A missing project falls back to the "default"
    // project rather than a hard 404 — real OpenStack clients (and most Terraform configs)
    // routinely omit domain qualifiers and expect a sensible single-domain default.
    let mut project_row: Option<(Uuid, String, Uuid, String)> = None;
    if let Some(scope) = &req.auth.scope {
        if let Some(p) = &scope.project {
            let name = p
                .name
                .clone()
                .ok_or_else(|| ApiError::bad_request("scope.project.name is required"))?;
            let row: Option<(Uuid, String, Uuid, String)> = sqlx::query_as(
                "SELECT p.id, p.name, d.id, d.name FROM projects p
                 JOIN domains d ON d.id = p.domain_id WHERE p.name = ?",
            )
            .bind(&name)
            .fetch_optional(&state.pool)
            .await?;
            project_row =
                Some(row.ok_or_else(|| ApiError::not_found(format!("project '{name}' not found")))?);
        }
    }

    // Project-scoped role: an explicit project_role_assignments row wins; otherwise the
    // caller's controller-wide role (admin/operator/viewer) applies in every project, so
    // existing users work immediately without pre-populating per-project assignments.
    let project_role = if let Some((project_id, _, _, _)) = &project_row {
        let explicit: Option<String> = sqlx::query_scalar(
            "SELECT role FROM project_role_assignments WHERE user_id = ? AND project_id = ? LIMIT 1",
        )
        .bind(user_id)
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;
        explicit.unwrap_or_else(|| user.role.clone())
    } else {
        user.role.clone()
    };

    let now = chrono::Utc::now();
    let expires = now + chrono::Duration::hours(1);
    let raw_token = generate_opaque_token();
    let token_hash = hash_token(&raw_token);

    sqlx::query(
        "INSERT INTO keystone_tokens (token_hash, user_id, project_id, role, expires_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&token_hash)
    .bind(user_id)
    .bind(project_row.as_ref().map(|(pid, ..)| *pid))
    .bind(&project_role)
    .bind(expires.to_rfc3339())
    .execute(&state.pool)
    .await?;

    let default_domain = || DomainRef { id: "default".into(), name: "default".into() };
    let project_ref = project_row.as_ref().map(|(pid, pname, did, dname)| ProjectRef {
        id: pid.to_string(),
        name: pname.clone(),
        domain: DomainRef { id: did.to_string(), name: dname.clone() },
    });
    let catalog_project_id =
        project_row.as_ref().map(|(pid, ..)| pid.to_string()).unwrap_or_default();

    let body = TokenResponseEnvelope {
        token: TokenBody {
            methods: vec!["password"],
            user: UserRef { id: user_id.to_string(), name: username, domain: default_domain() },
            project: project_ref,
            roles: vec![RoleRef { id: project_role.clone(), name: project_role }],
            catalog: build_catalog(&state.config.public_base_url, &catalog_project_id),
            expires_at: expires.to_rfc3339(),
            issued_at: now.to_rfc3339(),
        },
    };

    let mut resp = (StatusCode::CREATED, Json(body)).into_response();
    resp.headers_mut().insert(
        "X-Subject-Token",
        HeaderValue::from_str(&raw_token).map_err(|e| ApiError::internal(e.to_string()))?,
    );
    Ok(resp)
}

/// `GET /v3/auth/catalog` — protected. Rebuilds the catalog for the caller's own token
/// scope (empty project segment for an unscoped token).
pub async fn auth_catalog(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
) -> Json<serde_json::Value> {
    let project_id = actor.project_id.clone().unwrap_or_default();
    Json(serde_json::json!({
        "catalog": build_catalog(&state.config.public_base_url, &project_id)
    }))
}

// ---------------------------------------------------------------------------
// Projects / domains / role assignments CRUD
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct ProjectRow {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub project: CreateProjectBody,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectBody {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub async fn list_projects(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, domain_id, name, description, enabled FROM projects ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "projects": rows })))
}

pub async fn create_project(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&actor)?;
    let domain_id: Uuid = sqlx::query_scalar("SELECT id FROM domains WHERE name = 'default'")
        .fetch_one(&state.pool)
        .await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO projects (id, domain_id, name, description, enabled) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(domain_id)
    .bind(&req.project.name)
    .bind(&req.project.description)
    .bind(req.project.enabled)
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({
        "project": ProjectRow {
            id,
            domain_id,
            name: req.project.name,
            description: req.project.description,
            enabled: req.project.enabled,
        }
    })))
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct DomainRow {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateDomainRequest {
    pub domain: CreateDomainBody,
}

#[derive(Debug, Deserialize)]
pub struct CreateDomainBody {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

pub async fn list_domains(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, DomainRow>("SELECT id, name, enabled FROM domains ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "domains": rows })))
}

pub async fn create_domain(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<CreateDomainRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_admin(&actor)?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO domains (id, name, enabled) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&req.domain.name)
        .bind(req.domain.enabled)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({
        "domain": DomainRow { id, name: req.domain.name, enabled: req.domain.enabled }
    })))
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct RoleAssignmentRow {
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub role: String,
}

pub async fn list_role_assignments(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, RoleAssignmentRow>(
        "SELECT user_id, project_id, role FROM project_role_assignments ORDER BY project_id",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "role_assignments": rows })))
}

pub async fn list_user_projects(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, ProjectRow>(
        "SELECT p.id, p.domain_id, p.name, p.description, p.enabled
         FROM projects p
         JOIN project_role_assignments a ON a.project_id = p.id
         WHERE a.user_id = ?
         ORDER BY p.name",
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "projects": rows })))
}

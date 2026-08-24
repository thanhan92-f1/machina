// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Zeus Launchpad — same-origin proxy to Hermes catalog and gateway APIs.

use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use serde::Serialize;

use crate::api::ApiError;
use crate::auth::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchpadConfig {
    pub public_base: String,
    pub path_prefix: String,
    pub enabled: bool,
}

/// Builds an `ApiError` with a caller-chosen status/code/remediation — the
/// shared shape behind every proxy-failure branch below. `ApiError`'s own
/// constructors (`not_found`, `bad_request`, `internal`, ...) don't cover the
/// 503/502 "upstream Hermes unavailable" statuses this proxy needs.
fn proxy_error(
    status: StatusCode,
    message: impl Into<String>,
    error_code: &str,
    remediation: Option<&str>,
) -> ApiError {
    ApiError {
        status,
        message: message.into(),
        error_code: Some(error_code.into()),
        remediation: remediation.map(|s| s.into()),
        object_ref: None,
    }
}

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/launchpad/config", get(launchpad_config))
        .route("/api/v1/launchpad/{*path}", any(launchpad_proxy))
}

async fn launchpad_config(
    State(state): State<AppState>,
) -> Result<axum::Json<LaunchpadConfig>, ApiError> {
    Ok(axum::Json(LaunchpadConfig {
        public_base: state.config.hermes_public_base.clone(),
        path_prefix: state.config.hermes_path_prefix.clone(),
        enabled: !state.config.hermes_api_base.is_empty(),
    }))
}

async fn launchpad_proxy(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    method: Method,
    Path(path): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let base = state.config.hermes_api_base.trim();
    if base.is_empty() {
        return Err(proxy_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Launchpad is not configured (set HERMES_API_BASE)",
            "launchpad_unavailable",
            Some("Install Hermes and set HERMES_API_BASE on machina-controller."),
        ));
    }

    let mut url = format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    if !query.is_empty() {
        let qs: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect();
        url.push('?');
        url.push_str(&qs.join("&"));
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // 8 MiB cap: bounds how much of a client-supplied proxied body this controller
    // buffers in memory before forwarding it to Hermes.
    let body_bytes = axum::body::to_bytes(body, 8 * 1024 * 1024)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut rb = client.request(method.clone(), &url);
    for (k, v) in headers.iter() {
        let name = k.as_str();
        if name.eq_ignore_ascii_case("host")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("content-length")
            || name.eq_ignore_ascii_case("authorization")
            // SECURITY: strip any client-supplied `x-hermes-*` header. Hermes trusts
            // this controller as its authenticating gateway and reads `x-hermes-user`
            // (and other `x-hermes-*` trust headers) for identity/role. reqwest's
            // `.header()` appends rather than replaces, so without this a caller could
            // send `x-hermes-user: admin` / `x-hermes-roles: …` and have it forwarded
            // alongside the real identity → privilege escalation into Hermes.
            || name.to_ascii_lowercase().starts_with("x-hermes-")
        {
            continue;
        }
        if let Ok(val) = v.to_str() {
            rb = rb.header(k, val);
        }
    }
    // Set the trusted identity only after all client `x-hermes-*` headers were dropped.
    rb = rb.header("x-hermes-user", &user.username);
    if !body_bytes.is_empty() {
        rb = rb.body(body_bytes.to_vec());
    }

    let resp = rb.send().await.map_err(|e| {
        // Hermes not installed / not reachable is a "launchpad unavailable" state the
        // UI already handles gracefully — not a gateway fault. Only surface a 502 for
        // genuine upstream protocol errors from a reachable Hermes.
        if e.is_connect() || e.is_timeout() {
            proxy_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Launchpad backend (Hermes) is unreachable",
                "launchpad_unavailable",
                Some("Install/start Hermes and set HERMES_API_BASE on machina-controller."),
            )
        } else {
            proxy_error(
                StatusCode::BAD_GATEWAY,
                format!("Hermes proxy error: {e}"),
                "hermes_proxy_error",
                None,
            )
        }
    })?;

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut out = HeaderMap::new();
    for (k, v) in resp.headers() {
        if k == header::TRANSFER_ENCODING || k == header::CONNECTION {
            continue;
        }
        if let Ok(val) = HeaderValue::from_bytes(v.as_bytes()) {
            out.insert(k.clone(), val);
        }
    }
    let bytes = resp.bytes().await.map_err(|e| {
        proxy_error(StatusCode::BAD_GATEWAY, e.to_string(), "hermes_proxy_error", None)
    })?;

    Ok((status, out, bytes).into_response())
}

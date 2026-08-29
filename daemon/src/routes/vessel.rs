// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

//! Vessel REST API — local Podman/Docker containers and Podman pods.

use std::sync::Arc;

use axum::extract::{Extension, Path, Query};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use machina_core::{LibvirtError, LibvirtManager, VesselConfig};
use serde::Deserialize;
use serde_json::{json, Value};
use vessel_core::{CreateContainerRequest, CreatePodRequest, VesselError};

use crate::auth::{require_write, RequestActor};
use crate::error::{ok_json, AppError};
use crate::routes::events::{EventBus, MachinaEvent};
use crate::vessel_handle::VesselHandle;

fn map_vessel(err: VesselError) -> AppError {
    let code = err.code();
    let rem = err.remediation().unwrap_or("");
    let msg = if rem.is_empty() {
        err.to_string()
    } else {
        format!("{err} — {rem}")
    };
    match &err {
        VesselError::NotFound(_) => AppError::from(LibvirtError::NotFound(msg)),
        VesselError::Unsupported(_) => AppError::from(LibvirtError::Operation(format!(
            "[{code}] {msg}"
        ))),
        VesselError::Connection(_) => AppError::from(LibvirtError::Operation(format!(
            "[vessel_unavailable] {msg}"
        ))),
        VesselError::Api(_) => AppError::from(LibvirtError::Invalid(msg)),
        VesselError::Internal(_) => AppError::from(LibvirtError::Internal(msg)),
    }
}

fn map_unavailable(e: crate::vessel_handle::VesselUnavailable) -> AppError {
    AppError::from(LibvirtError::Operation(format!(
        "[vessel_unavailable] {} — Start Podman or Docker, or set [vessel].socket",
        e.message
    )))
}

async fn require_client(handle: &VesselHandle) -> Result<vessel_core::VesselClient, AppError> {
    handle.client().await.map_err(map_unavailable)
}

fn emit(bus: &EventBus, kind: &str, target: &str, status: &str) {
    bus.emit(MachinaEvent::now(kind, target, status));
}

async fn vessel_status(
    Extension(handle): Extension<VesselHandle>,
) -> Result<Json<Value>, AppError> {
    let snap = handle.status_snapshot().await;
    let mut out = json!({
        "enabled": snap.enabled,
        "connected": snap.connected,
        "socket": snap.socket,
        "error": snap.error,
    });
    if let Some(client) = snap.client {
        match client.host_info().await {
            Ok(info) => {
                out["engine"] = json!(info.engine);
                out["version"] = json!(info.version);
                out["name"] = json!(info.name);
                out["os"] = json!(info.os);
                out["arch"] = json!(info.arch);
                out["cpus"] = json!(info.cpus);
                out["memory_total"] = json!(info.memory_total);
                out["capabilities"] = json!(info.capabilities);
            }
            Err(e) => {
                out["error"] = json!(e.to_string());
                out["connected"] = json!(false);
            }
        }
    }
    Ok(Json(out))
}

async fn vessel_reconnect(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let cfg = MachinaConfigLoad::vessel();
    handle.try_connect(&cfg).await;
    emit(&bus, "vessel.reconnect", "local", "ok");
    vessel_status(Extension(handle)).await
}

/// Thin loader so reconnect picks up latest config.toml without threading MachinaConfig everywhere.
struct MachinaConfigLoad;
impl MachinaConfigLoad {
    fn vessel() -> VesselConfig {
        machina_core::MachinaConfig::load().vessel
    }
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    #[serde(default = "default_all")]
    all: bool,
}

fn default_all() -> bool {
    true
}

async fn list_containers(
    Extension(handle): Extension<VesselHandle>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, AppError> {
    let client = require_client(&handle).await?;
    let items = client.list_containers(q.all).await.map_err(map_vessel)?;
    Ok(Json(json!({ "items": items })))
}

async fn start_container(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.start_container(&id).await.map_err(map_vessel)?;
    emit(&bus, "vessel.container.started", &id, "ok");
    Ok(ok_json("started", &id))
}

async fn stop_container(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.stop_container(&id).await.map_err(map_vessel)?;
    emit(&bus, "vessel.container.stopped", &id, "ok");
    Ok(ok_json("stopped", &id))
}

async fn restart_container(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.restart_container(&id).await.map_err(map_vessel)?;
    emit(&bus, "vessel.container.restarted", &id, "ok");
    Ok(ok_json("restarted", &id))
}

#[derive(Debug, Deserialize)]
struct RemoveQuery {
    #[serde(default)]
    force: bool,
}

async fn remove_container(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
    Query(q): Query<RemoveQuery>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client
        .remove_container(&id, q.force)
        .await
        .map_err(map_vessel)?;
    emit(&bus, "vessel.container.removed", &id, "ok");
    Ok(ok_json("removed", &id))
}

async fn create_container(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Json(req): Json<CreateContainerRequest>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    let created = client.create_container(req).await.map_err(map_vessel)?;
    emit(
        &bus,
        "vessel.container.created",
        &created.name,
        "ok",
    );
    Ok(Json(json!(created)))
}

async fn list_pods(Extension(handle): Extension<VesselHandle>) -> Result<Json<Value>, AppError> {
    let client = require_client(&handle).await?;
    let items = client.list_pods().await.map_err(map_vessel)?;
    Ok(Json(json!({ "items": items })))
}

async fn create_pod(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Json(req): Json<CreatePodRequest>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    let created = client.create_pod(req).await.map_err(map_vessel)?;
    emit(&bus, "vessel.pod.created", &created.name, "ok");
    Ok(Json(json!(created)))
}

async fn start_pod(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.start_pod(&id).await.map_err(map_vessel)?;
    emit(&bus, "vessel.pod.started", &id, "ok");
    Ok(ok_json("started", &id))
}

async fn stop_pod(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.stop_pod(&id).await.map_err(map_vessel)?;
    emit(&bus, "vessel.pod.stopped", &id, "ok");
    Ok(ok_json("stopped", &id))
}

async fn remove_pod(
    Extension(actor): Extension<RequestActor>,
    Extension(handle): Extension<VesselHandle>,
    Extension(bus): Extension<Arc<EventBus>>,
    Path(id): Path<String>,
    Query(q): Query<RemoveQuery>,
) -> Result<Json<Value>, AppError> {
    require_write(&actor, "vessel")?;
    let client = require_client(&handle).await?;
    client.remove_pod(&id, q.force).await.map_err(map_vessel)?;
    emit(&bus, "vessel.pod.removed", &id, "ok");
    Ok(ok_json("removed", &id))
}

pub fn vessel_routes() -> Router<LibvirtManager> {
    Router::new()
        .route("/vessel/status", get(vessel_status))
        .route("/vessel/reconnect", post(vessel_reconnect))
        .route("/vessel/containers", get(list_containers).post(create_container))
        .route("/vessel/containers/{id}/start", post(start_container))
        .route("/vessel/containers/{id}/stop", post(stop_container))
        .route(
            "/vessel/containers/{id}/restart",
            post(restart_container),
        )
        .route("/vessel/containers/{id}", delete(remove_container))
        .route("/vessel/pods", get(list_pods).post(create_pod))
        .route("/vessel/pods/{id}/start", post(start_pod))
        .route("/vessel/pods/{id}/stop", post(stop_pod))
        .route("/vessel/pods/{id}", delete(remove_pod))
}

// ── WebSocket: stats + logs ─────────────────────────────────────────

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

async fn vessel_stats_ws(
    ws: WebSocketUpgrade,
    Extension(handle): Extension<VesselHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| vessel_stats_session(socket, handle, id))
}

async fn vessel_stats_session(socket: WebSocket, handle: VesselHandle, id: String) {
    let (mut sink, mut stream) = socket.split();
    let client = match handle.client().await {
        Ok(c) => c,
        Err(e) => {
            let _ = sink
                .send(Message::Text(
                    json!({"error": e.message, "error_code": "vessel_unavailable"}).to_string().into(),
                ))
                .await;
            return;
        }
    };

    let mut stats = client.stats_stream(&id);
    let mut client_gone = false;
    loop {
        tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => { client_gone = true; }
                    Some(Ok(Message::Ping(p))) => { let _ = sink.send(Message::Pong(p)).await; }
                    _ => {}
                }
                if client_gone { break; }
            }
            item = stats.next() => {
                match item {
                    Some(Ok(s)) => {
                        if sink.send(Message::Text(json!(s).to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        let _ = sink.send(Message::Text(
                            json!({"error": e.to_string(), "error_code": e.code()}).to_string().into()
                        )).await;
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

async fn vessel_logs_ws(
    ws: WebSocketUpgrade,
    Extension(handle): Extension<VesselHandle>,
    Path(id): Path<String>,
    Query(q): Query<LogsQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| vessel_logs_session(socket, handle, id, q))
}

#[derive(Debug, Deserialize)]
struct LogsQuery {
    #[serde(default = "default_follow")]
    follow: bool,
    #[serde(default = "default_tail")]
    tail: u64,
}

fn default_follow() -> bool {
    true
}
fn default_tail() -> u64 {
    200
}

async fn vessel_logs_session(socket: WebSocket, handle: VesselHandle, id: String, q: LogsQuery) {
    let (mut sink, mut stream) = socket.split();
    let client = match handle.client().await {
        Ok(c) => c,
        Err(e) => {
            let _ = sink
                .send(Message::Text(
                    json!({"error": e.message, "error_code": "vessel_unavailable"}).to_string().into(),
                ))
                .await;
            return;
        }
    };

    let mut logs = client.logs_stream(&id, q.follow, Some(q.tail));
    let mut client_gone = false;
    loop {
        tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => { client_gone = true; }
                    Some(Ok(Message::Ping(p))) => { let _ = sink.send(Message::Pong(p)).await; }
                    _ => {}
                }
                if client_gone { break; }
            }
            item = logs.next() => {
                match item {
                    Some(Ok(line)) => {
                        if sink.send(Message::Text(json!(line).to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        let _ = sink.send(Message::Text(
                            json!({"error": e.to_string(), "error_code": e.code()}).to_string().into()
                        )).await;
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

pub fn vessel_ws_routes() -> Router<LibvirtManager> {
    Router::new()
        .route("/vessel/containers/{id}/stats", get(vessel_stats_ws))
        .route("/vessel/containers/{id}/logs", get(vessel_logs_ws))
}
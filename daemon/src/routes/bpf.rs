// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// Native eBPF datapath on this host, via the local machina-bpfd socket.

use std::convert::Infallible;

use axum::extract::{Extension, Path, Query};
use axum::http::header;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use base64::Engine as _;
use futures_util::Stream;
use machina_bpf::api::{Mode, Policy, Request, TelemetryConfig};
use machina_bpf::BpfdClient;
use machina_core::{LibvirtError, LibvirtManager};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::RequestActor;
use crate::error::AppError;

fn require_admin(actor: &RequestActor, what: &str) -> Result<(), AppError> {
    if actor.role.is_admin() {
        Ok(())
    } else {
        Err(LibvirtError::Forbidden(format!("{what} requires the admin role.")).into())
    }
}

async fn bpfd(req: Request) -> Result<Json<Value>, AppError> {
    BpfdClient::from_env()
        .call(&req)
        .await
        .map(Json)
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")).into())
}

async fn status() -> Json<Value> {
    let client = BpfdClient::from_env();
    match client.call(&Request::Status).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({
            "available": false,
            "socket": client.socket_path(),
            "error": format!("{e:#}"),
            "probe": machina_core::bpf_probe::probe_bpf_summary(),
        })),
    }
}

async fn list_policies() -> Result<Json<Value>, AppError> {
    bpfd(Request::ListPolicies).await
}

async fn apply_policy(
    Extension(actor): Extension<RequestActor>,
    Json(policy): Json<Policy>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Changing runtime enforcement policies")?;
    bpfd(Request::ApplyPolicy { policy }).await
}

async fn remove_policy(
    Extension(actor): Extension<RequestActor>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Changing runtime enforcement policies")?;
    bpfd(Request::RemovePolicy { id }).await
}

#[derive(Deserialize)]
struct ModeBody {
    mode: Mode,
    lease_secs: Option<u64>,
}

async fn set_mode(
    Extension(actor): Extension<RequestActor>,
    Json(b): Json<ModeBody>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Switching enforcement mode")?;
    bpfd(Request::SetMode {
        mode: b.mode,
        lease_secs: b.lease_secs,
    })
    .await
}

async fn list_interfaces() -> Result<Json<Value>, AppError> {
    bpfd(Request::ListInterfaces).await
}

#[derive(Deserialize)]
struct AttachBody {
    name: String,
    #[serde(default)]
    guest_side: bool,
    #[serde(default)]
    xdp: bool,
}

async fn attach_interface(
    Extension(actor): Extension<RequestActor>,
    Json(b): Json<AttachBody>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Attaching the eBPF datapath")?;
    bpfd(Request::AttachInterface {
        name: b.name,
        guest_side: b.guest_side,
        xdp: b.xdp,
    })
    .await
}

async fn detach_interface(
    Extension(actor): Extension<RequestActor>,
    Path(name): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Detaching the eBPF datapath")?;
    bpfd(Request::DetachInterface { name }).await
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
    kind: Option<String>,
    vm: Option<String>,
}

async fn flows(Query(q): Query<ListQuery>) -> Result<Json<Value>, AppError> {
    bpfd(Request::Flows { limit: q.limit, vm: q.vm }).await
}

async fn events(Query(q): Query<ListQuery>) -> Result<Json<Value>, AppError> {
    bpfd(Request::Events { limit: q.limit, kind: q.kind }).await
}

async fn dns(Query(q): Query<ListQuery>) -> Result<Json<Value>, AppError> {
    bpfd(Request::Dns { limit: q.limit }).await
}

async fn processes(Query(q): Query<ListQuery>) -> Result<Json<Value>, AppError> {
    bpfd(Request::ProcEvents { limit: q.limit, kind: q.kind }).await
}

async fn anomalies(Query(q): Query<ListQuery>) -> Result<Json<Value>, AppError> {
    bpfd(Request::Anomalies { limit: q.limit }).await
}

async fn net_health() -> Result<Json<Value>, AppError> {
    bpfd(Request::NetHealth).await
}

#[derive(Deserialize)]
struct CaptureBody {
    iface: String,
    duration_secs: Option<u64>,
    sample: Option<u32>,
    snaplen: Option<u32>,
    max_packets: Option<usize>,
}

async fn capture_start(
    Extension(actor): Extension<RequestActor>,
    Json(b): Json<CaptureBody>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Starting a packet capture")?;
    bpfd(Request::CaptureStart {
        iface: b.iface,
        duration_secs: b.duration_secs,
        sample: b.sample,
        snaplen: b.snaplen,
        max_packets: b.max_packets,
    })
    .await
}

async fn capture_list() -> Result<Json<Value>, AppError> {
    bpfd(Request::CaptureList).await
}

/// pcapng download (opens in Wireshark).
async fn capture_get(
    Extension(actor): Extension<RequestActor>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    require_admin(&actor, "Downloading packet captures")?;
    let Json(v) = bpfd(Request::CaptureGet { id: id.clone() }).await?;
    let b64 = v["pcapng_base64"].as_str().unwrap_or("");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| LibvirtError::Internal(format!("invalid capture payload: {e}")))?;
    let safe: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-pcapng".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"machina-{safe}.pcapng\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Deserialize)]
struct QosBody {
    iface: Option<String>,
    vm: Option<String>,
    #[serde(default)]
    egress_bps: u64,
    #[serde(default)]
    ingress_bps: u64,
}

async fn set_qos(
    Extension(actor): Extension<RequestActor>,
    Json(b): Json<QosBody>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Changing VM bandwidth limits")?;
    bpfd(Request::SetQos {
        iface: b.iface,
        vm: b.vm,
        egress_bps: b.egress_bps,
        ingress_bps: b.ingress_bps,
    })
    .await
}

async fn get_telemetry() -> Result<Json<Value>, AppError> {
    bpfd(Request::GetTelemetry).await
}

async fn set_telemetry(
    Extension(actor): Extension<RequestActor>,
    Json(telemetry): Json<TelemetryConfig>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Changing eBPF telemetry")?;
    bpfd(Request::SetTelemetry { telemetry }).await
}

#[derive(Deserialize)]
struct StreamQuery {
    /// Comma-separated: net, flow, dns, proc, anomaly. Empty = all.
    topics: Option<String>,
}

/// Live events as Server-Sent Events.
async fn stream(
    Query(q): Query<StreamQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let topics: Vec<String> = q
        .topics
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect();
    let refs: Vec<&str> = topics.iter().map(String::as_str).collect();
    let rx = BpfdClient::from_env()
        .subscribe(&refs)
        .await
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")))?;
    let events = futures_util::StreamExt::map(tokio_stream::wrappers::ReceiverStream::new(rx), |ev| {
        Ok(Event::default()
            .event(ev.topic)
            .data(ev.event.to_string()))
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

pub fn bpf_routes() -> Router<LibvirtManager> {
    Router::new()
        .route("/bpf/status", get(status))
        .route("/bpf/policies", get(list_policies).post(apply_policy))
        .route("/bpf/policies/{id}", delete(remove_policy))
        .route("/bpf/mode", put(set_mode))
        .route("/bpf/interfaces", get(list_interfaces).post(attach_interface))
        .route("/bpf/interfaces/{name}", delete(detach_interface))
        .route("/bpf/flows", get(flows))
        .route("/bpf/events", get(events))
        .route("/bpf/dns", get(dns))
        .route("/bpf/processes", get(processes))
        .route("/bpf/anomalies", get(anomalies))
        .route("/bpf/health", get(net_health))
        .route("/bpf/captures", get(capture_list).post(capture_start))
        .route("/bpf/captures/{id}", get(capture_get))
        .route("/bpf/qos", put(set_qos))
        .route("/bpf/telemetry", get(get_telemetry).put(set_telemetry))
        .route("/bpf/stream", get(stream))
}

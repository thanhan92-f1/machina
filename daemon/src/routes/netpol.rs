// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// VM network policies (CiliumNetworkPolicy schema) for this host, compiled
// into the local machina-bpfd VM edge, plus VM labels and the flow API.
// When the controller owns the host's VM edge, local policies stay stored
// but inactive so the two writers never replace each other's state.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::sync::{Mutex, OnceLock};

use axum::body::Bytes;
use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::Stream;
use machina_bpf::api::{Request, VmEdgeStatus, VmFlowRecord};
use machina_bpf::netpol::{self, FlowFilter, Inputs, NetpolVm, TraceQuery, VmNetworkPolicy};
use machina_bpf::BpfdClient;
use machina_core::{LibvirtError, LibvirtManager};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::bpf::require_admin;
use crate::auth::RequestActor;
use crate::error::AppError;

const STORE: &str = "/var/lib/machina/vm-netpol.json";
const OWNER: &str = "daemon";
const RESYNC_SECS: u64 = 60;

#[derive(Serialize, Deserialize, Default)]
struct Store {
    policies: Vec<VmNetworkPolicy>,
    /// The local bpfd VM edge currently carries our compiled state.
    #[serde(default)]
    synced: bool,
}

#[derive(Serialize, Clone, Default)]
struct SyncReport {
    at: Option<String>,
    ok: bool,
    error: Option<String>,
    /// Why nothing was pushed (no policies, controller-managed, ...).
    skipped: Option<String>,
    vms: usize,
    rules: usize,
    peers: usize,
    warnings: Vec<String>,
}

static STORE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static LAST: Mutex<Option<SyncReport>> = Mutex::new(None);
static INVENTORY: Mutex<Vec<NetpolVm>> = Mutex::new(Vec::new());
static MANAGER: OnceLock<LibvirtManager> = OnceLock::new();

fn load_store() -> Store {
    std::fs::read_to_string(STORE).ok().and_then(|d| serde_json::from_str(&d).ok()).unwrap_or_default()
}

fn save_store(s: &Store) -> Result<(), LibvirtError> {
    if let Some(dir) = std::path::Path::new(STORE).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let data = serde_json::to_string_pretty(s).map_err(LibvirtError::map_op("serialize policies"))?;
    let tmp = format!("{STORE}.tmp");
    std::fs::write(&tmp, data).map_err(LibvirtError::map_op("write policies"))?;
    std::fs::rename(&tmp, STORE).map_err(LibvirtError::map_op("write policies"))?;
    Ok(())
}

/// Global unicast addresses of this host (`host` entity).
fn host_addresses() -> Vec<String> {
    let Ok(out) = std::process::Command::new("ip").args(["-j", "addr", "show"]).output() else { return Vec::new() };
    let v: Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let mut addrs = Vec::new();
    for link in v.as_array().into_iter().flatten() {
        if link["ifname"].as_str() == Some("lo") {
            continue;
        }
        for a in link["addr_info"].as_array().into_iter().flatten() {
            if a["scope"].as_str() == Some("global") {
                if let Some(ip) = a["local"].as_str() {
                    addrs.push(ip.to_string());
                }
            }
        }
    }
    addrs.sort();
    addrs.dedup();
    addrs
}

fn usable_addr(a: &str) -> bool {
    match a.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified(),
        Ok(std::net::IpAddr::V6(v6)) => !v6.is_loopback() && (v6.segments()[0] & 0xffc0) != 0xfe80,
        Err(_) => false,
    }
}

/// Local VMs with labels and guest addresses (lease / ARP / agent).
fn inventory(m: &LibvirtManager) -> Vec<NetpolVm> {
    let labels = machina_core::libvirt::extras::load_labels();
    let vms = m.list_all_vms().unwrap_or_default();
    vms.into_iter()
        .map(|vm| {
            let mut addresses: Vec<String> = Vec::new();
            if vm.state == "running" {
                if let Ok(rows) = m.with_conn(|c| machina_core::libvirt::guest_agent::get_guest_interfaces(c, &vm.name)) {
                    addresses.extend(rows.into_iter().map(|r| r.address));
                }
                addresses.extend(vm.guest_ip.clone());
            }
            addresses.retain(|a| usable_addr(a));
            addresses.sort();
            addresses.dedup();
            NetpolVm { labels: labels.get(&vm.name).cloned().unwrap_or_default(), name: vm.name, addresses, ..Default::default() }
        })
        .collect()
}

async fn refresh_inventory(m: &LibvirtManager) -> Vec<NetpolVm> {
    let m = m.clone();
    let inv = tokio::task::spawn_blocking(move || inventory(&m)).await.unwrap_or_default();
    *INVENTORY.lock().unwrap_or_else(|e| e.into_inner()) = inv.clone();
    inv
}

async fn cached_inventory(m: &LibvirtManager) -> Vec<NetpolVm> {
    let cached = INVENTORY.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if cached.is_empty() {
        refresh_inventory(m).await
    } else {
        cached
    }
}

async fn edge_status() -> Option<VmEdgeStatus> {
    let v = BpfdClient::from_env().call(&Request::VmEdgeStatus).await.ok()?;
    serde_json::from_value(v).ok()
}

/// Compile the stored policies and push them to the local bpfd.
async fn sync(m: &LibvirtManager) -> SyncReport {
    let _g = STORE_LOCK.lock().await;
    let mut store = load_store();
    let mut rep = SyncReport { at: Some(chrono::Utc::now().to_rfc3339()), ..Default::default() };
    if store.policies.is_empty() && !store.synced {
        rep.ok = true;
        rep.skipped = Some("no policies".into());
    } else if edge_status().await.is_some_and(|s| s.owner == "controller") {
        rep.ok = true;
        rep.skipped = Some("the controller manages this host's VM edge; local policies are inactive".into());
    } else {
        let inv = refresh_inventory(m).await;
        let hosts = tokio::task::spawn_blocking(host_addresses).await.unwrap_or_default();
        let c = netpol::compile(&Inputs {
            policies: &store.policies,
            vms: &inv,
            host: None,
            host_addresses: &hosts,
            remote_node_addresses: &[],
        });
        let mut state = c.state;
        state.owner = if store.policies.is_empty() { String::new() } else { OWNER.into() };
        if store.policies.is_empty() {
            state.flow_log = false;
        }
        rep.vms = state.vms.len();
        rep.rules = state.policy.len();
        rep.peers = state.peers.len();
        rep.warnings = c.warnings;
        match BpfdClient::from_env().call(&Request::VmEdgeSync { state }).await {
            Ok(_) => {
                rep.ok = true;
                store.synced = !store.policies.is_empty();
                if let Err(e) = save_store(&store) {
                    tracing::warn!("vm netpol store: {e}");
                }
            }
            Err(e) => rep.error = Some(format!("{e:#}")),
        }
    }
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(rep.clone());
    rep
}

/// Periodic resync (guest addresses change with DHCP) and lifecycle hooks.
pub fn spawn_resync_loop(m: LibvirtManager) {
    let _ = MANAGER.set(m.clone());
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        loop {
            let rep = sync(&m).await;
            if let Some(e) = rep.error {
                tracing::debug!("vm netpol sync: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(RESYNC_SECS)).await;
        }
    });
}

/// Called after VM start/stop/label changes.
pub fn trigger_resync() {
    if let Some(m) = MANAGER.get().cloned() {
        tokio::spawn(async move {
            sync(&m).await;
        });
    }
}

fn bad_request(msg: &str, v: &netpol::Validation) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg, "errors": v.errors, "warnings": v.warnings }))).into_response()
}

/// Body: raw YAML/JSON policy text, or `{"yaml": "..."}`.
fn body_text(body: &Bytes) -> String {
    let text = String::from_utf8_lossy(body).into_owned();
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) if m.get("yaml").is_some_and(Value::is_string) => {
            m["yaml"].as_str().unwrap_or_default().to_string()
        }
        _ => text,
    }
}

fn policy_json(p: &VmNetworkPolicy, selected: Option<&Vec<String>>) -> Value {
    json!({
        "name": p.name,
        "kind": p.kind,
        "description": p.description(),
        "labels": p.labels,
        "annotations": p.annotations,
        "specs": p.specs,
        "yaml": p.to_yaml(),
        "selected_vms": selected.cloned().unwrap_or_default(),
    })
}

fn selected_map(c: &netpol::Compiled) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for e in &c.endpoints {
        for p in &e.policies {
            out.entry(p.clone()).or_default().push(e.name.clone());
        }
    }
    out
}

async fn compile_cached(m: &LibvirtManager, policies: &[VmNetworkPolicy]) -> netpol::Compiled {
    let inv = cached_inventory(m).await;
    let hosts = tokio::task::spawn_blocking(host_addresses).await.unwrap_or_default();
    netpol::compile(&Inputs { policies, vms: &inv, host: None, host_addresses: &hosts, remote_node_addresses: &[] })
}

async fn list(State(m): State<LibvirtManager>) -> Result<Json<Value>, AppError> {
    let store = load_store();
    let c = compile_cached(&m, &store.policies).await;
    let sel = selected_map(&c);
    let items: Vec<Value> = store.policies.iter().map(|p| policy_json(p, sel.get(&p.name))).collect();
    Ok(Json(json!({ "items": items, "warnings": c.warnings })))
}

#[derive(Deserialize, Default)]
struct ApplyQuery {
    #[serde(default)]
    dry_run: Option<String>,
}

fn truthy(s: &Option<String>) -> bool {
    matches!(s.as_deref(), Some("1" | "true" | "yes" | ""))
}

async fn preview(m: &LibvirtManager, parsed: &[VmNetworkPolicy], v: &netpol::Validation) -> Value {
    let mut all = load_store().policies;
    all.retain(|p| !parsed.iter().any(|n| n.name == p.name));
    all.extend(parsed.iter().cloned());
    let c = compile_cached(m, &all).await;
    let sel = selected_map(&c);
    json!({
        "valid": v.ok(),
        "errors": v.errors,
        "warnings": v.warnings,
        "compile_warnings": c.warnings,
        "policies": parsed.iter().map(|p| policy_json(p, sel.get(&p.name))).collect::<Vec<_>>(),
        "rules": c.state.policy.len(),
        "endpoints": c.endpoints,
        "selectors": c.selectors.into_iter().filter(|s| parsed.iter().any(|p| p.name == s.policy)).collect::<Vec<_>>(),
    })
}

async fn apply(
    State(m): State<LibvirtManager>,
    Extension(actor): Extension<RequestActor>,
    Query(q): Query<ApplyQuery>,
    body: Bytes,
) -> Result<Response, AppError> {
    let (parsed, v) = netpol::parse_documents(&body_text(&body));
    if truthy(&q.dry_run) {
        return Ok(Json(preview(&m, &parsed, &v).await).into_response());
    }
    require_admin(&actor, "Changing VM network policies")?;
    if !v.ok() {
        return Ok(bad_request("invalid policy", &v));
    }
    let names: Vec<String> = parsed.iter().map(|p| p.name.clone()).collect();
    {
        let _g = STORE_LOCK.lock().await;
        let mut store = load_store();
        let mut created = Vec::new();
        for p in parsed {
            match store.policies.iter_mut().find(|x| x.name == p.name) {
                Some(x) => *x = p,
                None => {
                    created.push(p.name.clone());
                    store.policies.push(p);
                }
            }
        }
        store.policies.sort_by(|a, b| a.name.cmp(&b.name));
        save_store(&store)?;
        tracing::info!(actor = %actor.username, policies = ?names, "vm network policies applied");
    }
    let rep = sync(&m).await;
    Ok(Json(json!({ "applied": names, "warnings": v.warnings, "sync": rep })).into_response())
}

async fn validate(State(m): State<LibvirtManager>, body: Bytes) -> Json<Value> {
    let (parsed, v) = netpol::parse_documents(&body_text(&body));
    Json(preview(&m, &parsed, &v).await)
}

#[derive(Deserialize, Default)]
struct GetQuery {
    #[serde(default)]
    format: Option<String>,
}

async fn get_one(
    State(m): State<LibvirtManager>,
    Path(name): Path<String>,
    Query(q): Query<GetQuery>,
) -> Result<Response, AppError> {
    let store = load_store();
    let p = store
        .policies
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| LibvirtError::NotFound(format!("VM network policy `{name}`")))?;
    if q.format.as_deref() == Some("yaml") {
        return Ok(([(header::CONTENT_TYPE, "application/yaml")], p.to_yaml()).into_response());
    }
    let c = compile_cached(&m, &store.policies).await;
    let sel = selected_map(&c);
    Ok(Json(policy_json(p, sel.get(&p.name))).into_response())
}

async fn delete_one(
    State(m): State<LibvirtManager>,
    Extension(actor): Extension<RequestActor>,
    Path(name): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_admin(&actor, "Changing VM network policies")?;
    {
        let _g = STORE_LOCK.lock().await;
        let mut store = load_store();
        let before = store.policies.len();
        store.policies.retain(|p| p.name != name);
        if store.policies.len() == before {
            return Err(LibvirtError::NotFound(format!("VM network policy `{name}`")).into());
        }
        save_store(&store)?;
    }
    let rep = sync(&m).await;
    Ok(Json(json!({ "deleted": name, "sync": rep })))
}

async fn trace(State(m): State<LibvirtManager>, Json(q): Json<TraceQuery>) -> Result<Json<Value>, AppError> {
    let store = load_store();
    let inv = cached_inventory(&m).await;
    let hosts = tokio::task::spawn_blocking(host_addresses).await.unwrap_or_default();
    let r = netpol::trace(&store.policies, &inv, &hosts, &[], &q).map_err(LibvirtError::Invalid)?;
    Ok(Json(serde_json::to_value(r).unwrap_or_default()))
}

async fn endpoints(State(m): State<LibvirtManager>) -> Json<Value> {
    let c = compile_cached(&m, &load_store().policies).await;
    Json(json!({ "items": c.endpoints }))
}

async fn selectors(State(m): State<LibvirtManager>) -> Json<Value> {
    let c = compile_cached(&m, &load_store().policies).await;
    Json(json!({ "items": c.selectors }))
}

async fn status() -> Json<Value> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let edge = edge_status().await;
    let mode = BpfdClient::from_env().call(&Request::Status).await.ok().map(|s| {
        json!({ "mode": s["mode"], "lease_remaining_secs": s["lease_remaining_secs"] })
    });
    let store = load_store();
    let managed_by = match edge.as_ref().map(|e| e.owner.as_str()) {
        Some("controller") => "controller",
        _ => "local",
    };
    Json(json!({
        "policies": store.policies.len(),
        "managed_by": managed_by,
        "last_sync": last,
        "edge": edge,
        "enforcement": mode,
        "bpfd_available": edge.is_some(),
        "cilium": netpol::cilium_present(),
    }))
}

async fn fqdn_cache() -> Result<Json<Value>, AppError> {
    let v = BpfdClient::from_env()
        .call(&Request::VmFqdnCache)
        .await
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")))?;
    Ok(Json(json!({ "items": v })))
}

async fn auth_table() -> Result<Json<Value>, AppError> {
    let v = BpfdClient::from_env()
        .call(&Request::VmAuthTable)
        .await
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")))?;
    Ok(Json(json!({ "items": v })))
}

#[derive(Deserialize, Default)]
struct FlowQuery {
    limit: Option<usize>,
    /// Backfill this many recent flows before streaming.
    last: Option<usize>,
    vm: Option<String>,
    from_vm: Option<String>,
    to_vm: Option<String>,
    label: Option<String>,
    ip: Option<String>,
    cidr: Option<String>,
    port: Option<String>,
    protocol: Option<String>,
    verdict: Option<String>,
    drop_reason: Option<String>,
    policy: Option<String>,
    direction: Option<String>,
}

impl FlowQuery {
    fn filter(&self) -> FlowFilter {
        let s = |v: &Option<String>| v.clone().filter(|x| !x.is_empty());
        FlowFilter {
            vm: s(&self.vm),
            from_vm: s(&self.from_vm),
            to_vm: s(&self.to_vm),
            label: s(&self.label),
            ip: s(&self.ip),
            cidr: s(&self.cidr),
            port: self.port.as_deref().and_then(|p| p.parse().ok()),
            protocol: s(&self.protocol),
            verdict: s(&self.verdict),
            drop_reason: s(&self.drop_reason),
            policy: s(&self.policy),
            direction: s(&self.direction),
            host: None,
        }
    }
}

async fn recent_flows(f: &FlowFilter, limit: usize) -> Result<Vec<VmFlowRecord>, AppError> {
    let v = BpfdClient::from_env()
        .call(&Request::VmFlows { limit: Some(5000), vm: None, verdict: None })
        .await
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")))?;
    let all: Vec<VmFlowRecord> = serde_json::from_value(v).unwrap_or_default();
    Ok(all.into_iter().filter(|r| f.matches(r)).take(limit).collect())
}

async fn flows(Query(q): Query<FlowQuery>) -> Result<Json<Value>, AppError> {
    let items = recent_flows(&q.filter(), q.limit.unwrap_or(200).min(5000)).await?;
    Ok(Json(json!({ "items": items })))
}

async fn flow_stream(Query(q): Query<FlowQuery>) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let f = q.filter();
    let mut backfill = match q.last.filter(|n| *n > 0) {
        Some(n) => recent_flows(&f, n.min(5000)).await.unwrap_or_default(),
        None => Vec::new(),
    };
    backfill.reverse();
    let rx = BpfdClient::from_env()
        .subscribe(&["flow"])
        .await
        .map_err(|e| LibvirtError::Operation(format!("{e:#}")))?;
    let ev = |r: &VmFlowRecord| Event::default().event("flow").data(serde_json::to_string(r).unwrap_or_default());
    let head = futures_util::stream::iter(backfill.iter().map(ev).map(Ok).collect::<Vec<_>>());
    let live = futures_util::StreamExt::filter_map(tokio_stream::wrappers::ReceiverStream::new(rx), move |e| {
        let out = serde_json::from_value::<VmFlowRecord>(e.event).ok().filter(|r| f.matches(r)).map(|r| Ok(ev(&r)));
        std::future::ready(out)
    });
    Ok(Sse::new(futures_util::StreamExt::chain(head, live)).keep_alive(KeepAlive::default()))
}

async fn get_labels(Path(name): Path<String>) -> Json<Value> {
    let labels = machina_core::libvirt::extras::get_vm_labels(&name);
    Json(json!({ "name": name, "labels": labels }))
}

#[derive(Deserialize)]
struct LabelsBody {
    labels: BTreeMap<String, String>,
}

async fn put_labels(
    Extension(actor): Extension<RequestActor>,
    Path(name): Path<String>,
    Json(b): Json<LabelsBody>,
) -> Result<Json<Value>, AppError> {
    crate::auth::require_write(&actor, "vms:write")?;
    if name.contains('/') || name.contains("..") {
        return Err(LibvirtError::Invalid("bad VM name".into()).into());
    }
    machina_core::libvirt::extras::set_vm_labels(&name, b.labels.clone())?;
    if let Ok(mut inv) = INVENTORY.lock() {
        if let Some(vm) = inv.iter_mut().find(|v| v.name == name) {
            vm.labels = b.labels.clone();
        }
    }
    trigger_resync();
    Ok(Json(json!({ "status": "ok", "name": name, "labels": b.labels })))
}

pub fn netpol_routes() -> Router<LibvirtManager> {
    Router::new()
        .route("/vm-network-policies", get(list).post(apply))
        .route("/vm-network-policies/validate", post(validate))
        .route("/vm-network-policies/trace", post(trace))
        .route("/vm-network-policies/endpoints", get(endpoints))
        .route("/vm-network-policies/selectors", get(selectors))
        .route("/vm-network-policies/status", get(status))
        .route("/vm-network-policies/fqdn-cache", get(fqdn_cache))
        .route("/vm-network-policies/auth", get(auth_table))
        .route("/vm-network-policies/{name}", get(get_one).delete(delete_one))
        .route("/flows", get(flows))
        .route("/flows/stream", get(flow_stream))
        .route("/vms/{name}/labels", get(get_labels).put(put_labels))
}

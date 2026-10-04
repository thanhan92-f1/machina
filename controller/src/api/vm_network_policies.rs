// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Fleet VM network policies (CiliumNetworkPolicy schema), VM labels and
//! the fleet flow API (Hubble-style, polled from every host's bpfd).

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use futures_util::stream::Stream;
use machina_bpf::api::{
    Request, VmEdgeStatus, VmFlowAlert, VmFlowEdge, VmFlowRecord, VmQuarantineBody,
};
use machina_bpf::netpol::{
    self, FlowFilter, LearnOptions, ReplayInputs, TraceQuery, VmNetworkPolicy,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::api::ApiError;
use crate::auth::{require_admin, require_operator, AuthUser};
use crate::engine::bpf;
use crate::engine::vm_netpol::{self, Fleet};
use crate::state::AppState;

fn body_text(body: &Bytes) -> String {
    let text = String::from_utf8_lossy(body).into_owned();
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) if m.get("yaml").is_some_and(Value::is_string) => {
            m["yaml"].as_str().unwrap_or_default().into()
        }
        _ => text,
    }
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

fn policy_json(
    p: &VmNetworkPolicy,
    enabled: bool,
    generation: i64,
    updated: &str,
    sel: Option<&Vec<String>>,
) -> Value {
    json!({
        "name": p.name,
        "kind": p.kind,
        "description": p.description(),
        "labels": p.labels,
        "annotations": p.annotations,
        "specs": p.specs,
        "yaml": p.to_yaml(),
        "enabled": enabled,
        "generation": generation,
        "updated_at": updated,
        "selected_vms": sel.cloned().unwrap_or_default(),
    })
}

pub async fn list(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let rows = vm_netpol::policies(&state.pool).await?;
    let fleet = Fleet::load(&state.pool).await;
    let c = fleet.compile(None);
    let sel = selected_map(&c);
    let items: Vec<Value> = rows
        .iter()
        .map(|(p, en, g, u)| policy_json(p, *en, *g, u, sel.get(&p.name)))
        .collect();
    Ok(Json(json!({ "items": items, "warnings": c.warnings })))
}

#[derive(Deserialize, Default)]
pub struct ApplyQuery {
    #[serde(default)]
    dry_run: Option<String>,
}

async fn preview(state: &AppState, parsed: &[VmNetworkPolicy], v: &netpol::Validation) -> Value {
    let mut fleet = Fleet::load(&state.pool).await;
    fleet
        .policies
        .retain(|p| !parsed.iter().any(|n| n.name == p.name));
    fleet.policies.extend(parsed.iter().cloned());
    let c = fleet.compile(None);
    let sel = selected_map(&c);
    json!({
        "valid": v.ok(),
        "errors": v.errors,
        "warnings": v.warnings,
        "compile_warnings": c.warnings,
        "policies": parsed.iter().map(|p| policy_json(p, true, 0, "", sel.get(&p.name))).collect::<Vec<_>>(),
        "rules": c.state.policy.len(),
        "endpoints": c.endpoints,
        "selectors": c.selectors.into_iter().filter(|s| parsed.iter().any(|p| p.name == s.policy)).collect::<Vec<_>>(),
    })
}

pub async fn apply(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Query(q): Query<ApplyQuery>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let (parsed, v) = netpol::parse_documents(&body_text(&body));
    if matches!(q.dry_run.as_deref(), Some("1" | "true" | "yes" | "")) {
        return Ok(Json(preview(&state, &parsed, &v).await).into_response());
    }
    require_admin(&actor)?;
    if !v.ok() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid policy", "errors": v.errors, "warnings": v.warnings })),
        )
            .into_response());
    }
    let mut created = Vec::new();
    for p in &parsed {
        if vm_netpol::upsert(&state.pool, p, &actor.username).await? {
            created.push(p.name.clone());
        }
    }
    let names: Vec<String> = parsed.iter().map(|p| p.name.clone()).collect();
    tracing::info!(actor = %actor.username, policies = ?names, "vm network policies applied");
    let sync = vm_netpol::reconcile(&state.pool, false).await;
    Ok(
        Json(json!({ "applied": names, "created": created, "warnings": v.warnings, "sync": sync }))
            .into_response(),
    )
}

pub async fn validate(State(state): State<AppState>, body: Bytes) -> Json<Value> {
    let (parsed, v) = netpol::parse_documents(&body_text(&body));
    Json(preview(&state, &parsed, &v).await)
}

#[derive(Deserialize, Default)]
pub struct GetQuery {
    #[serde(default)]
    format: Option<String>,
}

pub async fn get_one(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<GetQuery>,
) -> Result<Response, ApiError> {
    let rows = vm_netpol::policies(&state.pool).await?;
    let (p, en, g, u) = rows
        .iter()
        .find(|r| r.0.name == name)
        .ok_or_else(|| ApiError::not_found(format!("VM network policy `{name}`")))?;
    if q.format.as_deref() == Some("yaml") {
        return Ok(([(header::CONTENT_TYPE, "application/yaml")], p.to_yaml()).into_response());
    }
    let c = Fleet::load(&state.pool).await.compile(None);
    let sel = selected_map(&c);
    Ok(Json(policy_json(p, *en, *g, u, sel.get(&p.name))).into_response())
}

pub async fn delete_one(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    if !vm_netpol::delete(&state.pool, &name).await? {
        return Err(ApiError::not_found(format!("VM network policy `{name}`")));
    }
    let sync = vm_netpol::reconcile(&state.pool, false).await;
    Ok(Json(json!({ "deleted": name, "sync": sync })))
}

#[derive(Deserialize)]
pub struct EnabledBody {
    enabled: bool,
}

pub async fn set_enabled(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(name): Path<String>,
    Json(b): Json<EnabledBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    if !vm_netpol::set_enabled(&state.pool, &name, b.enabled).await? {
        return Err(ApiError::not_found(format!("VM network policy `{name}`")));
    }
    let sync = vm_netpol::reconcile(&state.pool, false).await;
    Ok(Json(
        json!({ "name": name, "enabled": b.enabled, "sync": sync }),
    ))
}

pub async fn trace(
    State(state): State<AppState>,
    Json(q): Json<TraceQuery>,
) -> Result<Json<Value>, ApiError> {
    let fleet = Fleet::load(&state.pool).await;
    let r = netpol::trace(
        &fleet.policies,
        &fleet.vms,
        &fleet.services,
        &fleet.all_host_addresses(),
        &[],
        &q,
    )
    .map_err(ApiError::bad_request)?;
    Ok(Json(serde_json::to_value(r).unwrap_or_default()))
}

pub async fn endpoints(State(state): State<AppState>) -> Json<Value> {
    let c = Fleet::load(&state.pool).await.compile(None);
    Json(json!({ "items": c.endpoints }))
}

pub async fn selectors(State(state): State<AppState>) -> Json<Value> {
    let c = Fleet::load(&state.pool).await.compile(None);
    Json(json!({ "items": c.selectors }))
}

pub async fn status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    type Row = (
        String,
        String,
        Option<String>,
        bool,
        Option<String>,
        i64,
        i64,
        i64,
        String,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT host_id, hostname, synced_at, ok, error, vms, rules, peers, warnings FROM vm_netpol_host_status ORDER BY hostname",
    )
    .fetch_all(&state.pool)
    .await?;
    let synced: HashMap<String, Value> = rows
        .into_iter()
        .map(|(id, hn, at, ok, err, vms, rules, peers, w)| {
            let warnings: Vec<String> = serde_json::from_str(&w).unwrap_or_default();
            (id.clone(), json!({ "host_id": id, "hostname": hn, "synced_at": at, "ok": ok, "error": err, "vms": vms, "rules": rules, "peers": peers, "warnings": warnings }))
        })
        .collect();
    let mut hosts = Vec::new();
    for (h, res) in bpf::fan_out(&state.pool, &Request::VmEdgeStatus).await {
        let edge: Option<VmEdgeStatus> = res.ok().and_then(|v| serde_json::from_value(v).ok());
        let mut row = synced
            .get(&h.id)
            .cloned()
            .unwrap_or_else(|| json!({ "host_id": h.id, "hostname": h.hostname }));
        row["reachable"] = json!(edge.is_some());
        row["owner"] = json!(edge.as_ref().map(|e| e.owner.clone()));
        row["enforcing"] = json!(edge.as_ref().is_some_and(|e| e.enforcing));
        row["taps"] = json!(edge.as_ref().map_or(0, |e| e.taps.len()));
        row["missing"] = json!(edge.as_ref().map(|e| e.missing.clone()).unwrap_or_default());
        row["cilium"] = json!(edge.as_ref().and_then(|e| e.cilium.clone()));
        hosts.push(row);
    }
    let policies = vm_netpol::policies(&state.pool).await?.len();
    Ok(Json(
        json!({ "policies": policies, "managed_by": "controller", "hosts": hosts }),
    ))
}

pub async fn sync_now(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    Ok(Json(
        json!({ "sync": vm_netpol::reconcile(&state.pool, true).await }),
    ))
}

// ---- labels -------------------------------------------------------------------

pub async fn get_labels(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, labels FROM vms WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    let (name, labels) = row.ok_or_else(|| ApiError::not_found("vm not found"))?;
    let labels: BTreeMap<String, String> = labels
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    Ok(Json(json!({ "id": id, "name": name, "labels": labels })))
}

#[derive(Deserialize)]
pub struct LabelsBody {
    pub labels: BTreeMap<String, String>,
}

pub fn validate_labels(labels: &BTreeMap<String, String>) -> Result<(), ApiError> {
    let part = |s: &str, max: usize| {
        !s.is_empty()
            && s.len() <= max
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    };
    for (k, v) in labels {
        let name = match k.split_once('/') {
            Some((p, n)) if part(p, 253) => n,
            Some(_) => {
                return Err(ApiError::bad_request(format!(
                    "label key `{k}`: bad prefix"
                )))
            }
            None => k.as_str(),
        };
        if !part(name, 63) {
            return Err(ApiError::bad_request(format!(
                "label key `{k}` is not a valid label name"
            )));
        }
        if !v.is_empty() && !part(v, 63) {
            return Err(ApiError::bad_request(format!(
                "label `{k}`: value `{v}` is not a valid label value"
            )));
        }
    }
    Ok(())
}

pub async fn put_labels(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(id): Path<Uuid>,
    Json(b): Json<LabelsBody>,
) -> Result<Json<Value>, ApiError> {
    require_operator(&actor)?;
    validate_labels(&b.labels)?;
    let r = sqlx::query("UPDATE vms SET labels = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(serde_json::to_string(&b.labels).unwrap_or_else(|_| "{}".into()))
        .bind(id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ApiError::not_found("vm not found"));
    }
    let pool = state.pool.clone();
    tokio::spawn(async move {
        vm_netpol::reconcile(&pool, false).await;
    });
    Ok(Json(json!({ "id": id, "labels": b.labels })))
}

// ---- flows --------------------------------------------------------------------

#[derive(Deserialize, Default, Clone)]
pub struct FlowQuery {
    limit: Option<usize>,
    last: Option<usize>,
    host: Option<String>,
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
            host: s(&self.host),
        }
    }
}

/// Recent flows from every online host, newest first, each tagged with its host.
async fn fleet_flows(state: &AppState, f: &FlowFilter, per_host: usize) -> Vec<VmFlowRecord> {
    let req = Request::VmFlows {
        limit: Some(per_host),
        vm: None,
        verdict: None,
    };
    let mut out = Vec::new();
    for (h, res) in bpf::fan_out(&state.pool, &req).await {
        let Ok(v) = res else { continue };
        let recs: Vec<VmFlowRecord> = serde_json::from_value(v).unwrap_or_default();
        for mut r in recs {
            r.host = Some(h.hostname.clone());
            if f.matches(&r) {
                out.push(r);
            }
        }
    }
    out.sort_by(|a, b| b.ts.cmp(&a.ts));
    out
}

#[derive(Deserialize, Default)]
pub struct EdgeQuery {
    vm: Option<String>,
    host: Option<String>,
    limit: Option<usize>,
}

/// Flow history edges from every online host, each tagged with its host.
pub async fn fleet_edges(state: &AppState, vm: Option<String>) -> Vec<VmFlowEdge> {
    let req = Request::VmFlowEdges { vm };
    let mut out = Vec::new();
    for (h, res) in bpf::fan_out(&state.pool, &req).await {
        let Ok(v) = res else { continue };
        let edges: Vec<VmFlowEdge> = serde_json::from_value(v).unwrap_or_default();
        out.extend(edges.into_iter().map(|mut e| {
            e.host = Some(h.hostname.clone());
            e
        }));
    }
    out.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    out
}

pub async fn flow_edges(State(state): State<AppState>, Query(q): Query<EdgeQuery>) -> Json<Value> {
    let mut items = fleet_edges(&state, q.vm.filter(|v| !v.is_empty())).await;
    if let Some(h) = q.host.filter(|h| !h.is_empty()) {
        items.retain(|e| e.host.as_deref() == Some(h.as_str()));
    }
    Json(json!({ "items": items }))
}

pub async fn flow_edges_reset(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    let res = bpf::fan_out(&state.pool, &Request::VmFlowEdgesReset {}).await;
    let ok = res.iter().filter(|(_, r)| r.is_ok()).count();
    Ok(Json(json!({ "ok": true, "hosts": ok })))
}

pub async fn fleet_alerts(state: &AppState, limit: usize) -> Vec<VmFlowAlert> {
    let req = Request::VmFlowAlerts { limit: Some(limit) };
    let mut out = Vec::new();
    for (h, res) in bpf::fan_out(&state.pool, &req).await {
        let Ok(v) = res else { continue };
        let alerts: Vec<VmFlowAlert> = serde_json::from_value(v).unwrap_or_default();
        out.extend(alerts.into_iter().map(|mut a| {
            a.host = Some(h.hostname.clone());
            a
        }));
    }
    out.sort_by(|a, b| b.ts.cmp(&a.ts));
    out
}

pub async fn flow_alerts(State(state): State<AppState>, Query(q): Query<EdgeQuery>) -> Json<Value> {
    let limit = q.limit.unwrap_or(200).min(1000);
    let mut items = fleet_alerts(&state, limit).await;
    items.truncate(limit);
    Json(json!({ "items": items }))
}

#[derive(Deserialize, Default)]
pub struct HostQuery {
    host: Option<String>,
}

/// Online hosts matching `host` (id or hostname); all of them by default,
/// so a quarantine follows the VM wherever it runs or migrates.
async fn target_hosts(state: &AppState, host: Option<String>) -> Vec<bpf::HostRef> {
    let hosts = bpf::online_hosts(&state.pool).await;
    match host.filter(|h| !h.is_empty()) {
        Some(h) => hosts
            .into_iter()
            .filter(|x| x.id == h || x.hostname == h)
            .collect(),
        None => hosts,
    }
}

/// `req` on the target hosts; per-host results, error unless one succeeded.
async fn on_hosts(
    state: &AppState,
    host: Option<String>,
    req: &Request,
) -> Result<(Vec<Value>, Vec<Value>), ApiError> {
    let hosts = target_hosts(state, host).await;
    if hosts.is_empty() {
        return Err(ApiError::not_found("no online host matches"));
    }
    let results = futures_util::future::join_all(hosts.iter().map(|h| bpf::call(h, req))).await;
    let (mut ok, mut errors) = (Vec::new(), Vec::new());
    for (h, r) in hosts.iter().zip(results) {
        match r {
            Ok(mut v) => {
                if let Some(o) = v.as_object_mut() {
                    o.insert("host_id".into(), json!(h.id));
                    o.insert("hostname".into(), json!(h.hostname));
                }
                ok.push(v);
            }
            Err(e) => errors.push(json!({ "hostname": h.hostname, "error": format!("{e:#}") })),
        }
    }
    if ok.is_empty() {
        let first = errors
            .first()
            .and_then(|e| e["error"].as_str())
            .unwrap_or("failed")
            .to_string();
        return Err(ApiError::bad_request(first));
    }
    Ok((ok, errors))
}

/// Quarantine `vm` on the target hosts (also the `vm.quarantine` action).
pub(crate) async fn quarantine_vm(
    state: &AppState,
    vm: &str,
    host: Option<String>,
    b: VmQuarantineBody,
    by: &str,
) -> Result<Value, ApiError> {
    let reason = b.reason.clone();
    let req = b.into_request(vm.to_string(), by.to_string());
    let (hosts, errors) = on_hosts(state, host, &req).await?;
    let secs = match &req {
        Request::VmQuarantine { secs, .. } => *secs,
        _ => 0,
    };
    state.emit_event(
        "netpol.quarantine",
        format!(
            "{by} quarantined {vm} for {secs}s{}",
            if reason.is_empty() {
                String::new()
            } else {
                format!(": {reason}")
            }
        ),
    );
    Ok(json!({ "vm": vm, "hosts": hosts, "errors": errors }))
}

pub async fn quarantine(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(name): Path<String>,
    Query(q): Query<HostQuery>,
    Json(b): Json<VmQuarantineBody>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    Ok(Json(
        quarantine_vm(&state, &name, q.host, b, &actor.username).await?,
    ))
}

pub async fn quarantine_release(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(name): Path<String>,
    Query(q): Query<HostQuery>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&actor)?;
    let req = Request::VmQuarantineRelease { vm: name.clone() };
    let (hosts, errors) = on_hosts(&state, q.host, &req).await?;
    let released = hosts.iter().any(|h| h["released"].as_bool() == Some(true));
    if released {
        state.emit_event(
            "netpol.quarantine",
            format!("{} released the quarantine of {name}", actor.username),
        );
    }
    Ok(Json(
        json!({ "vm": name, "released": released, "hosts": hosts, "errors": errors }),
    ))
}

pub async fn quarantines(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "items": bpf::fan_out_items(&state.pool, &Request::VmQuarantines).await }))
}

/// Address → DNS names from every host's `toFQDNs` cache.
async fn fqdn_names(state: &AppState) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in bpf::fan_out_items(&state.pool, &Request::VmFqdnCache).await {
        let (Some(addr), Some(name)) = (v["address"].as_str(), v["name"].as_str()) else {
            continue;
        };
        let names = m.entry(addr.to_string()).or_default();
        if !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    m
}

pub async fn learn(
    State(state): State<AppState>,
    Json(mut o): Json<LearnOptions>,
) -> Result<Json<Value>, ApiError> {
    let edges = fleet_edges(&state, None).await;
    let fleet = Fleet::load(&state.pool).await;
    o.fqdn.extend(fqdn_names(&state).await);
    let r = tokio::task::spawn_blocking(move || netpol::learn(&edges, &fleet.vms, &o))
        .await
        .map_err(|e| ApiError::internal(format!("learn: {e}")))?;
    Ok(Json(serde_json::to_value(r).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct ReplayBody {
    yaml: String,
    #[serde(default)]
    limit: Option<usize>,
}

pub async fn replay(
    State(state): State<AppState>,
    Json(b): Json<ReplayBody>,
) -> Result<Response, ApiError> {
    let (parsed, v) = netpol::parse_documents(&b.yaml);
    if !v.ok() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid policy", "errors": v.errors, "warnings": v.warnings })),
        )
            .into_response());
    }
    let fleet = Fleet::load(&state.pool).await;
    let mut draft = fleet.policies.clone();
    draft.retain(|p| !parsed.iter().any(|n| n.name == p.name));
    draft.extend(parsed);
    let edges = fleet_edges(&state, None).await;
    let fqdn = fqdn_names(&state).await;
    let limit = b.limit.unwrap_or(5000).min(20_000);
    let r = tokio::task::spawn_blocking(move || {
        let hosts = fleet.all_host_addresses();
        netpol::replay(
            &edges,
            &fleet.policies,
            &draft,
            &ReplayInputs {
                vms: &fleet.vms,
                services: &fleet.services,
                host_addresses: &hosts,
                remote_node_addresses: &[],
                fqdn: &fqdn,
                limit,
            },
        )
    })
    .await
    .map_err(|e| ApiError::internal(format!("replay: {e}")))?;
    Ok(Json(serde_json::to_value(r).unwrap_or_default()).into_response())
}

/// `toFQDNs` bindings learned on every online host.
pub async fn fqdn_cache(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "items": bpf::fan_out_items(&state.pool, &Request::VmFqdnCache).await }))
}

/// Mutual-authentication table of every online host.
pub async fn auth_table(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "items": bpf::fan_out_items(&state.pool, &Request::VmAuthTable).await }))
}

pub async fn flows(State(state): State<AppState>, Query(q): Query<FlowQuery>) -> Json<Value> {
    let limit = q.limit.unwrap_or(200).min(5000);
    let mut items = fleet_flows(&state, &q.filter(), 5000).await;
    items.truncate(limit);
    Json(json!({ "items": items }))
}

/// Live fleet flows: polls each host's bpfd once a second and emits records
/// newer than the last one seen per host.
pub async fn flow_stream(
    State(state): State<AppState>,
    Query(q): Query<FlowQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    struct Poll {
        state: AppState,
        f: FlowFilter,
        last: usize,
        seen: HashMap<String, String>,
        first: bool,
        queue: std::collections::VecDeque<VmFlowRecord>,
    }
    let init = Poll {
        state,
        f: q.filter(),
        last: q.last.unwrap_or(0).min(5000),
        seen: HashMap::new(),
        first: true,
        queue: Default::default(),
    };
    let stream = futures_util::stream::unfold(init, |mut p| async move {
        loop {
            if let Some(r) = p.queue.pop_front() {
                let ev = Event::default()
                    .event("flow")
                    .data(serde_json::to_string(&r).unwrap_or_default());
                return Some((Ok(ev), p));
            }
            if !p.first {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let mut batch = fleet_flows(&p.state, &p.f, 500).await;
            batch.reverse();
            if p.first {
                for r in &batch {
                    let h = r.host.clone().unwrap_or_default();
                    if p.seen.get(&h).is_none_or(|t| r.ts > *t) {
                        p.seen.insert(h, r.ts.clone());
                    }
                }
                let skip = batch.len().saturating_sub(p.last);
                p.queue.extend(batch.into_iter().skip(skip));
                p.first = false;
            } else {
                for r in batch {
                    let h = r.host.clone().unwrap_or_default();
                    if p.seen.get(&h).is_some_and(|t| r.ts <= *t) {
                        continue;
                    }
                    p.seen.insert(h, r.ts.clone());
                    p.queue.push_back(r);
                }
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

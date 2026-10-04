// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

//! Fleet VM network policies: compile per host and push the result to each
//! host's machina-bpfd VM edge (`owner = controller`, which makes the
//! host daemon's local policies inactive). Re-pushes when the compiled
//! state changes (policies, labels, addresses, placement) and periodically.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use machina_bpf::api::{Request, VmAuthIdentity, VmEgressSnat};
use machina_bpf::authca;
use machina_bpf::netpol::tenant::{self, ProjectNet};
use machina_bpf::netpol::{
    self, Inputs, NetpolService, NetpolVm, ServiceEndpoint, VmNetworkPolicy,
};
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;
use uuid::Uuid;

use super::bpf::{self, HostRef, LOCAL_HOST_ID};
use crate::state::AppState;

pub const OWNER: &str = "controller";
const TICK_SECS: u64 = 30;
/// Push unchanged state again every this many ticks (bpfd restarts, drift).
const FORCE_EVERY: u32 = 10;
/// Check URL threat feeds for a refetch hourly.
const THREAT_REFRESH_EVERY: u32 = 120;

static LAST_PUSH: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);
/// Every host has an empty egress SNAT set and nothing asks for one.
static EGRESS_IDLE: AtomicBool = AtomicBool::new(false);

pub async fn policies(
    pool: &SqlitePool,
) -> anyhow::Result<Vec<(VmNetworkPolicy, bool, i64, String)>> {
    let rows: Vec<(String, bool, i64, String)> = sqlx::query_as(
        "SELECT policy_json, enabled, generation, updated_at FROM vm_network_policies ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(j, en, g, u)| serde_json::from_str(&j).ok().map(|p| (p, en, g, u)))
        .collect())
}

pub async fn enabled_policies(pool: &SqlitePool) -> Vec<VmNetworkPolicy> {
    let now = chrono::Utc::now();
    policies(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.1 && !netpol::jit::expired(&p.0, now))
        .map(|p| p.0)
        .collect()
}

/// Delete policies past `machina.io/expires-at`; returns their names.
pub async fn reap_expired(pool: &SqlitePool) -> Vec<String> {
    let now = chrono::Utc::now();
    let mut gone = Vec::new();
    for (p, ..) in policies(pool).await.unwrap_or_default() {
        if netpol::jit::expired(&p, now) && delete(pool, &p.name).await.unwrap_or(false) {
            gone.push(p.name);
        }
    }
    gone
}

pub async fn upsert(pool: &SqlitePool, p: &VmNetworkPolicy, actor: &str) -> anyhow::Result<bool> {
    let json = serde_json::to_string(p)?;
    let existed: Option<String> =
        sqlx::query_scalar("SELECT name FROM vm_network_policies WHERE name = ?")
            .bind(&p.name)
            .fetch_optional(pool)
            .await?;
    sqlx::query(
        "INSERT INTO vm_network_policies (name, kind, policy_json, created_by) VALUES (?, ?, ?, ?)
         ON CONFLICT(name) DO UPDATE SET kind = excluded.kind, policy_json = excluded.policy_json,
           generation = generation + 1, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&p.name)
    .bind(&p.kind)
    .bind(json)
    .bind(actor)
    .execute(pool)
    .await?;
    Ok(existed.is_none())
}

pub async fn delete(pool: &SqlitePool, name: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM vm_network_policies WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn set_enabled(pool: &SqlitePool, name: &str, enabled: bool) -> anyhow::Result<bool> {
    let r = sqlx::query(
        "UPDATE vm_network_policies SET enabled = ?, generation = generation + 1, updated_at = CURRENT_TIMESTAMP WHERE name = ?",
    )
    .bind(enabled)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// libvirt VMs of the fleet (KubeVirt VMs are pod endpoints, not taps).
pub async fn inventory(pool: &SqlitePool) -> Vec<NetpolVm> {
    type Row = (
        String,
        Option<Uuid>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT name, host_id, project, labels, tags, guest_ip FROM vms
             WHERE COALESCE(inventory_source, 'libvirt') != 'kubevirt' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|(name, host, project, labels, tags, ip)| {
            let mut l: BTreeMap<String, String> = labels
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default();
            if l.is_empty() {
                let tags: Vec<String> = tags
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                l = tags
                    .iter()
                    .filter_map(|t| t.split_once('='))
                    .filter(|(k, _)| !k.is_empty())
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
            }
            NetpolVm {
                name,
                host: host.map(|h| h.to_string()),
                project: project.filter(|p| !p.is_empty()),
                labels: l,
                addresses: ip.into_iter().filter(|a| !a.is_empty()).collect(),
            }
        })
        .collect()
}

/// host id → management address.
pub async fn host_addresses(pool: &SqlitePool) -> BTreeMap<String, String> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, address FROM hosts")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|(id, a)| {
            (
                id.to_string(),
                a.split(':').next().unwrap_or("").to_string(),
            )
        })
        .filter(|(_, a)| a.parse::<std::net::IpAddr>().is_ok())
        .collect()
}

/// Fleet Cloud load balancers as `toServices` targets: name = LB name,
/// namespace = project, endpoints = listener on the owning host plus the
/// enabled members.
pub async fn services(pool: &SqlitePool) -> Vec<NetpolService> {
    type Row = (
        Uuid,
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
        Option<i64>,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT lb.id, lb.name, COALESCE(p.name, ''), lb.protocol, lb.listener_port, h.address,
                v.guest_ip, m.port
           FROM load_balancers lb
           JOIN hosts h ON h.id = lb.host_id
           LEFT JOIN projects p ON p.id = lb.project_id
           LEFT JOIN lb_members m ON m.load_balancer_id = lb.id AND m.enabled = 1
           LEFT JOIN vms v ON v.id = m.vm_id
          ORDER BY lb.id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut out: BTreeMap<Uuid, NetpolService> = BTreeMap::new();
    for (id, name, project, protocol, listener, host, member, member_port) in rows {
        let proto = if protocol.eq_ignore_ascii_case("udp") {
            17
        } else {
            6
        };
        let ep = |address: &str, port: i64| ServiceEndpoint {
            address: address.to_string(),
            port: u16::try_from(port).unwrap_or(0),
            proto,
        };
        let s = out.entry(id).or_insert_with(|| NetpolService {
            name,
            namespace: project,
            labels: BTreeMap::new(),
            endpoints: vec![ep(host.split(':').next().unwrap_or(""), listener)],
        });
        if let (Some(ip), Some(port)) = (member.filter(|a| !a.is_empty()), member_port) {
            s.endpoints.push(ep(&ip, port));
        }
    }
    out.into_values().collect()
}

/// Project network settings, the default (`*`) first.
pub async fn project_settings(pool: &SqlitePool) -> Vec<ProjectNet> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT settings, updated_by, updated_at FROM vm_netpol_projects
         ORDER BY project != '*', project",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .filter_map(|(j, by, at)| {
            let mut s: ProjectNet = serde_json::from_str(&j).ok()?;
            s.updated_by = by;
            s.updated_at = at;
            Some(s)
        })
        .collect()
}

pub async fn project_put(pool: &SqlitePool, s: &ProjectNet, actor: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO vm_netpol_projects (project, settings, updated_by) VALUES (?, ?, ?)
         ON CONFLICT(project) DO UPDATE SET settings = excluded.settings,
           updated_by = excluded.updated_by, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&s.project)
    .bind(serde_json::to_string(s)?)
    .bind(actor)
    .execute(pool)
    .await?;
    EGRESS_IDLE.store(false, Ordering::Relaxed);
    Ok(())
}

pub async fn project_delete(pool: &SqlitePool, project: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM vm_netpol_projects WHERE project = ?")
        .bind(project)
        .execute(pool)
        .await?;
    EGRESS_IDLE.store(false, Ordering::Relaxed);
    Ok(r.rows_affected() > 0)
}

/// Fleet Cloud projects plus every project a VM names.
pub async fn project_names(pool: &SqlitePool, vms: &[NetpolVm]) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = sqlx::query_scalar("SELECT name FROM projects")
        .fetch_all(pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    out.extend(vms.iter().filter_map(|v| v.project.clone()));
    out
}

/// Everything needed to compile for any host.
pub struct Fleet {
    /// Stored policies plus the generated project policies.
    pub policies: Vec<VmNetworkPolicy>,
    pub vms: Vec<NetpolVm>,
    pub services: Vec<NetpolService>,
    pub host_addrs: BTreeMap<String, String>,
    pub projects: Vec<ProjectNet>,
}

impl Fleet {
    pub async fn load(pool: &SqlitePool) -> Self {
        let vms = inventory(pool).await;
        let projects = project_settings(pool).await;
        let mut policies = enabled_policies(pool).await;
        let generated = tenant::policies(&projects, &project_names(pool, &vms).await);
        for g in generated {
            if !policies.iter().any(|p| p.name == g.name) {
                policies.push(g);
            }
        }
        Fleet {
            policies,
            vms,
            services: services(pool).await,
            host_addrs: host_addresses(pool).await,
            projects,
        }
    }

    /// Compile for one host (`None` / the local pseudo host = every VM).
    pub fn compile(&self, host_id: Option<&str>) -> netpol::Compiled {
        let host = host_id.filter(|h| *h != LOCAL_HOST_ID);
        let own: Vec<String> = host
            .and_then(|h| self.host_addrs.get(h))
            .cloned()
            .into_iter()
            .collect();
        let remote: Vec<String> = self
            .host_addrs
            .iter()
            .filter(|(id, _)| Some(id.as_str()) != host)
            .map(|(_, a)| a.clone())
            .collect();
        let mut c = netpol::compile(&Inputs {
            policies: &self.policies,
            vms: &self.vms,
            services: &self.services,
            host,
            host_addresses: &own,
            remote_node_addresses: if host.is_some() { &remote } else { &[] },
        });
        if host.is_some() {
            c.state.host_addrs = self.host_addrs.clone();
        }
        c
    }

    pub fn all_host_addresses(&self) -> Vec<String> {
        self.host_addrs.values().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostSync {
    pub host_id: String,
    pub hostname: String,
    pub ok: bool,
    pub pushed: bool,
    pub error: Option<String>,
    pub vms: usize,
    pub rules: usize,
    pub peers: usize,
    pub warnings: Vec<String>,
}

fn hash_value(v: &Value) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.to_string().hash(&mut h);
    h.finish()
}

async fn record(pool: &SqlitePool, s: &HostSync) {
    let _ = sqlx::query(
        "INSERT INTO vm_netpol_host_status (host_id, hostname, synced_at, ok, error, vms, rules, peers, warnings)
         VALUES (?, ?, CURRENT_TIMESTAMP, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(host_id) DO UPDATE SET hostname = excluded.hostname, synced_at = excluded.synced_at,
           ok = excluded.ok, error = excluded.error, vms = excluded.vms, rules = excluded.rules,
           peers = excluded.peers, warnings = excluded.warnings",
    )
    .bind(&s.host_id)
    .bind(&s.hostname)
    .bind(s.ok)
    .bind(&s.error)
    .bind(s.vms as i64)
    .bind(s.rules as i64)
    .bind(s.peers as i64)
    .bind(serde_json::to_string(&s.warnings).unwrap_or_else(|_| "[]".into()))
    .execute(pool)
    .await;
}

async fn previously_synced(pool: &SqlitePool, host_id: &str) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vm_netpol_host_status WHERE host_id = ?")
        .bind(host_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
        > 0
}

static CA: Mutex<Option<std::sync::Arc<authca::Ca>>> = Mutex::new(None);

/// The VM network policy CA (`MACHINA_NETPOL_CA_DIR`, default
/// `/var/lib/machina/netpol-ca`).
fn netpol_ca() -> anyhow::Result<std::sync::Arc<authca::Ca>> {
    let mut g = CA.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ca) = g.as_ref() {
        return Ok(ca.clone());
    }
    let dir = std::env::var("MACHINA_NETPOL_CA_DIR")
        .unwrap_or_else(|_| "/var/lib/machina/netpol-ca".into());
    let ca = std::sync::Arc::new(authca::Ca::load_or_create(std::path::Path::new(&dir))?);
    *g = Some(ca.clone());
    Ok(ca)
}

/// Give the host's bpfd a certificate for bpfd-to-bpfd authentication,
/// re-issued past half its lifetime. The key never leaves the host.
async fn ensure_auth_cert(h: &HostRef) -> anyhow::Result<bool> {
    let id: VmAuthIdentity = serde_json::from_value(bpf::call(h, &Request::VmAuthIdentity).await?)?;
    let fresh = id.cert.as_ref().is_some_and(|c| {
        c.host_id == h.id && c.not_after - authca::unix_now() > authca::HOST_CERT_SECS / 2
    });
    if fresh {
        return Ok(false);
    }
    let ca = netpol_ca()?;
    let (cert_pem, not_after) = ca.sign_host(&id.csr, &h.id)?;
    bpf::call(
        h,
        &Request::VmAuthCert {
            host_id: h.id.clone(),
            ca_pem: ca.cert_pem.clone(),
            cert_pem,
            not_after,
        },
    )
    .await?;
    Ok(true)
}

async fn sync_host(pool: &SqlitePool, fleet: &Fleet, h: &HostRef, force: bool) -> HostSync {
    let c = fleet.compile(Some(&h.id));
    let mut state = c.state;
    let empty = fleet.policies.is_empty();
    let mut out = HostSync {
        host_id: h.id.clone(),
        hostname: h.hostname.clone(),
        ok: true,
        pushed: false,
        error: None,
        vms: state.vms.len(),
        rules: state.policy.len(),
        peers: state.peers.len(),
        warnings: c.warnings,
    };
    if empty && !previously_synced(pool, &h.id).await {
        return out;
    }
    if netpol::uses_authentication(&fleet.policies) {
        match ensure_auth_cert(h).await {
            Ok(true) => {
                tracing::info!(host = %h.hostname, "issued VM network policy host certificate")
            }
            Ok(false) => {}
            Err(e) => out
                .warnings
                .push(format!("mutual authentication certificate: {e:#}")),
        }
    }
    if empty {
        state.owner = String::new();
        state.flow_log = false;
    } else {
        state.owner = OWNER.into();
        state.node_is_host = true;
    }
    let v = serde_json::to_value(&state).unwrap_or_default();
    let digest = hash_value(&v);
    let unchanged = LAST_PUSH
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashMap::new)
        .get(&h.id)
        == Some(&digest);
    if unchanged && !force {
        return out;
    }
    match bpf::call(h, &Request::VmEdgeSync { state }).await {
        Ok(_) => {
            out.pushed = true;
            LAST_PUSH
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(HashMap::new)
                .insert(h.id.clone(), digest);
        }
        Err(e) => {
            out.ok = false;
            out.error = Some(format!("{e:#}"));
        }
    }
    if empty && out.ok {
        let _ = sqlx::query("DELETE FROM vm_netpol_host_status WHERE host_id = ?")
            .bind(&h.id)
            .execute(pool)
            .await;
    } else {
        record(pool, &out).await;
    }
    out
}

/// Compile and push to every online host.
pub async fn reconcile(pool: &SqlitePool, force: bool) -> Vec<HostSync> {
    let fleet = Fleet::load(pool).await;
    let hosts = bpf::online_hosts(pool).await;
    let mut out = Vec::new();
    for h in &hosts {
        out.push(sync_host(pool, &fleet, h, force).await);
    }
    reconcile_egress(&fleet, &hosts).await;
    out
}

/// Push each host its project egress SNAT rules. Once every host holds an
/// empty set and no project has an egress IP, nothing is sent until a
/// project setting changes.
async fn reconcile_egress(fleet: &Fleet, hosts: &[HostRef]) {
    let wanted = fleet.projects.iter().any(|p| !p.egress_ips.is_empty());
    if !wanted && EGRESS_IDLE.load(Ordering::Relaxed) {
        return;
    }
    let mut all_ok = true;
    for h in hosts {
        let rules = tenant::snat_rules(&fleet.projects, &fleet.vms, &h.id, &h.hostname);
        let none = rules.is_empty();
        let req = Request::VmEgressSnatSet {
            config: VmEgressSnat {
                rules,
                exclude: None,
            },
        };
        match bpf::call(h, &req).await {
            Ok(_) => {}
            // A bpfd without egress support has nothing to clear.
            Err(e) if none && format!("{e:#}").contains("unknown variant") => {}
            Err(e) => {
                all_ok = false;
                tracing::warn!(host = %h.hostname, "egress SNAT push: {e:#}");
            }
        }
    }
    if !wanted && all_ok {
        EGRESS_IDLE.store(true, Ordering::Relaxed);
    }
}

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECS));
        let mut n: u32 = 0;
        let mut alerts_seen: Option<String> = None;
        loop {
            interval.tick().await;
            if !state.leader.is_leader() {
                continue;
            }
            n = n.wrapping_add(1);
            for name in reap_expired(&state.pool).await {
                tracing::info!(policy = %name, "temporary VM network policy expired");
                state.emit_event(
                    "netpol.jit",
                    format!("temporary policy {name} expired and was removed"),
                );
            }
            for r in reconcile(&state.pool, n.is_multiple_of(FORCE_EVERY)).await {
                if let Some(e) = r.error {
                    tracing::warn!(host = %r.hostname, "vm network policy sync: {e}");
                }
            }
            if n.is_multiple_of(THREAT_REFRESH_EVERY) {
                for f in refresh_due_threat_feeds(&state.pool).await {
                    for (h, r) in bpf::fan_out(&state.pool, &f.request()).await {
                        if let Err(e) = r {
                            tracing::warn!(host = %h.hostname, feed = %f.name, "threat feed push: {e:#}");
                        }
                    }
                    state.emit_event(
                        "netpol.threat",
                        format!(
                            "threat feed {} refreshed: {} domains",
                            f.name, f.domain_count
                        ),
                    );
                }
            }
            reconcile_threat(&state.pool).await;
            forward_alerts(&state, &mut alerts_seen).await;
        }
    });
}

/// Detection alerts newer than `seen` become events (and so webhooks / SIEM).
/// The first pass only records where the hosts are.
async fn forward_alerts(state: &AppState, seen: &mut Option<String>) {
    let alerts = crate::api::vm_network_policies::fleet_alerts(state, 200).await;
    let newest = alerts.first().map(|a| a.ts.clone());
    if let Some(last) = seen.as_ref() {
        for a in alerts.iter().rev().filter(|a| a.ts > *last) {
            state.emit_event(
                "netpol.alert",
                format!(
                    "[{}] {} on {}: {}",
                    a.severity,
                    a.kind,
                    a.host.as_deref().unwrap_or("?"),
                    a.detail
                ),
            );
            propose_quarantine(state, a).await;
        }
    }
    if newest.is_some() || seen.is_none() {
        *seen = Some(newest.unwrap_or_default());
    }
}

/// A high-severity scan from a VM becomes a pending `vm.quarantine` action
/// (one per VM at a time); nothing happens until someone approves it.
async fn propose_quarantine(state: &AppState, a: &machina_bpf::api::VmFlowAlert) {
    let Some(vm) = a.src_vm.as_deref() else {
        return;
    };
    if a.severity != "high"
        || !matches!(
            a.kind.as_str(),
            "port_scan" | "host_sweep" | "threat_domain"
        )
    {
        return;
    }
    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_actions WHERE status = 'pending' AND action_type = 'vm.quarantine' AND json_extract(object_ref, '$.vm') = ?",
    )
    .bind(vm)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(1);
    if pending > 0 {
        return;
    }
    let host = a.host.clone().unwrap_or_default();
    let body = crate::engine::ai::actions::CreateActionBody {
        action_type: "vm.quarantine".into(),
        label: format!("Quarantine {vm} for 1 hour"),
        review: format!(
            "{} from {vm} on {host}: {}. Cuts every flow of the VM, including open ones, except SSH from its host; lifts itself after an hour.",
            a.kind, a.detail
        ),
        risk: "Disconnects the VM".into(),
        object_ref: serde_json::json!({
            "vm": vm,
            "host": a.host,
            "secs": 3600,
            "allow_host_ssh": true,
            "reason": format!("{}: {}", a.kind, a.detail),
        }),
        source: "netpol".into(),
    };
    if let Err(e) =
        crate::engine::ai::actions::create_action(&state.pool, &body, "netpol-detector").await
    {
        tracing::warn!("quarantine proposal for {vm}: {e:#}");
    }
}

// ---- DNS threat feeds -------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ThreatFeedRow {
    pub name: String,
    pub source: String,
    pub block: bool,
    #[serde(skip)]
    pub domains: Vec<String>,
    pub domain_count: usize,
    pub updated_by: String,
    pub updated_at: String,
}

impl ThreatFeedRow {
    pub fn request(&self) -> Request {
        Request::VmThreatFeedSet {
            name: self.name.clone(),
            source: self.source.clone(),
            block: self.block,
            domains: self.domains.clone(),
        }
    }
}

pub async fn threat_feeds(pool: &SqlitePool) -> Vec<ThreatFeedRow> {
    let rows: Vec<(String, String, bool, String, String, String)> = sqlx::query_as(
        "SELECT name, source, block, domains, updated_by, updated_at FROM vm_netpol_threat_feeds ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|(name, source, block, d, updated_by, updated_at)| {
            let domains: Vec<String> = serde_json::from_str(&d).unwrap_or_default();
            ThreatFeedRow {
                name,
                source,
                block,
                domain_count: domains.len(),
                domains,
                updated_by,
                updated_at,
            }
        })
        .collect()
}

pub async fn threat_feed_put(
    pool: &SqlitePool,
    name: &str,
    source: &str,
    block: bool,
    domains: &[String],
    by: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO vm_netpol_threat_feeds (name, source, block, domains, updated_by, updated_at)
         VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(name) DO UPDATE SET source = excluded.source, block = excluded.block,
           domains = excluded.domains, updated_by = excluded.updated_by, updated_at = excluded.updated_at",
    )
    .bind(name)
    .bind(source)
    .bind(block)
    .bind(serde_json::to_string(domains)?)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn threat_feed_delete(pool: &SqlitePool, name: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM vm_netpol_threat_feeds WHERE name = ?")
        .bind(name)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn fetch_feed(url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut resp = client.get(url).send().await?.error_for_status()?;
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > netpol::threat::MAX_FEED_BYTES {
            anyhow::bail!(
                "feed {url} is larger than {} MiB",
                netpol::threat::MAX_FEED_BYTES >> 20
            );
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// What a host must change to match `want`: feeds to (re)send, names to drop.
fn threat_diff<'a>(
    want: &'a [ThreatFeedRow],
    have: &[machina_bpf::api::VmThreatFeed],
) -> (Vec<&'a ThreatFeedRow>, Vec<String>) {
    let send = want
        .iter()
        .filter(|w| {
            !have.iter().any(|h| {
                h.name == w.name
                    && h.source == w.source
                    && h.block == w.block
                    && h.domains == w.domain_count
            })
        })
        .collect();
    let drop = have
        .iter()
        .filter(|h| !want.iter().any(|w| w.name == h.name))
        .map(|h| h.name.clone())
        .collect();
    (send, drop)
}

/// With any fleet feeds, every online host carries exactly those.
pub async fn reconcile_threat(pool: &SqlitePool) {
    let want = threat_feeds(pool).await;
    if want.is_empty() {
        return;
    }
    for h in bpf::online_hosts(pool).await {
        let have: machina_bpf::api::VmThreatStatus =
            match bpf::call(&h, &Request::VmThreatFeeds).await {
                Ok(v) => serde_json::from_value(v).unwrap_or_default(),
                Err(e) => {
                    tracing::debug!(host = %h.hostname, "threat feeds: {e:#}");
                    continue;
                }
            };
        let (send, drop) = threat_diff(&want, &have.feeds);
        for f in send {
            if let Err(e) = bpf::call(&h, &f.request()).await {
                tracing::warn!(host = %h.hostname, feed = %f.name, "threat feed push: {e:#}");
            }
        }
        for name in drop {
            if let Err(e) = bpf::call(&h, &Request::VmThreatFeedRemove { name: name.clone() }).await
            {
                tracing::warn!(host = %h.hostname, feed = %name, "threat feed remove: {e:#}");
            }
        }
    }
}

/// Refetch URL feeds older than [`netpol::threat::REFRESH_SECS`]; returns
/// the refreshed names (pushed to the hosts by the caller).
pub async fn refresh_due_threat_feeds(pool: &SqlitePool) -> Vec<ThreatFeedRow> {
    let cutoff = (chrono::Utc::now()
        - chrono::Duration::seconds(netpol::threat::REFRESH_SECS as i64))
    .format("%Y-%m-%d %H:%M:%S")
    .to_string();
    let mut out = Vec::new();
    for f in threat_feeds(pool).await {
        if f.source.is_empty() || f.updated_at > cutoff {
            continue;
        }
        match refresh_threat_feed(pool, &f, "threat-feed-refresh").await {
            Ok(r) => out.push(r),
            Err(e) => tracing::warn!(feed = %f.name, "threat feed refresh: {e:#}"),
        }
    }
    out
}

pub async fn refresh_threat_feed(
    pool: &SqlitePool,
    f: &ThreatFeedRow,
    by: &str,
) -> anyhow::Result<ThreatFeedRow> {
    let body = fetch_feed(&f.source).await?;
    let b = netpol::threat::FeedBody {
        url: Some(f.source.clone()),
        block: f.block,
        ..Default::default()
    };
    let domains = b.domains(Some(&body)).map_err(|e| anyhow::anyhow!(e))?;
    threat_feed_put(pool, &f.name, &f.source, f.block, &domains, by).await?;
    Ok(ThreatFeedRow {
        domain_count: domains.len(),
        domains,
        updated_by: by.to_string(),
        ..f.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::test_support::{seed_host, test_state};

    #[tokio::test]
    async fn load_balancers_are_services() {
        let (state, _rx) = test_state().await;
        let pool = &state.pool;
        let host = seed_host(pool, Uuid::from_u128(1)).await;
        sqlx::query("UPDATE hosts SET address = '192.0.2.10:50051' WHERE id = ?")
            .bind(host)
            .execute(pool)
            .await
            .unwrap();
        let (vm, lb) = (Uuid::from_u128(2), Uuid::from_u128(3));
        sqlx::query(
            "INSERT INTO vms (id, name, host_id, guest_ip) VALUES (?, 'web-1', ?, '10.0.0.5')",
        )
        .bind(vm)
        .bind(host)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO load_balancers (id, name, protocol, host_id, listener_port) VALUES (?, 'web-lb', 'tcp', ?, 8080)",
        )
        .bind(lb)
        .bind(host)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO lb_members (id, load_balancer_id, vm_id, port) VALUES (?, ?, ?, 80)",
        )
        .bind(Uuid::from_u128(4))
        .bind(lb)
        .bind(vm)
        .execute(pool)
        .await
        .unwrap();
        let s = services(pool).await;
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "web-lb");
        let eps: Vec<(&str, u16, u8)> = s[0]
            .endpoints
            .iter()
            .map(|e| (e.address.as_str(), e.port, e.proto))
            .collect();
        assert_eq!(eps, [("192.0.2.10", 8080, 6), ("10.0.0.5", 80, 6)]);
    }

    #[tokio::test]
    async fn scans_propose_one_quarantine() {
        let (state, _rx) = test_state().await;
        let alert = |kind: &str, severity: &str| machina_bpf::api::VmFlowAlert {
            kind: kind.into(),
            severity: severity.into(),
            src: "10.0.0.9".into(),
            src_vm: Some("np-bad".into()),
            host: Some("hv1".into()),
            detail: "probed 25 ports".into(),
            ..Default::default()
        };
        propose_quarantine(&state, &alert("new_peer", "low")).await;
        propose_quarantine(&state, &alert("port_scan", "high")).await;
        propose_quarantine(&state, &alert("host_sweep", "high")).await;
        let pending = crate::engine::ai::actions::list_pending(&state.pool)
            .await
            .unwrap();
        let q: Vec<_> = pending
            .iter()
            .filter(|a| a.action_type == "vm.quarantine")
            .collect();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].object_ref["vm"], "np-bad");
        assert_eq!(q[0].object_ref["allow_host_ssh"], true);
        let body: machina_bpf::api::VmQuarantineBody =
            serde_json::from_value(q[0].object_ref.clone()).unwrap();
        assert_eq!(body.secs, Some(3600));
    }

    #[tokio::test]
    async fn jit_needs_a_second_admin_and_expires() {
        use crate::api::vm_network_policies::JIT_ACTION;
        use crate::engine::ai::actions;
        let (state, _rx) = test_state().await;
        let pool = &state.pool;
        for (i, name) in ["np-a", "np-b"].iter().enumerate() {
            sqlx::query("INSERT INTO vms (id, name) VALUES (?, ?)")
                .bind(Uuid::from_u128(10 + i as u128))
                .bind(name)
                .execute(pool)
                .await
                .unwrap();
        }
        let req = netpol::jit::JitRequest {
            from: "np-a".into(),
            to: "np-b".into(),
            port: 22,
            secs: Some(60),
            ..Default::default()
        };
        let body = actions::CreateActionBody {
            action_type: JIT_ACTION.into(),
            label: "jit".into(),
            review: String::new(),
            risk: String::new(),
            object_ref: serde_json::to_value(&req).unwrap(),
            source: "netpol".into(),
        };
        let a = actions::create_action(pool, &body, "alice").await.unwrap();
        let user = |name: &str, role: &str| crate::auth::AuthUser {
            username: name.into(),
            role: role.into(),
            auth_source: None,
        };
        assert!(
            actions::approve_and_execute(&state, a.id, &user("alice", "admin"))
                .await
                .is_err()
        );
        assert!(
            actions::approve_and_execute(&state, a.id, &user("carol", "operator"))
                .await
                .is_err()
        );
        assert_eq!(
            actions::get_action(pool, a.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            "pending",
            "refused approvals leave the request pending"
        );
        actions::approve_and_execute(&state, a.id, &user("bob", "admin"))
            .await
            .unwrap();
        let grants = netpol::jit::grants(&enabled_policies(pool).await, chrono::Utc::now());
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].granted_by, "bob");

        let mut p = enabled_policies(pool).await.remove(0);
        p.annotations.insert(
            netpol::jit::ANNOTATION_EXPIRES.into(),
            "2000-01-01T00:00:00Z".into(),
        );
        upsert(pool, &p, "bob").await.unwrap();
        assert!(
            enabled_policies(pool).await.is_empty(),
            "expired is not compiled"
        );
        assert_eq!(reap_expired(pool).await, [p.name]);
        assert!(policies(pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn threat_feeds_are_mirrored_by_name_and_shape() {
        let (state, _rx) = test_state().await;
        let pool = &state.pool;
        let d = vec!["a.example".to_string(), "b.example".to_string()];
        threat_feed_put(pool, "urlhaus", "", true, &d, "alice")
            .await
            .unwrap();
        let want = threat_feeds(pool).await;
        assert_eq!((want.len(), want[0].domain_count), (1, 2));
        let have = |block, domains| machina_bpf::api::VmThreatFeed {
            name: "urlhaus".into(),
            block,
            domains,
            ..Default::default()
        };
        let local = machina_bpf::api::VmThreatFeed {
            name: "local".into(),
            ..Default::default()
        };
        let (send, drop) = threat_diff(&want, &[have(true, 2), local]);
        assert!(send.is_empty());
        assert_eq!(drop, ["local"]);
        let (send, _) = threat_diff(&want, &[have(false, 2)]);
        assert_eq!(send.len(), 1, "block changed");
        let (send, _) = threat_diff(&want, &[have(true, 3)]);
        assert_eq!(send.len(), 1, "list changed");
        assert!(threat_feed_delete(pool, "urlhaus").await.unwrap());
        assert!(threat_feeds(pool).await.is_empty());
    }
}

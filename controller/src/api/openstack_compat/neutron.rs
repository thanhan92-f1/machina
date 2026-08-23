// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

//! Neutron v2.0-compatible networking endpoints.
//!
//! Networks are a thin wrapper over the EXISTING `api::networks` handlers. Ports bound
//! to a VM (`device_id` set) delegate to the existing `vms::attach_vm_nic`/
//! `detach_vm_nic` — actual libvirt NIC plumbing happens there, never in this module.
//!
//! Two things are intentionally simplified for this first cut (flagged in the
//! implementation plan as open design questions, not silently decided):
//! - **Routers** are logical-only records. Machina's networks are flat L2/bridge-based,
//!   so creating a router and attaching subnet interfaces succeeds at the API level but
//!   does not program any inter-subnet routing in the dataplane.
//! - **Floating IPs** are logical 1:1 records (no NAT rule is installed yet). Wiring
//!   this to a real DNAT mechanism — most naturally by extending
//!   `api::vms::port_forwards` — is follow-up work.
//! - **Security groups** are stored and CRUD-able, but enforcement (translating rules
//!   into a `FirewallPlanRequest` via `engine::zeus_firewall`, the same mechanism
//!   `firewall_profile` already uses) is not wired up in this first cut.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::api::networks::{self, CreateNetworkBody, NetworkRow};
use crate::api::vms;
use crate::api::ApiError;
use crate::auth::AuthUser;
use crate::state::AppState;

fn network_json(row: &NetworkRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.id.to_string(),
        "name": row.name,
        "status": "ACTIVE",
        "admin_state_up": true,
        "shared": false,
        "subnets": [],
    })
}

// ---------------------------------------------------------------------------
// Networks — thin wrapper over api::networks
// ---------------------------------------------------------------------------

pub async fn list_networks(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = networks::list_networks(State(state), Extension(actor)).await?.0;
    Ok(Json(serde_json::json!({ "networks": rows.iter().map(network_json).collect::<Vec<_>>() })))
}

pub async fn get_network(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = networks::get_network(State(state), Extension(actor), Path(id)).await?.0;
    Ok(Json(serde_json::json!({ "network": network_json(&row) })))
}

#[derive(Debug, Deserialize)]
pub struct NetworkCreateRequest {
    pub network: NetworkCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct NetworkCreateBody {
    pub name: String,
}

pub async fn create_network(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<NetworkCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let body = CreateNetworkBody {
        name: req.network.name,
        backend: "linux-bridge".into(),
        vlan_id: None,
        bridge: None,
        host_id: None,
        segment_id: None,
        firewall_profile: None,
    };
    let row = networks::create_network(State(state), Extension(actor), Json(body)).await?.0;
    Ok(Json(serde_json::json!({ "network": network_json(&row) })))
}

// ---------------------------------------------------------------------------
// Subnets
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct SubnetRow {
    id: Uuid,
    network_id: Uuid,
    name: String,
    cidr: String,
    gateway_ip: Option<String>,
    dns_nameservers_json: String,
    allocation_pools_json: String,
    enable_dhcp: bool,
}

impl SubnetRow {
    fn to_neutron(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "network_id": self.network_id.to_string(),
            "name": self.name,
            "cidr": self.cidr,
            "gateway_ip": self.gateway_ip,
            "dns_nameservers": serde_json::from_str::<serde_json::Value>(&self.dns_nameservers_json).unwrap_or(serde_json::json!([])),
            "allocation_pools": serde_json::from_str::<serde_json::Value>(&self.allocation_pools_json).unwrap_or(serde_json::json!([])),
            "enable_dhcp": self.enable_dhcp,
            "ip_version": 4,
        })
    }
}

const SUBNET_SELECT: &str = "SELECT id, network_id, name, cidr, gateway_ip, dns_nameservers_json, \
    allocation_pools_json, enable_dhcp FROM subnets";

pub async fn list_subnets(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, SubnetRow>(&format!("{SUBNET_SELECT} ORDER BY name"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "subnets": rows.iter().map(SubnetRow::to_neutron).collect::<Vec<_>>() })))
}

pub async fn get_subnet(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, SubnetRow>(&format!("{SUBNET_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "subnet": row.to_neutron() })))
}

#[derive(Debug, Deserialize)]
pub struct SubnetCreateRequest {
    pub subnet: SubnetCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct SubnetCreateBody {
    pub network_id: Uuid,
    #[serde(default)]
    pub name: String,
    pub cidr: String,
    #[serde(default)]
    pub gateway_ip: Option<String>,
    #[serde(default = "default_true")]
    pub enable_dhcp: bool,
}

fn default_true() -> bool {
    true
}

pub async fn create_subnet(
    State(state): State<AppState>,
    Json(req): Json<SubnetCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO subnets (id, network_id, name, cidr, gateway_ip, enable_dhcp) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(req.subnet.network_id)
    .bind(&req.subnet.name)
    .bind(&req.subnet.cidr)
    .bind(&req.subnet.gateway_ip)
    .bind(req.subnet.enable_dhcp)
    .execute(&state.pool)
    .await?;
    let row = sqlx::query_as::<_, SubnetRow>(&format!("{SUBNET_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "subnet": row.to_neutron() })))
}

pub async fn delete_subnet(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM subnets WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------------------------------------------------------------------------
// Ports — device_id set to a VM delegates to the existing NIC attach/detach handlers
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct PortRow {
    id: Uuid,
    network_id: Uuid,
    subnet_id: Option<Uuid>,
    vm_id: Option<Uuid>,
    mac_address: String,
    fixed_ip: Option<String>,
    status: String,
}

impl PortRow {
    fn to_neutron(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "network_id": self.network_id.to_string(),
            "device_id": self.vm_id.map(|v| v.to_string()).unwrap_or_default(),
            "mac_address": self.mac_address,
            "status": self.status,
            "fixed_ips": self.subnet_id.map(|sid| serde_json::json!([{
                "subnet_id": sid.to_string(),
                "ip_address": self.fixed_ip,
            }])).unwrap_or(serde_json::json!([])),
        })
    }
}

const PORT_SELECT: &str =
    "SELECT id, network_id, subnet_id, vm_id, mac_address, fixed_ip, status FROM ports";

pub async fn list_ports(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, PortRow>(&format!("{PORT_SELECT} ORDER BY created_at"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ports": rows.iter().map(PortRow::to_neutron).collect::<Vec<_>>() })))
}

pub async fn get_port(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, PortRow>(&format!("{PORT_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "port": row.to_neutron() })))
}

#[derive(Debug, Deserialize)]
pub struct PortCreateRequest {
    pub port: PortCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct PortCreateBody {
    pub network_id: Uuid,
    #[serde(default)]
    pub subnet_id: Option<Uuid>,
    #[serde(default)]
    pub device_id: Option<Uuid>,
}

pub async fn create_port(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Json(req): Json<PortCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let network_name: String = sqlx::query_scalar("SELECT name FROM networks WHERE id = ?")
        .bind(req.port.network_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("network not found"))?;

    let id = Uuid::new_v4();
    let mut status = "DOWN";
    if let Some(vm_id) = req.port.device_id {
        // Actual libvirt NIC plumbing happens here, via the same handler the native
        // Machina API uses — this module never touches domain XML directly.
        let _ = vms::attach_vm_nic(
            State(state.clone()),
            Extension(actor),
            Path(vm_id),
            Json(vms::AttachNicBody { network: network_name, model: "virtio".into() }),
        )
        .await?;
        status = "ACTIVE";
    }

    sqlx::query(
        "INSERT INTO ports (id, network_id, subnet_id, vm_id, status) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(req.port.network_id)
    .bind(req.port.subnet_id)
    .bind(req.port.device_id)
    .bind(status)
    .execute(&state.pool)
    .await?;
    let row = sqlx::query_as::<_, PortRow>(&format!("{PORT_SELECT} WHERE id = ?"))
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "port": row.to_neutron() })))
}

pub async fn delete_port(
    State(state): State<AppState>,
    Extension(actor): Extension<AuthUser>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let row: Option<(Option<Uuid>, String)> =
        sqlx::query_as("SELECT vm_id, mac_address FROM ports WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.pool)
            .await?;
    if let Some((Some(vm_id), mac)) = row {
        if !mac.is_empty() {
            let _ = vms::detach_vm_nic(State(state.clone()), Extension(actor), Path((vm_id, mac))).await;
        }
    }
    sqlx::query("DELETE FROM ports WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------------------------------------------------------------------------
// Routers — logical-only records, see module doc comment
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct RouterRow {
    id: Uuid,
    name: String,
    external_network_id: Option<Uuid>,
}

impl RouterRow {
    fn to_neutron(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "name": self.name,
            "status": "ACTIVE",
            "admin_state_up": true,
            "external_gateway_info": self.external_network_id.map(|n| serde_json::json!({ "network_id": n.to_string() })),
        })
    }
}

pub async fn list_routers(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, RouterRow>(
        "SELECT id, name, external_network_id FROM routers ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({ "routers": rows.iter().map(RouterRow::to_neutron).collect::<Vec<_>>() })))
}

#[derive(Debug, Deserialize)]
pub struct RouterCreateRequest {
    pub router: RouterCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct RouterCreateBody {
    pub name: String,
}

pub async fn create_router(
    State(state): State<AppState>,
    Json(req): Json<RouterCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO routers (id, name) VALUES (?, ?)")
        .bind(id)
        .bind(&req.router.name)
        .execute(&state.pool)
        .await?;
    let row = RouterRow { id, name: req.router.name, external_network_id: None };
    Ok(Json(serde_json::json!({ "router": row.to_neutron() })))
}

pub async fn delete_router(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM routers WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
pub struct AddRouterInterfaceRequest {
    pub subnet_id: Uuid,
}

pub async fn add_router_interface(
    State(state): State<AppState>,
    Path(router_id): Path<Uuid>,
    Json(req): Json<AddRouterInterfaceRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    sqlx::query("INSERT OR IGNORE INTO router_interfaces (router_id, subnet_id) VALUES (?, ?)")
        .bind(router_id)
        .bind(req.subnet_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({
        "id": router_id.to_string(),
        "subnet_id": req.subnet_id.to_string(),
    })))
}

// ---------------------------------------------------------------------------
// Floating IPs — logical 1:1 records, see module doc comment
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct FloatingIpRow {
    id: Uuid,
    floating_network_id: Uuid,
    floating_ip_address: String,
    port_id: Option<Uuid>,
    fixed_ip_address: Option<String>,
}

impl FloatingIpRow {
    fn to_neutron(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id.to_string(),
            "floating_network_id": self.floating_network_id.to_string(),
            "floating_ip_address": self.floating_ip_address,
            "port_id": self.port_id.map(|p| p.to_string()),
            "fixed_ip_address": self.fixed_ip_address,
            "status": if self.port_id.is_some() { "ACTIVE" } else { "DOWN" },
        })
    }
}

const FIP_SELECT: &str =
    "SELECT id, floating_network_id, floating_ip_address, port_id, fixed_ip_address FROM floating_ips";

pub async fn list_floating_ips(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, FloatingIpRow>(&format!("{FIP_SELECT} ORDER BY created_at"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "floatingips": rows.iter().map(FloatingIpRow::to_neutron).collect::<Vec<_>>() })))
}

#[derive(Debug, Deserialize)]
pub struct FloatingIpCreateRequest {
    pub floatingip: FloatingIpCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct FloatingIpCreateBody {
    pub floating_network_id: Uuid,
    #[serde(default)]
    pub port_id: Option<Uuid>,
}

pub async fn create_floating_ip(
    State(state): State<AppState>,
    Json(req): Json<FloatingIpCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::new_v4();
    // No real IPAM against the external network's own CIDR yet — allocate a
    // placeholder-visible address derived from the id so the record is at least
    // unique and inspectable; a real pool-backed allocator is follow-up work
    // alongside the NAT enforcement noted in the module doc comment.
    let addr = format!("203.0.113.{}", (id.as_u128() % 254) + 1);
    let fixed_ip: Option<String> = if let Some(port_id) = req.floatingip.port_id {
        sqlx::query_scalar("SELECT fixed_ip FROM ports WHERE id = ?")
            .bind(port_id)
            .fetch_optional(&state.pool)
            .await?
            .flatten()
    } else {
        None
    };
    sqlx::query(
        "INSERT INTO floating_ips (id, floating_network_id, floating_ip_address, port_id, fixed_ip_address)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(req.floatingip.floating_network_id)
    .bind(&addr)
    .bind(req.floatingip.port_id)
    .bind(&fixed_ip)
    .execute(&state.pool)
    .await?;
    let row = FloatingIpRow {
        id,
        floating_network_id: req.floatingip.floating_network_id,
        floating_ip_address: addr,
        port_id: req.floatingip.port_id,
        fixed_ip_address: fixed_ip,
    };
    Ok(Json(serde_json::json!({ "floatingip": row.to_neutron() })))
}

pub async fn delete_floating_ip(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM floating_ips WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

// ---------------------------------------------------------------------------
// Security groups / rules — CRUD only; enforcement not wired up yet (see module doc)
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct SecurityGroupRow {
    id: Uuid,
    name: String,
    description: String,
}

pub async fn list_security_groups(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, SecurityGroupRow>(
        "SELECT id, name, description FROM security_groups ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    let groups: Vec<_> = rows
        .iter()
        .map(|r| serde_json::json!({ "id": r.id.to_string(), "name": r.name, "description": r.description }))
        .collect();
    Ok(Json(serde_json::json!({ "security_groups": groups })))
}

#[derive(Debug, Deserialize)]
pub struct SecurityGroupCreateRequest {
    pub security_group: SecurityGroupCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct SecurityGroupCreateBody {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

pub async fn create_security_group(
    State(state): State<AppState>,
    Json(req): Json<SecurityGroupCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO security_groups (id, name, description) VALUES (?, ?, ?)")
        .bind(id)
        .bind(&req.security_group.name)
        .bind(&req.security_group.description)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({
        "security_group": {
            "id": id.to_string(),
            "name": req.security_group.name,
            "description": req.security_group.description,
            "security_group_rules": [],
        }
    })))
}

pub async fn delete_security_group(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM security_groups WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[derive(Debug, Deserialize)]
pub struct SecurityGroupRuleCreateRequest {
    pub security_group_rule: SecurityGroupRuleCreateBody,
}

#[derive(Debug, Deserialize)]
pub struct SecurityGroupRuleCreateBody {
    pub security_group_id: Uuid,
    #[serde(default = "default_direction")]
    pub direction: String,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub port_range_min: Option<i32>,
    #[serde(default)]
    pub port_range_max: Option<i32>,
    #[serde(default)]
    pub remote_ip_prefix: Option<String>,
}

fn default_direction() -> String {
    "ingress".into()
}

pub async fn create_security_group_rule(
    State(state): State<AppState>,
    Json(req): Json<SecurityGroupRuleCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO security_group_rules
            (id, security_group_id, direction, protocol, port_range_min, port_range_max, remote_ip_prefix)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(req.security_group_rule.security_group_id)
    .bind(&req.security_group_rule.direction)
    .bind(&req.security_group_rule.protocol)
    .bind(req.security_group_rule.port_range_min)
    .bind(req.security_group_rule.port_range_max)
    .bind(&req.security_group_rule.remote_ip_prefix)
    .execute(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({
        "security_group_rule": {
            "id": id.to_string(),
            "security_group_id": req.security_group_rule.security_group_id.to_string(),
            "direction": req.security_group_rule.direction,
            "protocol": req.security_group_rule.protocol,
            "port_range_min": req.security_group_rule.port_range_min,
            "port_range_max": req.security_group_rule.port_range_max,
            "remote_ip_prefix": req.security_group_rule.remote_ip_prefix,
        }
    })))
}

pub async fn delete_security_group_rule(State(state): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, ApiError> {
    sqlx::query("DELETE FROM security_group_rules WHERE id = ?").bind(id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

-- Neutron v2.0-compatible routers and floating IPs (controller/src/api/openstack_compat/neutron.rs).
-- Machina's networks are flat L2/bridge-based, so a router here is a LOGICAL record
-- for API compatibility (openstack CLI / Terraform calls succeed) — it does not
-- program any dataplane routing between subnets. Floating IPs likewise start as
-- logical 1:1 records; NAT enforcement is a follow-up (extending
-- api::vms::port_forwards or a new agent RPC), flagged as an open question in the
-- implementation plan rather than decided here.
CREATE TABLE IF NOT EXISTS routers (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    external_network_id TEXT REFERENCES networks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS router_interfaces (
    router_id TEXT NOT NULL REFERENCES routers(id) ON DELETE CASCADE,
    subnet_id TEXT NOT NULL REFERENCES subnets(id) ON DELETE CASCADE,
    port_id TEXT REFERENCES ports(id) ON DELETE SET NULL,
    PRIMARY KEY (router_id, subnet_id)
);

CREATE TABLE IF NOT EXISTS floating_ips (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    floating_network_id TEXT NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
    floating_ip_address TEXT NOT NULL,
    port_id TEXT REFERENCES ports(id) ON DELETE SET NULL,
    fixed_ip_address TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_floating_ips_port ON floating_ips(port_id);

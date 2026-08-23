-- Neutron v2.0-compatible subnets and ports (controller/src/api/openstack_compat/neutron.rs).
-- `networks` (existing table) stays a flat L2 network; a subnet is a new, separate,
-- project-scoped CIDR/DHCP record 1:1 with a network the way Neutron expects. A port
-- with vm_id set corresponds to an actual libvirt NIC, plumbed via the existing
-- vms::attach_vm_nic/detach_vm_nic — this table only tracks the Neutron-shaped record.
CREATE TABLE IF NOT EXISTS subnets (
    id TEXT NOT NULL PRIMARY KEY,
    network_id TEXT NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL DEFAULT '',
    cidr TEXT NOT NULL,
    gateway_ip TEXT,
    dns_nameservers_json TEXT NOT NULL DEFAULT '[]',
    allocation_pools_json TEXT NOT NULL DEFAULT '[]',
    enable_dhcp INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_subnets_network ON subnets(network_id);

CREATE TABLE IF NOT EXISTS security_groups (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS security_group_rules (
    id TEXT NOT NULL PRIMARY KEY,
    security_group_id TEXT NOT NULL REFERENCES security_groups(id) ON DELETE CASCADE,
    direction TEXT NOT NULL DEFAULT 'ingress',
    ethertype TEXT NOT NULL DEFAULT 'IPv4',
    protocol TEXT,
    port_range_min INTEGER,
    port_range_max INTEGER,
    remote_ip_prefix TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_secgroup_rules_group ON security_group_rules(security_group_id);

CREATE TABLE IF NOT EXISTS ports (
    id TEXT NOT NULL PRIMARY KEY,
    network_id TEXT NOT NULL REFERENCES networks(id) ON DELETE CASCADE,
    subnet_id TEXT REFERENCES subnets(id) ON DELETE SET NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    vm_id TEXT REFERENCES vms(id) ON DELETE SET NULL,
    mac_address TEXT NOT NULL DEFAULT '',
    fixed_ip TEXT,
    security_group_id TEXT REFERENCES security_groups(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'DOWN',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_ports_network ON ports(network_id);
CREATE INDEX IF NOT EXISTS idx_ports_vm ON ports(vm_id);

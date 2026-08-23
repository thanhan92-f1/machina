-- Nova v2.1-compatible flavor catalog (controller/src/api/openstack_compat/nova.rs).
-- No sizing preset existed anywhere before this — Machina VMs always specify CPU/RAM
-- inline. Seeds the standard devstack-style names so Terraform configs written for
-- real OpenStack often work unmodified against Machina.
CREATE TABLE IF NOT EXISTS flavors (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    vcpus INTEGER NOT NULL,
    ram_mib INTEGER NOT NULL,
    disk_gib INTEGER NOT NULL DEFAULT 0,
    is_public INTEGER NOT NULL DEFAULT 1,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

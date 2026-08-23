-- Cinder v3-compatible volumes (controller/src/api/openstack_compat/cinder.rs). A
-- volume here is a standalone, project-owned resource created unattached — distinct
-- from `vm_disks`, which is always VM-scoped from creation. Attach/detach delegate to
-- the existing vms::attach_vm_disk/detach_vm_disk handlers.
CREATE TABLE IF NOT EXISTS volumes (
    id TEXT NOT NULL PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    size_gib INTEGER NOT NULL,
    volume_type TEXT NOT NULL DEFAULT 'default',
    storage_pool_id TEXT REFERENCES storage_pools(id) ON DELETE SET NULL,
    path TEXT,
    status TEXT NOT NULL DEFAULT 'creating',
    attached_vm_id TEXT REFERENCES vms(id) ON DELETE SET NULL,
    attached_device TEXT,
    bootable INTEGER NOT NULL DEFAULT 0,
    source_image_id TEXT REFERENCES content_images(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_volumes_project ON volumes(project_id);

CREATE TABLE IF NOT EXISTS volume_types (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    storage_class TEXT NOT NULL DEFAULT 'silver'
);

CREATE TABLE IF NOT EXISTS volume_snapshots (
    id TEXT NOT NULL PRIMARY KEY,
    volume_id TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'available',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_volume_snapshots_volume ON volume_snapshots(volume_id);

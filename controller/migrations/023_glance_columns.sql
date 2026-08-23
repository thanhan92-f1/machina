-- Glance v2-compatible fields on the existing content_images table (extended, not
-- replaced — see controller/src/api/openstack_compat/glance.rs). `glance_status` is a
-- new column rather than overloading the existing `status`, which already drives an
-- unrelated submitted/approved/rejected governance workflow in api/content.rs.
ALTER TABLE content_images ADD COLUMN disk_format TEXT NOT NULL DEFAULT 'qcow2';
ALTER TABLE content_images ADD COLUMN container_format TEXT NOT NULL DEFAULT 'bare';
ALTER TABLE content_images ADD COLUMN visibility TEXT NOT NULL DEFAULT 'private';
ALTER TABLE content_images ADD COLUMN min_disk_gib INTEGER NOT NULL DEFAULT 0;
ALTER TABLE content_images ADD COLUMN min_ram_mib INTEGER NOT NULL DEFAULT 0;
ALTER TABLE content_images ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;
ALTER TABLE content_images ADD COLUMN glance_status TEXT NOT NULL DEFAULT 'queued';

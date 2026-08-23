-- Backs Cinder volumes/snapshots with the EXISTING Atlas storage control plane
-- (engine/atlas_bridge.rs, engine/atlas_vm.rs — Ceph/RBD volumes, real
-- snapshot/backup/restore) when ATLAS_ENABLED=1, instead of only the raw
-- libvirt-pool RPC path. See controller/src/api/openstack_compat/cinder.rs.
ALTER TABLE volumes ADD COLUMN atlas_volume_id TEXT;
ALTER TABLE volume_snapshots ADD COLUMN atlas_snapshot_id TEXT;

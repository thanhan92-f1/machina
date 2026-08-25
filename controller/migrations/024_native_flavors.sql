-- Native compute flavor catalog — Phase 1 of the OpenStack-client replacement
-- ("Fleet Cloud" native compute). A flavor is a named (vcpus, memory, disk) preset
-- an operator picks at instance-create time, mirroring the Nova flavor concept
-- without any external-cloud dependency. Instance lifecycle itself (create/start/
-- stop/reboot/console/rename) already exists via the classic native VM APIs
-- (api::vms) — this table is the one genuinely missing native piece.
--
-- No FK from vms to flavors: a flavor is a creation-time preset, not a persistent
-- attribute of the VM (matches how create_vm already takes raw cpu/memory/disk
-- values — a flavor just resolves to those same values before calling it).
CREATE TABLE IF NOT EXISTS flavors (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    vcpus INTEGER NOT NULL,
    memory_mib INTEGER NOT NULL,
    disk_gib INTEGER NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    is_public INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

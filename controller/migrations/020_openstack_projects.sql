-- Foundation for an OpenStack Keystone-compatible identity API: real domain/project
-- entities and user->project role assignments. Existing project-scoped queries
-- (vms.project, project_quotas.project) are untouched and keep matching by free-text
-- name — these tables are joined in by name from Rust (see
-- db::ensure_openstack_identity_foundation), not by rewriting every existing
-- project filter across the codebase.
CREATE TABLE IF NOT EXISTS domains (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT NOT NULL PRIMARY KEY,
    domain_id TEXT NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(domain_id, name)
);

CREATE TABLE IF NOT EXISTS project_role_assignments (
    id TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, project_id, role)
);
CREATE INDEX IF NOT EXISTS idx_project_role_assignments_project ON project_role_assignments(project_id);
CREATE INDEX IF NOT EXISTS idx_project_role_assignments_user ON project_role_assignments(user_id);

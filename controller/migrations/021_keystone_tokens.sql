-- Opaque, server-side-validated tokens for the Keystone v3-compatible identity API
-- (controller/src/api/openstack_compat/keystone.rs). Real OpenStack clients treat the
-- X-Subject-Token value as opaque and never decode it, so tokens are random bytes here,
-- validated by a single indexed hash lookup — not JWTs, and no Fernet crypto needed.
CREATE TABLE IF NOT EXISTS keystone_tokens (
    token_hash TEXT NOT NULL PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    role TEXT,
    issued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_keystone_tokens_expires ON keystone_tokens(expires_at);

-- Migration 003: multi-tenant support
-- Adds organizations + per-org API keys + per-org usage tracking.

CREATE TABLE IF NOT EXISTS organizations (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    slug          TEXT NOT NULL UNIQUE,        -- URL-friendly name
    owner_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    plan          TEXT NOT NULL DEFAULT 'free', -- free | pro | enterprise
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS organization_members (
    org_id        TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT NOT NULL DEFAULT 'member', -- owner | admin | member
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (org_id, user_id)
);

-- Add org_id column to api_keys (nullable — keys not in an org are personal)
ALTER TABLE api_keys ADD COLUMN org_id TEXT REFERENCES organizations(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_api_keys_org_id ON api_keys(org_id);

-- Add org_id column to usage_logs (nullable — personal usage has org_id = NULL)
ALTER TABLE usage_logs ADD COLUMN org_id TEXT REFERENCES organizations(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_usage_logs_org_id ON usage_logs(org_id);

-- Per-org quota overrides (in addition to per-key quotas in api_key_quotas)
CREATE TABLE IF NOT EXISTS org_quotas (
    org_id        TEXT PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
    limit_tokens  INTEGER NOT NULL,
    used_tokens   INTEGER NOT NULL DEFAULT 0,
    window_days   INTEGER NOT NULL DEFAULT 30,
    reset_at      TEXT NOT NULL,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (3);

-- OmniRoute-Rust initial schema
-- Mirrors the storage contract of the Node.js OmniRoute: API keys, usage logs,
-- provider connections, dashboard sessions.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

-- ─── Users (dashboard login) ─────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS users (
  id            TEXT PRIMARY KEY,
  username      TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role          TEXT NOT NULL DEFAULT 'admin',
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ─── API Keys (for /v1/* API auth) ──────────────────────────────────────────
-- The key value is stored encrypted with API_KEY_SECRET (enc:v1: prefix).
CREATE TABLE IF NOT EXISTS api_keys (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  key_hash      TEXT NOT NULL UNIQUE,        -- SHA-256 of the raw key (for lookup)
  key_encrypted TEXT NOT NULL,               -- enc:v1:<ciphertext> (for display)
  prefix        TEXT NOT NULL,               -- first 8 chars, for UI display
  user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  last_used_at  TEXT,
  expires_at    TEXT,
  enabled       INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);

-- ─── Providers (user-configured overrides) ──────────────────────────────────
-- If a row exists for a provider, its api_key overrides the env var.
CREATE TABLE IF NOT EXISTS providers (
  id            TEXT PRIMARY KEY,            -- e.g. "openai", "anthropic"
  display_name  TEXT NOT NULL,
  api_key_enc   TEXT,                        -- enc:v1:<ciphertext> or NULL
  base_url      TEXT,                        -- override the default base URL
  enabled       INTEGER NOT NULL DEFAULT 1,
  priority      INTEGER NOT NULL DEFAULT 100,-- lower = tried first in failover
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ─── Models (per-provider model catalog) ────────────────────────────────────
CREATE TABLE IF NOT EXISTS models (
  id            TEXT PRIMARY KEY,            -- e.g. "openai:gpt-4o"
  provider_id   TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
  model_name    TEXT NOT NULL,               -- upstream model id, e.g. "gpt-4o"
  display_name  TEXT,
  context_window INTEGER,
  enabled       INTEGER NOT NULL DEFAULT 1,
  created_at    TEXT NOT NULL DEFAULT (datetime('now')),
  UNIQUE(provider_id, model_name)
);

CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider_id);

-- ─── Usage logs (one row per request) ───────────────────────────────────────
CREATE TABLE IF NOT EXISTS usage_logs (
  id              TEXT PRIMARY KEY,
  api_key_id      TEXT REFERENCES api_keys(id) ON DELETE SET NULL,
  user_id         TEXT REFERENCES users(id) ON DELETE SET NULL,
  provider_id     TEXT NOT NULL,
  model           TEXT NOT NULL,
  endpoint        TEXT NOT NULL,             -- "/v1/chat/completions"
  method          TEXT NOT NULL,             -- "POST"
  status_code     INTEGER NOT NULL,
  prompt_tokens   INTEGER NOT NULL DEFAULT 0,
  completion_tokens INTEGER NOT NULL DEFAULT 0,
  total_tokens    INTEGER NOT NULL DEFAULT 0,
  duration_ms     INTEGER NOT NULL DEFAULT 0,
  error           TEXT,
  client_ip       TEXT,
  user_agent      TEXT,
  created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_usage_logs_created ON usage_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_usage_logs_api_key ON usage_logs(api_key_id);
CREATE INDEX IF NOT EXISTS idx_usage_logs_provider ON usage_logs(provider_id);

-- ─── Settings (key-value, dashboard-configurable) ───────────────────────────
CREATE TABLE IF NOT EXISTS settings (
  key           TEXT PRIMARY KEY,
  value         TEXT NOT NULL,
  updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ─── Schema version ─────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER PRIMARY KEY,
  applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);

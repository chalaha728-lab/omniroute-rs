-- Migration 002: expand audit logging
-- Adds columns to usage_logs for IP, user-agent, request body, response body, and cost.

ALTER TABLE usage_logs ADD COLUMN request_body TEXT;
ALTER TABLE usage_logs ADD COLUMN response_preview TEXT;  -- first 500 chars of response
ALTER TABLE usage_logs ADD COLUMN cost_usd REAL DEFAULT 0;
ALTER TABLE usage_logs ADD COLUMN api_version TEXT;       -- e.g. "v1"

-- Index for fast per-IP queries (rate limit analytics, abuse detection)
CREATE INDEX IF NOT EXISTS idx_usage_logs_client_ip ON usage_logs(client_ip);
CREATE INDEX IF NOT EXISTS idx_usage_logs_user_agent ON usage_logs(user_agent);

-- Add a new table for per-key daily aggregates (faster dashboard charts)
CREATE TABLE IF NOT EXISTS usage_daily (
    date          TEXT NOT NULL,           -- YYYY-MM-DD
    api_key_id    TEXT,
    provider_id   TEXT NOT NULL,
    model         TEXT NOT NULL,
    requests      INTEGER NOT NULL DEFAULT 0,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens  INTEGER NOT NULL DEFAULT 0,
    cost_usd      REAL NOT NULL DEFAULT 0,
    errors        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (date, api_key_id, provider_id, model)
);

CREATE INDEX IF NOT EXISTS idx_usage_daily_date ON usage_daily(date);

-- Quota tracking table (alternative to settings-based storage — faster lookups)
CREATE TABLE IF NOT EXISTS api_key_quotas (
    api_key_id    TEXT PRIMARY KEY REFERENCES api_keys(id) ON DELETE CASCADE,
    limit_tokens  INTEGER NOT NULL,
    used_tokens   INTEGER NOT NULL DEFAULT 0,
    window_days   INTEGER NOT NULL DEFAULT 30,
    reset_at      TEXT NOT NULL,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (2);

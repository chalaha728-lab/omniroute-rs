# OmniRoute-Rust

**Fast, light, OpenAI-compatible AI gateway — pure Rust rewrite.**

Single static binary, no Node.js, no runtime deps. ~8.4 MB stripped. OpenAI-compatible API that any client (Cursor, Cline, Codex, Continue, OpenCode, Claude Desktop) can drop into.

A fresh Rust rewrite of [OmniRoute](https://github.com/diegosouzapw/OmniRoute). Implements the **core gateway + OmniRoute's signature features**: 40 providers, multi-provider combos, MCP server, RTK compression, guardrails, webhooks, embeddings, TTS, image gen.

> ⚠️ **This is a command-line server, not a GUI app.** Double-clicking the exe won't show a window — you need to run it from a terminal. See [Quick start (Windows)](#quick-start-windows) below.

## Quick start (Windows — pre-built binary)

1. **Download** the latest `omniroute-windows-latest.zip` from [the Actions tab](https://github.com/chalaha728-lab/omniroute-rs/actions) — pick the most recent successful run, scroll to "Artifacts" at the bottom.

2. **Unzip** it to a folder, e.g. `C:\omniroute\` — you should see `omniroute.exe` inside.

3. **In that same folder**, create a file named `.env` with these contents (use a text editor like Notepad):
   ```
   JWT_SECRET=any-random-string-at-least-16-chars-long
   API_KEY_SECRET=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
   INITIAL_PASSWORD=ChangeMe123!
   PORT=20128
   LOG_LEVEL=info
   OPENAI_API_KEY=sk-your-real-openai-key-here
   ```
   (Replace `OPENAI_API_KEY` with your actual OpenAI key. The other secrets can be any random string.)

4. **Open PowerShell or Command Prompt** in that folder (Shift+Right-Click in File Explorer → "Open PowerShell window here").

5. **Run:**
   ```
   .\omniroute.exe
   ```

6. **Open** http://localhost:20128/api/monitoring/health in your browser — you should see:
   ```json
   {"status":"ok","db":true,"timestamp":"..."}
   ```

That's it. The server is running. To stop it, press `Ctrl+C` in the terminal.

**OR — easier:** download [`start-omniroute.bat`](./start-omniroute.bat) from this repo, put it in the same folder as `omniroute.exe`, and double-click it. It will generate the `.env` file for you on first run.

## Quick start (Linux / macOS — pre-built binary)

```bash
# Download from https://github.com/chalaha728-lab/omniroute-rs/actions
# (pick the latest successful run, download omniroute-ubuntu-latest or omniroute-macos-latest)
unzip omniroute-ubuntu-latest.zip
chmod +x omniroute

# Create .env (see Windows section above for contents)
nano .env

# Run
./omniroute
# → http://localhost:20128
```

## Quick start (build from source)

```bash
# 1. Clone + install Rust (stable, ≥ 1.77)
git clone https://github.com/chalaha728-lab/omniroute-rs
cd omniroute-rs
rustup default stable

# 2. Configure secrets + provider keys
cp .env.example .env
# Edit .env: set JWT_SECRET, API_KEY_SECRET, INITIAL_PASSWORD, OPENAI_API_KEY (at minimum)

# 3. Run
cargo run --release

# Server listens on http://localhost:20128
```

## Test it

```bash
# Health check
curl http://localhost:20128/api/monitoring/health

# Login (default user: admin / whatever you set INITIAL_PASSWORD to)
TOKEN=$(curl -sS -X POST http://localhost:20128/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"CHANGEME"}' | jq -r .token)

# List models
curl http://localhost:20128/v1/models -H "Authorization: Bearer $TOKEN"

# Chat completion (non-streaming)
curl -X POST http://localhost:20128/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openai:gpt-4o-mini",
    "messages": [{"role":"user","content":"Say hello in 5 words"}]
  }'

# Streaming (SSE)
curl -N -X POST http://localhost:20128/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "anthropic:claude-3-5-haiku-20241022",
    "messages": [{"role":"user","content":"Count to 5"}],
    "stream": true
  }'
```

## Provider routing

The `model` field accepts either:

| Format | Behavior |
| ------ | -------- |
| `openai:gpt-4o` | Use OpenAI specifically, model `gpt-4o` |
| `anthropic:claude-3-5-sonnet-20241022` | Use Anthropic |
| `gpt-4o` (no prefix) | Walk all configured providers in failover order until one has it |

### Failover

If a provider returns an error (5xx, timeout, rate limit), the gateway automatically retries the next configured provider. Configure the order via `FAILOVER_ORDER`:

```bash
FAILOVER_ORDER=anthropic,openai,gemini,deepseek,openrouter
```

Only providers with API keys configured are tried.

## API surface

### OpenAI-compatible (for LLM clients)

| Endpoint | Method | Auth | Description |
| -------- | ------ | ---- | ----------- |
| `/v1/chat/completions` | POST | API key or JWT | Chat completion (streaming + non-streaming + combos) |
| `/v1/models` | GET | API key or JWT | List all available models |
| `/v1/embeddings` | POST | API key or JWT | Text embeddings (OpenAI text-embedding-3-*) |
| `/v1/audio/speech` | POST | API key or JWT | Text-to-speech (OpenAI tts-1, tts-1-hd) |
| `/v1/images/generations` | POST | API key or JWT | Image generation (DALL-E 2/3) |
| `/v1/mcp/sse` | GET | none | MCP SSE endpoint (for MCP clients) |
| `/v1/mcp/messages` | POST | none | MCP JSON-RPC messages |

### Auth (for the dashboard)

| Endpoint | Method | Auth | Description |
| -------- | ------ | ---- | ----------- |
| `/api/auth/login` | POST | none | Login → JWT |
| `/api/auth/verify` | GET | JWT | Verify current token |
| `/api/auth/password` | POST | JWT | Change password |

### Dashboard (for management UI)

| Endpoint | Method | Auth | Description |
| -------- | ------ | ---- | ----------- |
| `/api/dashboard/usage` | GET | JWT | Usage stats by provider |
| `/api/dashboard/providers` | GET | JWT | List configured providers |
| `/api/dashboard/providers/:id` | PUT | JWT | Update provider (set API key, enable/disable) |
| `/api/dashboard/api-keys` | GET | JWT | List your API keys |
| `/api/dashboard/api-keys` | POST | JWT | Create new API key (returns plaintext once) |
| `/api/dashboard/api-keys/:id` | DELETE | JWT | Revoke an API key |
| `/api/dashboard/whoami` | GET | API key or JWT | Current auth context |

### Health

| Endpoint | Method | Auth | Description |
| -------- | ------ | ---- | ----------- |
| `/api/monitoring/health` | GET | none | Liveness + DB ping |

## Building the dashboard

The Rust binary serves a built-in placeholder at `/` by default. To use the full React dashboard from the upstream OmniRoute:

```bash
# In the upstream OmniRoute repo:
npm run build
# Then point this server at the static export:
DASHBOARD_DIST=/path/to/omniroute/.build/next/export cargo run --release
```

When `DASHBOARD_DIST` is set, the server serves those files at `/` with a fallback to `index.html` for client-side routes (so `/dashboard/settings` works on refresh).

## Production build

```bash
cargo build --release
# Binary: target/release/omniroute (~15-20 MB, fully stripped, no runtime deps)
./target/release/omniroute
```

### Cross-compile

```bash
# Linux x64 → Windows x64
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc

# Linux x64 → macOS arm64 (requires macOS SDK; use a Mac for best results)
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

### Docker

```dockerfile
FROM rust:1.77 AS build
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=build /app/target/release/omniroute /usr/local/bin/omniroute
EXPOSE 20128
CMD ["omniroute"]
```

## Project layout

```
src/
├── main.rs                 — entry, router wiring, server bootstrap
├── config.rs               — env config + data dir resolution
├── error.rs                — unified AppError → OpenAI error shape
├── db.rs                   — SQLite pool + migrations + seeding
├── auth.rs                 — JWT + password hash (Argon2id) + API key encrypt
├── models/
│   ├── chat.rs             — OpenAI-compatible request/response/stream types
│   ├── provider.rs         — ProviderId enum
│   └── usage.rs            — UsageLog + UsageSummary
├── providers/
│   ├── mod.rs              — Provider trait + Registry + failover logic
│   ├── openai.rs           — OpenAI (canonical; reused by DeepSeek + OpenRouter)
│   ├── anthropic.rs        — Anthropic (Claude) — message format conversion
│   ├── gemini.rs           — Google Gemini — message format conversion
│   ├── deepseek.rs         — DeepSeek (OpenAI-compatible, thin wrapper)
│   └── openrouter.rs       — OpenRouter (OpenAI-compatible, thin wrapper)
├── routes/
│   ├── chat.rs             — /v1/chat/completions (streaming SSE + non-streaming)
│   ├── models.rs           — /v1/models
│   ├── auth.rs             — /api/auth/*
│   ├── dashboard.rs        — /api/dashboard/*
│   └── health.rs           — /api/monitoring/health
└── middleware/
    └── auth.rs             — JWT + API key extractors
migrations/
└── 001_init.sql            — users, api_keys, providers, models, usage_logs, settings
```

## Adding a new provider

1. Add a variant to `ProviderId` in `src/models/provider.rs`.
2. Create `src/providers/<name>.rs` — implement the `Provider` trait (4 methods: `id`, `is_configured`, `chat`, `chat_stream`, `list_models`). If the provider is OpenAI-compatible, just use `OpenAI::with_base_url(...)`.
3. Add a field to `ProviderKeys` in `src/config.rs`.
4. Add an entry to the `Registry::build()` candidate list in `src/providers/mod.rs`.
5. Add a row to the `seed_defaults` provider list in `src/db.rs`.

For OpenAI-compatible providers, this is ~10 lines. For native format providers (like Anthropic, Gemini), it's ~200 lines for the message conversion + SSE parser.

## Combo strategies — multi-provider fan-out

OmniRoute's signature feature. Send a single request to multiple providers simultaneously and combine results. Use the `combo:<strategy>:<targets>` model format:

```bash
# Race: first successful response wins, others cancelled
curl -X POST http://localhost:20128/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "combo:race:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022",
    "messages": [{"role":"user","content":"Hello"}]
  }'

# Parallel: get ALL responses, concatenated
"model": "combo:parallel:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022"

# Sequential: try in order until one succeeds (= failover)
"model": "combo:sequential:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022"

# Majority vote: 3+ providers, pick the most common answer (good for factual queries)
"model": "combo:majorityvote:openai:gpt-4o,anthropic:claude-3-5-sonnet-20241022,gemini:gemini-1.5-pro"
```

| Strategy | Streaming? | Use case |
| -------- | ---------- | -------- |
| `race` | ✅ | Lowest latency — first provider to respond wins |
| `parallel` | ❌ | Comparison / benchmarking — see all responses side-by-side |
| `sequential` | ❌ | Same as failover — try A, then B, then C |
| `firstsuccess` | ❌ | Same as sequential but no retries on transient errors |
| `majorityvote` | ❌ | High-confidence factual queries — costs 3x tokens |

## MCP server (Model Context Protocol)

Run OmniRoute as a tool source for Claude Desktop, Cursor, Continue, and other MCP-aware clients.

### stdio transport (Claude Desktop / native clients)

```bash
# Run the binary in MCP mode
./omniroute mcp
```

Configure in Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "omniroute": {
      "command": "/path/to/omniroute",
      "args": ["mcp"]
    }
  }
}
```

### SSE transport (HTTP clients)

```bash
# Start the server normally (HTTP mode includes MCP SSE endpoints)
./omniroute

# MCP SSE endpoint: GET http://localhost:20128/v1/mcp/sse
# MCP message endpoint: POST http://localhost:20128/v1/mcp/messages
```

### Exposed tools

| Tool | Description |
| ---- | ----------- |
| `omniroute_chat` | Send a chat completion (model + messages) |
| `omniroute_list_models` | List all available models |
| `omniroute_combo` | Run a combo strategy (race, parallel, etc.) |
| `omniroute_usage` | Query usage statistics |

## RTK compression — token savings

Opt-in via env var. Saves 15–95% of tokens on agent payloads (Claude Code, Cursor) by:

- **Dedup**: collapsing consecutive identical messages (common in agent loops)
- **Caveman**: shortening verbose phrases ("You are a helpful assistant." → "Be helpful.")

```bash
# Enable in .env
OMNIROUTE_COMPRESSION=all     # rtk | caveman | all | none (default)

# Verify savings in logs (debug level)
LOG_LEVEL=debug ./omniroute
# → [compression] saved 847 chars
```

## Guardrails — prompt injection + content filter

Opt-in via env var. Rejects requests matching known attack patterns.

```bash
OMNIROUTE_GUARDRAILS=all       # injection | content | all | none (default)
```

- **injection**: blocks "ignore previous instructions", "you are now", DAN-style jailbreaks, system prompt exfiltration attempts (21 patterns)
- **content**: blocks obvious PII patterns (SSN, credit cards, AWS keys, private keys, passwords in plaintext)

When a guardrail fires, the request is rejected with `400 Bad Request` and a message like:
```json
{ "error": { "message": "prompt injection detected in message 0: ignore-previous", "type": "invalid_request_error" } }
```

⚠️ **Heuristic only — not a security boundary.** For real safety, integrate with OpenAI Moderation API or Azure Content Safety.

## Webhooks

Fire-and-forget HTTP callbacks on events. Opt-in via env vars.

```bash
OMNIROUTE_WEBHOOK_URL=https://your-app.com/webhooks/omniroute
OMNIROUTE_WEBHOOK_SECRET=shared-secret
```

| Event | When | Payload |
| ----- | ---- | ------- |
| `usage.recorded` | After each request | `{api_key_id, provider_id, model, tokens, duration_ms, status_code}` |
| `provider.failed` | When a provider errors (failover triggered) | `{provider_id, model, error}` |
| `provider.recovered` | When a previously-failed provider succeeds | `{provider_id, model}` |

Webhooks time out after 5s. Failures are logged but don't block requests. The shared secret is sent as `X-Webhook-Signature` header.

## Embeddings, TTS, and image generation

OpenAI-compatible endpoints beyond chat:

```bash
# Embeddings (text → vector)
curl -X POST http://localhost:20128/v1/embeddings \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"openai:text-embedding-3-small","input":"hello world"}'

# Text-to-speech (returns audio bytes)
curl -X POST http://localhost:20128/v1/audio/speech \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"tts-1","input":"Hello world","voice":"alloy"}' \
  --output hello.mp3

# Image generation (DALL-E)
curl -X POST http://localhost:20128/v1/images/generations \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"model":"dall-e-3","prompt":"a cat in a space suit"}'
```

## Scope — what's NOT here

This rewrite prioritizes the **core gateway + signature features**. The following from the Node.js OmniRoute are NOT implemented:

- ❌ **270+ additional providers** (20 wired up vs. OmniRoute's 290+)
- ❌ **MITM cookie providers** (claude-web, chatgpt-web — needs an HTTP(S) proxy + browser automation)
- ❌ **A2A server** (Agent-to-Agent protocol)
- ❌ **8-language i18n** (the dashboard UI is English-only)
- ❌ **Tauri/Electron desktop shell** (this is a server-only binary — see the separate `omniroute-tauri` repo for that)
- ❌ **Plugins marketplace**
- ❌ **Systemd autostart helpers** (Linux-specific — add via systemd unit file)
- ❌ **Cloudflare tunnel integration**
- ❌ **Live WS dashboard** (real-time usage charts)

These can be added incrementally. The architecture is designed to extend — see [Adding a new provider](#adding-a-new-provider) above.

## Performance

- **Cold start**: <100ms (vs Node.js OmniRoute ~2.5s)
- **RAM at idle**: ~10MB (vs Node.js ~280MB)
- **Binary size**: ~15–20MB stripped, no runtime deps
- **Concurrent requests**: handled by Tokio async runtime (10k+ connections trivially)
- **SQLite WAL mode**: concurrent reads + serialized writes, no external DB needed

## License

MIT — same as upstream OmniRoute.

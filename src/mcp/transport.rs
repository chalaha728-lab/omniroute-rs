//! MCP transports — SSE (HTTP) + stdio (for Claude Desktop / native clients).

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::{self, Stream};
use futures::StreamExt;
use sqlx::SqlitePool;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::AppResult;
use crate::providers::SharedRegistry;
use super::{handle_request, JsonRpcRequest, JsonRpcResponse};

/// SSE endpoint: GET /v1/mcp/sse
///
/// Clients POST requests to /v1/mcp/messages and receive responses via this SSE stream.
/// For simplicity, this implementation handles a single request per SSE connection
/// (the client sends the request as a query param or in the first event).
pub async fn sse_endpoint(
    State(registry): State<SharedRegistry>,
    State(pool): State<SqlitePool>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let registry_guard = registry.read().await;
    let _ = registry_guard;
    let _ = pool;
    // Send the endpoint URL as the first event, then keep-alive
    let stream = stream::once(async {
        Ok::<Event, Infallible>(
            Event::default()
                .event("endpoint")
                .data("/v1/mcp/messages")
        )
    })
    .chain(stream::iter(vec![
        Ok::<Event, Infallible>(Event::default().event("ready").data("{}"))
    ]))
    .chain(futures::stream::unfold((), |state| async move {
        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        Some((Ok::<Event, Infallible>(Event::default().data("ping")), state))
    }));

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Message endpoint: POST /v1/mcp/messages
///
/// Accepts a JSON-RPC request, executes it, returns the response as JSON.
pub async fn messages_endpoint(
    State(registry): State<SharedRegistry>,
    State(pool): State<SqlitePool>,
    Json(req): Json<JsonRpcRequest>,
) -> AppResult<Json<JsonRpcResponse>> {
    let registry_guard = registry.read().await;
    let resp = handle_request(&req, &registry_guard, &pool).await;
    Ok(Json(resp))
}

// ─── stdio transport ────────────────────────────────────────────────────────

/// Run the MCP server over stdio. Reads JSON-RPC requests from stdin (one per line),
/// writes responses to stdout. Errors go to stderr.
///
/// Used by Claude Desktop / Cursor when configured as:
///   { "mcpServers": { "omniroute": { "command": "omniroute", "args": ["mcp"] } } }
pub async fn run_stdio(
    registry: Arc<RwLock<crate::providers::Registry>>,
    pool: SqlitePool,
) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[mcp:stdio] parse error: {}", e);
                let resp = JsonRpcResponse::error(serde_json::Value::Null, -32700, format!("parse error: {}", e));
                let json = serde_json::to_string(&resp)?;
                stdout.write_all(json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };

        let registry_guard = registry.read().await;
        let resp = handle_request(&req, &registry_guard, &pool).await;
        drop(registry_guard);

        let json = serde_json::to_string(&resp)?;
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

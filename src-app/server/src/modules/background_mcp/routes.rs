//! background_mcp routes — the JSON-RPC endpoint at /api/background/mcp plus the
//! typed ITEM-25 steering-note REST at /api/background/runs/{run_id}/notes.

use aide::axum::routing::post_with;
use aide::axum::ApiRouter;
use axum::routing::post;

use super::handlers;
use super::run_notes;

pub fn background_mcp_router() -> ApiRouter {
    ApiRouter::new()
        // Like workflow_mcp / memory_mcp: bypass aide's `api_route` because the
        // JSON-RPC handler dispatches multiple methods over the same path and
        // isn't a typed REST endpoint suitable for OpenAPI docs. Both POST (the
        // MCP call channel) and GET (streamable-HTTP session open) hit the same
        // handler — GET with no body is treated as a no-op accept.
        .route(
            "/background/mcp",
            post(handlers::jsonrpc_handler).get(handlers::jsonrpc_handler_get),
        )
        // ITEM-25 — steer a RUNNING background run. Typed REST (OpenAPI-documented),
        // owner-scoped, gated `background::use`.
        .api_route(
            "/background/runs/{run_id}/notes",
            post_with(run_notes::post_run_note, run_notes::post_run_note_docs)
                .get_with(run_notes::list_run_notes, run_notes::list_run_notes_docs),
        )
}

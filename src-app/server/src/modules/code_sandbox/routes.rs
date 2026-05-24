//! Route registration for the code_sandbox HTTP surface.
//!
//! Two route families:
//!
//! 1. **Untyped legacy routes** (plain `axum::routing::{get, post}`):
//!    - POST `/code-sandbox`               — JSON-RPC MCP loopback
//!    - GET  `/code-sandbox/file/download` — workspace artifact download
//!    These are invoked by our own clients (MCP loopback, in-browser
//!    artifact link) and aren't typed via OpenAPI.
//!
//! 2. **Typed REST routes** (`aide::axum::routing::{get_with, post_with}`):
//!    - GET  `/code-sandbox/environments`
//!    - GET  `/code-sandbox/prefetch`
//!    - POST `/code-sandbox/prefetch`
//!    - GET  `/code-sandbox/prefetch/{flavor}/events`
//!    These surface in the generated `openapi.json` so the frontend's
//!    typed API client gets matching TypeScript types for free.
//!
//! `ApiRouter` accepts both `.route()` (untyped) and `.api_route()`
//! (typed) in the same router — they coexist cleanly.

use aide::axum::routing::{delete_with, get_with};
use aide::axum::ApiRouter;
use axum::routing::{get, post};

use crate::modules::code_sandbox::handlers;

pub fn code_sandbox_router() -> ApiRouter {
    ApiRouter::new()
        // ──────── Untyped legacy ────────
        // Note (Plan-3 Phase-3 / I2): the route is registered POST-only on
        // purpose. The MCP spec § Transports lets a server offer an
        // additional standalone GET-SSE for unsolicited server→client
        // messages (progress notifications, server-initiated sampling).
        // Our built-in code_sandbox server has no out-of-band producers —
        // every elicitation / progress / streamed output rides the POST
        // response — so we don't run a GET stream. axum's `MethodRouter`
        // turns a GET against this path into `405 Method Not Allowed`,
        // which is exactly the "no standalone stream" signal the spec
        // requires. The client (`mcp/client/http.rs::spawn_standalone_get_sse`)
        // tolerates 405 silently.
        .route("/code-sandbox", post(handlers::jsonrpc_handler))
        .route(
            "/code-sandbox/file/download",
            get(handlers::download_handler),
        )
        // ──────── Typed REST (admin UI) ────────
        .api_route(
            "/code-sandbox/environments",
            get_with(
                handlers::list_environments_handler,
                handlers::list_environments_docs,
            ),
        )
        .api_route(
            "/code-sandbox/environments/{flavor}",
            delete_with(
                handlers::evict_environment_handler,
                handlers::evict_environment_docs,
            ),
        )
        .api_route(
            "/code-sandbox/prefetch",
            get_with(
                handlers::list_prefetch_tasks_handler,
                handlers::list_prefetch_tasks_docs,
            )
            .post_with(
                handlers::start_prefetch_handler,
                handlers::start_prefetch_docs,
            ),
        )
        .api_route(
            "/code-sandbox/prefetch/{flavor}/events",
            get_with(
                handlers::subscribe_prefetch_events_handler,
                handlers::subscribe_prefetch_events_docs,
            ),
        )
        // ──────── Resource limits (Plan 1 §6) ────────
        .api_route(
            "/code-sandbox/resource-limits",
            get_with(
                handlers::get_resource_limits_handler,
                handlers::get_resource_limits_docs,
            )
            .put_with(
                handlers::update_resource_limits_handler,
                handlers::update_resource_limits_docs,
            ),
        )
}

//! Route registration for the code_sandbox HTTP surface.
//!
//! Two routes:
//!   POST `/code-sandbox`               — JSON-RPC entry (MCP loopback)
//!   GET  `/code-sandbox/file/download` — workspace artifact download
//!
//! These are intentionally simple `axum::Router` routes (not `aide`
//! ApiRouter) — the loopback is invoked by our own HTTP MCP client, not
//! consumed by external clients via OpenAPI.

use aide::axum::ApiRouter;
use axum::routing::{get, post};

use crate::modules::code_sandbox::handlers;

/// Plain axum routes mounted as part of the global ApiRouter.
///
/// These are NOT on the OpenAPI surface — they're consumed only by our
/// own HTTP MCP client over the loopback URL. Skipping `aide::api_route`
/// avoids OperationHandler trait constraints on hand-written JSON-RPC
/// dispatch.
pub fn code_sandbox_router() -> ApiRouter {
    ApiRouter::new()
        .route("/code-sandbox", post(handlers::jsonrpc_handler))
        .route(
            "/code-sandbox/file/download",
            get(handlers::download_handler),
        )
}

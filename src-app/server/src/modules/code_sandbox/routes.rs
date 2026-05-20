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
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::Deserialize;

use crate::modules::code_sandbox::handlers;

/// Query string for the workspace-artifact download endpoint.
#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub filename: String,
}

/// Placeholder download handler — Phase 5 wires JWT validation +
/// workspace path scoping.
pub async fn download_handler(Query(_q): Query<DownloadQuery>) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        "code_sandbox download not yet implemented",
    )
}

/// Plain axum `Router` mounted as part of the global ApiRouter.
///
/// These routes are NOT part of the OpenAPI surface — they're consumed
/// only by our own HTTP MCP client over the loopback URL. Skipping
/// `aide::api_route` avoids the OperationHandler trait constraints on
/// hand-written JSON-RPC dispatch.
pub fn code_sandbox_router() -> ApiRouter {
    ApiRouter::new()
        .route("/code-sandbox", post(handlers::jsonrpc_handler))
        .route("/code-sandbox/file/download", get(download_handler))
}

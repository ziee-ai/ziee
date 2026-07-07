//! office_bridge routes: the JSON-RPC MCP endpoint + admin settings REST.

use aide::axum::{ApiRouter, routing::get_with};
use axum::routing::post;

use super::handlers;

pub fn office_bridge_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route`, not `api_route`
        // (multi-method, not a typed REST endpoint).
        .route("/office-bridge/mcp", post(handlers::jsonrpc_handler))
        .api_route(
            "/office-bridge/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
}

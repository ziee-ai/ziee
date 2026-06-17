//! web_search routes: the JSON-RPC MCP endpoint + admin settings REST.

use aide::axum::{
    ApiRouter,
    routing::{get_with, put_with},
};
use axum::routing::post;

use super::handlers;

pub fn web_search_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route`, not `api_route`
        // (multi-method, not a typed REST endpoint).
        .route("/web-search/mcp", post(handlers::jsonrpc_handler))
        .api_route(
            "/web-search/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
        .api_route(
            "/web-search/providers",
            get_with(handlers::get_providers, handlers::get_providers_docs),
        )
        .api_route(
            "/web-search/providers/{provider}",
            put_with(handlers::update_provider, handlers::update_provider_docs),
        )
}

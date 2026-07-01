//! lit_search routes: the JSON-RPC MCP endpoint + admin settings/connectors REST.

use aide::axum::{
    ApiRouter,
    routing::{get_with, put_with},
};
use axum::routing::post;

use super::handlers;

pub fn lit_search_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route` (multi-method).
        .route("/lit-search/mcp", post(handlers::jsonrpc_handler))
        .api_route(
            "/lit-search/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
        .api_route(
            "/lit-search/connectors",
            get_with(handlers::get_connectors, handlers::get_connectors_docs),
        )
        .api_route(
            "/lit-search/connectors/{connector}",
            put_with(handlers::update_connector, handlers::update_connector_docs),
        )
        // User-scoped: the caller's OWN connector keys (masked read + set/clear).
        .api_route(
            "/lit-search/user-keys",
            get_with(handlers::list_user_keys, handlers::list_user_keys_docs),
        )
        .api_route(
            "/lit-search/user-keys/{connector}",
            put_with(handlers::save_user_key, handlers::save_user_key_docs)
                .delete_with(handlers::delete_user_key, handlers::delete_user_key_docs),
        )
}

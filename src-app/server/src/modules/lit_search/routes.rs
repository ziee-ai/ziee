//! lit_search routes: the JSON-RPC MCP endpoint + admin settings/connectors REST.

use aide::axum::{
    ApiRouter,
    routing::{get_with, put_with},
};
use axum::routing::post;

use super::handlers;

/// The settings/connectors half — mounted regardless of the kill switch.
///
/// Split from the MCP endpoint deliberately: these routes only READ AND WRITE
/// CONFIGURATION. Nothing here egresses a query, and the admin page that drives
/// them should keep working (and keep showing the feature as off) on a
/// deployment that has switched literature search off.
pub fn lit_search_router() -> ApiRouter {
    ApiRouter::new()
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

/// The MCP JSON-RPC endpoint — mounted ONLY when the kill switch is on.
///
/// This is the surface that runs live queries against Europe PMC / Crossref /
/// PubMed / arXiv / Semantic Scholar, five of which work KEYLESS. Leaving it
/// mounted made the switch merely stop the tools being ADVERTISED: it is gated
/// on `lit_search::use`, which the Users group holds by migration, and the
/// runtime `lit_search_settings.enabled` row defaults TRUE — so with
/// `lit_search: { enabled: false }` an ordinary user could still drive live
/// scholarly queries and the terms would still egress.
///
/// Plain `route`, not `api_route` — JSON-RPC is multi-method, not typed REST.
pub fn lit_search_mcp_router() -> ApiRouter {
    ApiRouter::new().route("/lit-search/mcp", post(handlers::jsonrpc_handler))
}

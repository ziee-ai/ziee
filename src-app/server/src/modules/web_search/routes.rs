//! web_search routes: the JSON-RPC MCP endpoint + admin settings REST.

use aide::axum::{
    ApiRouter,
    routing::{get_with, put_with},
};
use axum::routing::post;

use super::handlers;

/// The settings/admin half — mounted regardless of the kill switch.
///
/// Split from the MCP endpoint deliberately: these routes only READ AND WRITE
/// CONFIGURATION. Nothing here egresses a query, and the admin page that drives
/// them should keep working (and keep showing the feature as off) on a
/// deployment that has switched web search off.
pub fn web_search_router() -> ApiRouter {
    ApiRouter::new()
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
        // User-scoped: the caller's OWN provider keys (masked read + set/clear).
        .api_route(
            "/web-search/user-keys",
            get_with(handlers::list_user_keys, handlers::list_user_keys_docs),
        )
        .api_route(
            "/web-search/user-keys/{provider}",
            put_with(handlers::save_user_key, handlers::save_user_key_docs)
                .delete_with(handlers::delete_user_key, handlers::delete_user_key_docs),
        )
}

/// The MCP JSON-RPC endpoint — mounted ONLY when the kill switch is on.
///
/// This is the surface that actually performs searches and page fetches, so it
/// is what "disabled" has to mean. Leaving it mounted made the switch merely
/// stop the tools being ADVERTISED: the endpoint is gated on `web_search::use`,
/// which the Users group holds by migration, and the runtime
/// `web_search_settings.enabled` row defaults TRUE — so with
/// `web_search: { enabled: false }` an ordinary user could still drive live
/// queries, and the query terms the switch exists to keep in-house would still
/// egress.
///
/// Plain `route`, not `api_route` — JSON-RPC is multi-method, not typed REST.
pub fn web_search_mcp_router() -> ApiRouter {
    ApiRouter::new().route("/web-search/mcp", post(handlers::jsonrpc_handler))
}

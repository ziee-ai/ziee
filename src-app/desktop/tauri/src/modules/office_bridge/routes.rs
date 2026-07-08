//! office_bridge routes: the JSON-RPC MCP endpoint + admin settings REST.

use aide::axum::{ApiRouter, routing::get_with, routing::post_with};
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
        // The open-document list the "Open Office documents" chat panel refetches
        // on each `sync:office_document` notify. Gated on `office_bridge::use`
        // (the same read perm the client store self-gates on — the no-403 rule).
        .api_route(
            "/office-bridge/documents",
            get_with(handlers::list_documents, handlers::list_documents_docs),
        )
        // Admin `[Connect]` installer flow (ITEM-13): trust the bridge cert,
        // sideload the add-in, report readiness. Gated on `office_bridge::admin::manage`.
        .api_route(
            "/office-bridge/connect",
            post_with(handlers::connect, handlers::connect_docs),
        )
}

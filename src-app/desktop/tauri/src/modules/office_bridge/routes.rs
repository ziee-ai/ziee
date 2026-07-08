//! office_bridge routes: the JSON-RPC MCP endpoint + admin settings REST.

use aide::axum::{ApiRouter, routing::get_with, routing::post_with};
use axum::routing::post;

use super::handlers;

pub fn office_bridge_router() -> ApiRouter {
    // Desktop-module routes self-prefix with `/api` (they merge at the router
    // ROOT in `run_headless`/`start_backend_server`, NOT nested under `/api` the
    // way server AppModule routes are). Mirrors `host_mount_router`. The loopback
    // MCP url (`register_office_bridge`) + the frontend client + the tests all
    // address `/api/office-bridge/*`.
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route`, not `api_route`
        // (multi-method, not a typed REST endpoint).
        .route("/api/office-bridge/mcp", post(handlers::jsonrpc_handler))
        .api_route(
            "/api/office-bridge/settings",
            get_with(handlers::get_settings, handlers::get_settings_docs)
                .put_with(handlers::update_settings, handlers::update_settings_docs),
        )
        // The open-document list the "Open Office documents" chat panel refetches
        // on each `sync:office_document` notify. Gated on `office_bridge::use`
        // (the same read perm the client store self-gates on — the no-403 rule).
        .api_route(
            "/api/office-bridge/documents",
            get_with(handlers::list_documents, handlers::list_documents_docs),
        )
        // Admin `[Connect]` installer flow (ITEM-13): trust the bridge cert,
        // sideload the add-in, report readiness. Gated on `office_bridge::admin::manage`.
        .api_route(
            "/api/office-bridge/connect",
            post_with(handlers::connect, handlers::connect_docs),
        )
}

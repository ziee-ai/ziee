//! citations routes: the JSON-RPC MCP endpoint + the typed REST surface for the
//! library / import / verify / export / styles + project reference-list links.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with, post_with},
};
use axum::routing::post;

use super::{handlers, rest};

pub fn citations_router() -> ApiRouter {
    ApiRouter::new()
        // JSON-RPC dispatch over a single path — plain `route`, not `api_route`
        // (multi-method, not a typed REST endpoint).
        .route("/citations/mcp", post(handlers::jsonrpc_handler))
        // Library REST.
        .api_route(
            "/citations",
            get_with(rest::list_citations, rest::list_citations_docs),
        )
        .api_route(
            "/citations/import",
            post_with(rest::import_citations, rest::import_citations_docs),
        )
        .api_route(
            "/citations/verify",
            post_with(rest::verify_citations, rest::verify_citations_docs),
        )
        .api_route(
            "/citations/reverify",
            post_with(rest::reverify_citations, rest::reverify_citations_docs),
        )
        .api_route(
            "/citations/export",
            get_with(rest::export_citations, rest::export_citations_docs),
        )
        .api_route(
            "/citations/styles",
            get_with(rest::list_styles, rest::list_styles_docs),
        )
        .api_route(
            "/citations/{id}",
            delete_with(rest::delete_citation, rest::delete_citation_docs),
        )
        // Project reference-list membership.
        .api_route(
            "/projects/{project_id}/citations",
            post_with(rest::attach_to_project, rest::attach_to_project_docs),
        )
        .api_route(
            "/projects/{project_id}/citations/{entry_id}",
            delete_with(rest::detach_from_project, rest::detach_from_project_docs),
        )
}

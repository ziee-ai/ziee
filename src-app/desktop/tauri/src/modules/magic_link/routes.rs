//! Routes for magic-link issue + exchange.
//!
//! Mounted under `/auth` (so the full paths are
//! `/api/auth/magic-link/issue` and `/api/auth/magic-link/exchange`).
//!
//! `issue` is wrapped with the localhost-Host middleware from the
//! remote_access module (defense in depth, same as the rest of the
//! remote-access surface). `exchange` is intentionally NOT gated —
//! that's the endpoint phones call from the public tunnel.

use aide::axum::{ApiRouter, routing::post_with};
use axum::middleware;

use crate::modules::remote_access::middleware::require_localhost_host;

use super::handlers::{exchange, exchange_docs, issue, issue_docs};

pub fn magic_link_routes() -> ApiRouter {
    // Two sub-routers so we can apply the localhost middleware only
    // to `issue`, leaving `exchange` reachable from the tunnel.
    let issue_router = ApiRouter::new()
        .api_route("/api/auth/magic-link/issue", post_with(issue, issue_docs))
        .layer(middleware::from_fn(require_localhost_host));

    let exchange_router =
        ApiRouter::new().api_route("/api/auth/magic-link/exchange", post_with(exchange, exchange_docs));

    issue_router.merge(exchange_router)
}

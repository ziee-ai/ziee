//! Backend Routes
//!
//! Route definitions for backend management

use aide::axum::ApiRouter;
use super::handlers;
use ziee::get_with;

/// Create backend API routes with OpenAPI documentation
pub fn backend_api_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/api/desktop/backend/status",
            get_with(handlers::get_backend_status, handlers::get_backend_status_docs),
        )
}

use aide::axum::{routing::get_with, ApiRouter};
use aide::axum::routing::post_with;

use super::handlers::*;

// =====================================================
// Router Setup
// =====================================================

pub fn app_routes() -> ApiRouter {
    ApiRouter::new()
        .api_route("/setup/status", get_with(get_setup_status, get_setup_status_docs))
        .api_route("/setup/admin", post_with(setup_admin, setup_admin_docs))
}

use aide::axum::{routing::get_with, ApiRouter};
use aide::axum::routing::post_with;
use axum::Json;
use sqlx::PgPool;

use crate::modules::auth::AuthResponse;

use super::handlers::{get_setup_status, setup_admin};
use super::types::SetupStatusResponse;

// =====================================================
// Router Setup
// =====================================================

pub fn app_routes() -> ApiRouter<PgPool> {
    ApiRouter::new()
        .api_route(
            "/setup/status",
            get_with(get_setup_status, |op| {
                op.description("Check if initial admin setup is required")
                    .id("App.getSetupStatus")
                    .tag("app")
                    .response::<200, Json<SetupStatusResponse>>()
            }),
        )
        .api_route(
            "/setup/admin",
            post_with(setup_admin, |op| {
                op.description("Create the first administrator account")
                    .id("App.setupAdmin")
                    .tag("app")
                    .response::<201, Json<AuthResponse>>()
                    .response::<403, ()>()
                    .response::<400, ()>()
            }),
        )
}

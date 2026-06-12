use aide::transform::TransformOperation;
use axum::{Json, debug_handler, http::StatusCode};

use crate::modules::permissions::{RequirePermissions, with_permission};

use super::checker;
use super::permissions::ServerUpdateRead;
use super::types::UpdateStatusResponse;

/// GET /api/server-update/status
/// Cached server update-availability status (admin-only).
#[debug_handler]
pub async fn get_update_status(
    _auth: RequirePermissions<(ServerUpdateRead,)>,
) -> (StatusCode, Json<UpdateStatusResponse>) {
    (StatusCode::OK, Json(checker::cached_status()))
}

pub fn get_update_status_docs(op: TransformOperation) -> TransformOperation {
    // `with_permission` injects the 403 example — REQUIRED for the UI
    // `Permissions` enum to gain `ServerUpdateRead` (the enum is scraped from
    // these examples, not the Rust registry).
    with_permission::<(ServerUpdateRead,)>(op)
        .summary("Get server update-availability status")
        .description("Cached server update-availability status (admin).")
        .id("ServerUpdate.getStatus")
        .tag("Server Update")
        .response::<200, Json<UpdateStatusResponse>>()
        .response::<401, ()>()
}

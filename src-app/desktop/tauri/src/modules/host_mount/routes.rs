//! Route registration for the host_mount module.

use aide::axum::{routing::get_with, ApiRouter};

use super::handlers::*;

pub fn host_mount_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/api/host-mounts/policy",
            get_with(get_policy, get_policy_docs).put_with(update_policy, update_policy_docs),
        )
        .api_route(
            "/api/host-mounts/conversation/{conversation_id}",
            get_with(get_conversation_mounts, get_conversation_mounts_docs)
                .put_with(put_conversation_mounts, put_conversation_mounts_docs),
        )
        .api_route(
            "/api/host-mounts/project/{project_id}",
            get_with(get_project_mounts, get_project_mounts_docs)
                .put_with(put_project_mounts, put_project_mounts_docs),
        )
}

//! Memory module HTTP routes.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with},
};

use super::handlers::*;

pub fn memory_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/memories",
            get_with(list_memories, list_memories_docs)
                .post_with(create_memory, create_memory_docs),
        )
        .api_route(
            "/memories/all",
            delete_with(delete_all_memories, delete_all_memories_docs),
        )
        .api_route(
            "/memories/{id}",
            get_with(get_memory, get_memory_docs)
                .patch_with(update_memory, update_memory_docs)
                .delete_with(delete_memory, delete_memory_docs),
        )
        .api_route(
            "/memory/settings",
            get_with(get_user_settings, get_user_settings_docs)
                .put_with(update_user_settings, update_user_settings_docs),
        )
        .api_route(
            "/memory/audit-log",
            get_with(list_audit_log, list_audit_log_docs),
        )
        .api_route(
            "/admin/memory-settings",
            get_with(get_admin_settings, get_admin_settings_docs)
                .put_with(update_admin_settings, update_admin_settings_docs),
        )
}

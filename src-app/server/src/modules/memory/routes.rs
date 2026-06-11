//! Memory module HTTP routes.

use aide::axum::{
    ApiRouter,
    routing::{delete_with, get_with},
};

use super::handlers::*;

pub fn memory_router() -> ApiRouter {
    #[allow(unused_mut)]
    let mut router = ApiRouter::new()
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
            "/memory/admin-settings",
            get_with(get_admin_settings, get_admin_settings_docs)
                .put_with(update_admin_settings, update_admin_settings_docs),
        )
        .api_route(
            "/memory/admin-settings/rebuild-status",
            get_with(get_rebuild_status, get_rebuild_status_docs),
        )
        .api_route(
            "/memory/admin-settings/reembed",
            aide::axum::routing::post_with(trigger_reembed, trigger_reembed_docs),
        )
        .api_route(
            "/memory/admin/fts/rebuild",
            aide::axum::routing::post_with(trigger_fts_rebuild, trigger_fts_rebuild_docs),
        )
        .api_route(
            "/memory/admin/fts/rebuild/status",
            get_with(get_fts_rebuild_status, get_fts_rebuild_status_docs),
        );

    // Test-only synchronous hooks for the extraction + summarizer
    // pipelines. Compiled into debug builds only — `cargo test` and
    // dev `cargo run` see them; `cargo build --release` strips them
    // out so they can't reach production binaries.
    #[cfg(debug_assertions)]
    {
        use aide::axum::routing::post_with;
        router = router
            .api_route(
                "/_test/memory/extract",
                post_with(super::handlers::test_extract, super::handlers::test_extract_docs),
            )
            .api_route(
                "/_test/memory/summarize",
                post_with(super::handlers::test_summarize, super::handlers::test_summarize_docs),
            );
    }
    router
}

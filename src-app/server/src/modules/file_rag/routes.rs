//! Document-RAG admin HTTP routes.

use aide::axum::{
    ApiRouter,
    routing::{get_with, post_with},
};

use super::handlers::*;

pub fn file_rag_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/file-rag/admin-settings",
            get_with(get_admin_settings, get_admin_settings_docs)
                .put_with(update_admin_settings, update_admin_settings_docs),
        )
        .api_route(
            "/file-rag/admin-settings/reembed",
            post_with(reembed, reembed_docs),
        )
        .api_route("/file-rag/backfill", post_with(backfill, backfill_docs))
}

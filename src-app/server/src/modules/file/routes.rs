// File routes — the ziee consumer of the mountable SDK bundle.
//
// Chunk `ziee-file-http` moved the store-generic file routes into
// `ziee_file::http::file_routes::<R>()`. This builder mounts that bundle with
// ziee's `ZieeIdentityResolver` and MERGES the routes that stayed ziee-side
// (upload / export / download-with-token / the version-append POST / the
// conversation deliverables). The GET `/files/{file_id}/versions` (in the SDK
// bundle) and the POST here merge on the same path. Route paths + operationIds
// are byte-identical to the pre-move single router.

use aide::axum::routing::{get_with, post_with};
use aide::axum::ApiRouter;
use axum::extract::DefaultBodyLimit;

use super::deliverables::{
    list_deliverables, list_deliverables_docs, pin_deliverable, pin_deliverable_docs,
    unpin_deliverable, unpin_deliverable_docs,
};
use super::handlers::{
    append_version, append_version_docs, download_with_token, download_with_token_docs,
    export_file, export_file_docs, upload_file, upload_file_docs,
};
use crate::core::app_state::file_upload_body_limit_bytes;
use crate::modules::permissions::extractors::ZieeIdentityResolver;

/// File management routes
pub fn file_router() -> ApiRouter {
    // The store-generic subset (list / get / preview / raw / thumbnail / text /
    // text-rects / delete / download / download-token / version reads / restore),
    // mounted with ziee's resolver.
    ziee_file::http::file_routes::<ZieeIdentityResolver>()
        // ── ziee-RETAINED routes (processing / domain coupled) ──
        // Upload — explicit higher body limit than the app-wide default (see
        // main.rs). Derived from the configurable per-file cap (`cap + slack`,
        // set at boot in `app_state`) so the request is rejected before
        // buffering an over-cap body into RAM; the slack covers multipart
        // framing + extra fields, keeping the raw body limit above the handler
        // cap so an over-cap file gets a clear FILE_TOO_LARGE (400), not a 413.
        .api_route(
            "/files/upload",
            post_with(upload_file, upload_file_docs)
                .layer(DefaultBodyLimit::max(file_upload_body_limit_bytes())),
        )
        // User-facing export (pandoc) — stays ziee-side (processing).
        .api_route("/files/{file_id}/export", get_with(export_file, export_file_docs))
        // Unauthenticated token download — re-verifies identity by user-id
        // (identity-recheck), stays ziee-side.
        .api_route(
            "/files/{file_id}/download-with-token",
            get_with(download_with_token, download_with_token_docs),
        )
        // Version APPEND (the co-edit write) — runs commit_new_version
        // (ProcessingManager), stays ziee-side. Merges with the SDK bundle's GET
        // on the same path.
        .api_route(
            "/files/{file_id}/versions",
            post_with(append_version, append_version_docs),
        )
        // Conversation deliverables (derived model-authored files ∪ pinned − hidden)
        .api_route(
            "/conversations/{id}/deliverables",
            get_with(list_deliverables, list_deliverables_docs),
        )
        .api_route(
            "/conversations/{id}/deliverables/{file_id}",
            post_with(pin_deliverable, pin_deliverable_docs)
                .delete_with(unpin_deliverable, unpin_deliverable_docs),
        )
}

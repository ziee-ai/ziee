//! Summarization module HTTP routes.

use aide::axum::{ApiRouter, routing::get_with};

use super::handlers::*;

pub fn summarization_router() -> ApiRouter {
    #[allow(unused_mut)]
    let mut router = ApiRouter::new()
        .api_route(
            "/summarization/settings",
            get_with(get_admin_settings, get_admin_settings_docs)
                .put_with(update_admin_settings, update_admin_settings_docs),
        )
        .api_route(
            "/conversations/{conversation_id}/summary",
            get_with(get_conversation_summary, get_conversation_summary_docs),
        );

    // Test-only synchronous summary-refresh hook. Compiled into debug
    // builds only — `cargo test` and dev `cargo run` see it;
    // `cargo build --release` strips it out so it can't reach
    // production binaries.
    #[cfg(debug_assertions)]
    {
        use aide::axum::routing::post_with;
        router = router.api_route(
            "/_test/summarization/refresh",
            post_with(test_refresh, test_refresh_docs),
        );
    }
    router
}

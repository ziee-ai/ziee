//! Axum router mounting the proxy handlers under [`super::proxy::LOCAL_PROXY_PATH`].
//!
//! Three handlers, OpenAI-compat shaped:
//!  - `POST /api/local-llm/v1/chat/completions` — streaming or non-streaming
//!  - `POST /api/local-llm/v1/embeddings`       — single response
//!  - `GET  /api/local-llm/v1/models`           — list this provider's models
//!
//! Each handler shares the same auth + lookup + forward pipeline,
//! parametrized only by HTTP method + endpoint suffix.

use aide::axum::ApiRouter;
use aide::axum::routing::{get_with, post_with};
use axum::extract::DefaultBodyLimit;

use super::proxy::LOCAL_PROXY_PATH;
use super::proxy_handlers::{
    proxy_chat_completions, proxy_chat_completions_docs, proxy_embeddings, proxy_embeddings_docs,
    proxy_models, proxy_models_docs, proxy_rerank, proxy_rerank_docs,
};

/// Cap on the inbound request body the proxy buffers before
/// forwarding (M-2). A chat-completions body is text — 16 MiB is far
/// beyond any real prompt while bounding the memory a single
/// token-holder can force the server to buffer.
const PROXY_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// Build the proxy sub-router. Mounted by `routes::llm_local_runtime_router`.
pub fn proxy_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            &format!("{}/chat/completions", LOCAL_PROXY_PATH),
            post_with(proxy_chat_completions, proxy_chat_completions_docs)
                .layer(DefaultBodyLimit::max(PROXY_BODY_LIMIT)),
        )
        .api_route(
            &format!("{}/embeddings", LOCAL_PROXY_PATH),
            post_with(proxy_embeddings, proxy_embeddings_docs)
                .layer(DefaultBodyLimit::max(PROXY_BODY_LIMIT)),
        )
        .api_route(
            &format!("{}/rerank", LOCAL_PROXY_PATH),
            post_with(proxy_rerank, proxy_rerank_docs)
                .layer(DefaultBodyLimit::max(PROXY_BODY_LIMIT)),
        )
        .api_route(
            &format!("{}/models", LOCAL_PROXY_PATH),
            get_with(proxy_models, proxy_models_docs),
        )
}

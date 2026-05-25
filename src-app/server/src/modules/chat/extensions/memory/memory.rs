//! `MemoryExtension` — hooks that will inject relevant memories before
//! the LLM call and harvest new ones after.
//!
//! Phase 1: both hooks return immediately. Phase 2 wires
//! `before_llm_call` to do retrieval; Phase 3 wires `after_llm_call` to
//! kick off background extraction via `tokio::spawn`.

use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::ChatRequest;

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, StreamContext,
};
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::models::Message;

pub struct MemoryExtension {
    #[allow(dead_code)]
    pool: PgPool,
}

impl MemoryExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChatExtension for MemoryExtension {
    fn name(&self) -> &str {
        "memory"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Memory extension initialized (Phase 1 stub — no retrieval/extraction yet)");
        Ok(())
    }

    /// Phase 2 will: load admin + user settings; if retrieval_enabled
    /// AND admin.enabled AND embedding model configured → embed the
    /// last user message → cosine top-K → prepend a system block.
    async fn before_llm_call(
        &self,
        _context: &mut StreamContext,
        _request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        Ok(BeforeLlmAction::Continue)
    }

    /// Phase 3 will: spawn a background task that loads the last user
    /// + assistant messages, calls the extraction LLM, parses
    /// ADD/UPDATE/DELETE/NOOP ops, and writes via `Repos.memory`.
    async fn after_llm_call(
        &self,
        _context: &StreamContext,
        _final_message: &Message,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        Ok(ExtensionAction::Complete)
    }
}

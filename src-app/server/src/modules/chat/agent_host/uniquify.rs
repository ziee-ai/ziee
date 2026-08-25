//! Tool-use id uniquification for the agent-core CHAT path.
//!
//! The legacy chat loop mints a fresh `call_<uuid>` for any tool_use id that is
//! empty OR already taken on the assistant message (`resolve_unique_tool_use_id`,
//! `mcp/chat_extension/mcp.rs`), so a provider that REUSES an id across turns
//! (gpt-oss/harmony's constant `"tool_use"`, or a scripted stub) never collides.
//!
//! The agent-core loop persists the model's raw ids, so a reused id collides with
//! an already-persisted / just-claimed `tool_use_approvals` row keyed by that id —
//! the approval-claim regression: on a resume turn the model re-emits the same id,
//! a fresh pending row is created under the same key, and it SURVIVES the claim of
//! the original (each `mcp::approval_claim_test`).
//!
//! Fix: wrap the chat `ModelClient` and rewrite reused tool_use ids in the returned
//! assistant message at the ONE point that feeds the gate, the transcript, and the
//! tool_result pairing consistently (before the crate loop extracts `ToolCall`s).
//! This runs ONLY on the agent-core chat path (flag ON); the legacy path is
//! untouched.

use std::collections::HashSet;
use std::sync::Arc;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock};
use agent_core::{DeltaSink, ModelClient, Usage};
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::content::MessageContentData;
use crate::modules::mcp::chat_extension::content::McpContentData;
use crate::modules::mcp::chat_extension::mcp::resolve_unique_tool_use_id;

/// Wraps the real chat `ModelClient`; rewrites any tool_use id in the returned
/// assistant message that is empty or already taken (by a prior loop iteration or
/// a prior turn on the SAME `assistant_message_id`) with a fresh `call_<uuid>` —
/// byte-for-byte the legacy finalize behavior.
pub struct UniquifyingModelClient {
    inner: Arc<dyn ModelClient>,
    pool: PgPool,
    /// The assistant message the reply's tool_use blocks are persisted under — the
    /// same id the gate + transcript key on, so seeding the used-set from it sees a
    /// prior turn's reused id.
    assistant_message_id: Uuid,
}

impl UniquifyingModelClient {
    pub fn new(inner: Arc<dyn ModelClient>, pool: PgPool, assistant_message_id: Uuid) -> Self {
        Self { inner, pool, assistant_message_id }
    }

    /// Seed the used-id set from tool_use ids already persisted on this assistant
    /// message. Mirrors `mcp.rs`'s targeted `content_type = 'tool_use'` query;
    /// fail-soft to empty on a DB error (never fail the turn over uniquification).
    async fn seed_used(&self) -> HashSet<String> {
        let mut used = HashSet::new();
        match sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT content FROM message_contents \
             WHERE message_id = $1 AND content_type = 'tool_use'",
        )
        .bind(self.assistant_message_id)
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => {
                for raw in rows {
                    if let Ok(data) = serde_json::from_value::<MessageContentData>(raw)
                        && let Ok(McpContentData::ToolUse { id, .. }) =
                            McpContentData::from_message_content(&data)
                        && !id.is_empty()
                    {
                        used.insert(id);
                    }
                }
            }
            Err(e) => tracing::warn!(
                "uniquify: could not load existing tool_use ids for message {}: {e}; \
                 degrading to empty used-set",
                self.assistant_message_id
            ),
        }
        used
    }

    async fn rewrite(&self, mut msg: ChatMessage) -> ChatMessage {
        // Fast path: no tool_use blocks → nothing to uniquify (no DB hit).
        if !msg.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. })) {
            return msg;
        }
        let mut used = self.seed_used().await;
        for block in msg.content.iter_mut() {
            if let ContentBlock::ToolUse { id, .. } = block {
                let unique = resolve_unique_tool_use_id(id, &used);
                used.insert(unique.clone());
                *id = unique;
            }
        }
        msg
    }
}

#[async_trait]
impl ModelClient for UniquifyingModelClient {
    async fn call(&self, req: ChatRequest) -> Result<(ChatMessage, Usage), AppError> {
        let (msg, usage) = self.inner.call(req).await?;
        Ok((self.rewrite(msg).await, usage))
    }

    async fn call_streaming(
        &self,
        req: ChatRequest,
        sink: &dyn DeltaSink,
    ) -> Result<(ChatMessage, Usage), AppError> {
        let (msg, usage) = self.inner.call_streaming(req, sink).await?;
        Ok((self.rewrite(msg).await, usage))
    }
}

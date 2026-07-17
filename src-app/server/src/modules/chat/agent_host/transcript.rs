//! Chat agent-host port: `ChatTranscriptStore` — the block-aware chat transcript
//! (the `agent_core::TranscriptStore` impl). This is the crux port: it maps the
//! agent-core loop's flat `Vec<ai_providers::ChatMessage>` turn model onto chat's
//! **one-row-per-content-block** persistence (`message_contents`, UNIQUE(message_id,
//! sequence_order)), reproducing today's streaming persist + reconstruction
//! byte-for-byte so nothing about the on-wire history the model sees changes.
//!
//! # run_id → persistence mapping
//!
//! Chat has no single "run" uuid; a turn is addressed by
//! `(conversation_id, branch_id, assistant_message_id)`. The host therefore
//! constructs ONE `ChatTranscriptStore` **per assistant turn** with those ids baked
//! in, and the store treats the trait's `run_id` param as advisory (a `debug_assert`
//! pins `run_id == assistant_message_id` to catch host-wiring mistakes; the store
//! otherwise relies on its baked ids). Concretely:
//!
//! - **`load`** reconstructs from `branch_id` (the full-branch AI-context history).
//! - **`append`** / **`replace_head`** target `assistant_message_id` — the per-turn
//!   assistant DB message that accumulates ALL of this turn's blocks (assistant
//!   text/thinking/tool_use AND the tool_results), exactly as chat stores them
//!   today (the load path treats one assistant DB message as carrying the merged
//!   tool_use + tool_result blocks; see `group_assistant_blocks`).
//!
//! # Phase-5 UX walk
//!
//! A multi-tool assistant turn: the model streams text + thinking, requests two
//! tools, gets two results. `append(Role::Assistant, [text, thinking, tool_use,
//! tool_use])` persists those four blocks onto `assistant_message_id` with
//! monotonically-increasing `sequence_order`; `append(Role::Tool, [tool_result,
//! tool_result])` appends the two results onto the SAME message. The UI renders
//! exactly those blocks in order (it reads `message_contents`), and on the next
//! turn `load()` reconstructs the identical `[Assistant{text,thinking,tool_use×2},
//! Tool{tool_result×2}]` pairing via `group_assistant_blocks` — the same history the
//! model saw before this port existed.
//!
//! # Phase-5 infra-integration walk (touches + invariants)
//!
//! - **`message_contents` UNIQUE(message_id, sequence_order)** (constraint
//!   `uq_message_contents_message_sequence`, migration 124) — every insert here
//!   must land a distinct `sequence_order` for the message or the INSERT hard-errors
//!   (never silently duplicates a slot).
//! - **MAX+1 sequencing invariant** — the streamed content-block indices and each
//!   extension accumulator's indices come from INDEPENDENT sources, so they overlap.
//!   `append` (assistant) therefore reads `base = COALESCE(MAX(sequence_order),-1)+1`
//!   ONCE inside a tx and assigns a single monotonic counter (NOT `base+index`),
//!   exactly like `DeltaAccumulator::finalize`. The `Role::Tool` result append uses
//!   `Repos.chat.core.append_content`, whose MAX+1 is computed INSIDE the INSERT.
//! - **branch junction** — `load` reads via `get_conversation_history(branch_id)`
//!   (`list_messages_in_branch` joins `branch_messages`, ordered by junction
//!   `created_at ASC`; contents ordered `sequence_order ASC, created_at ASC, id ASC`).
//! - **thinking-metadata-in-JSON** — thinking blocks persist their
//!   `ThinkingMetadata{token_count,signature,redacted_data}` inside the content JSON
//!   (round-tripped through `MessageContentData::Thinking`). The provider block
//!   carries no reasoning-token count, so `token_count` is `None` here (matching the
//!   text extension's `convert_from_content_block`); the streaming path's separate
//!   `reasoning_tokens` restoration has no analog on this seam.
//! - **`sync:conversation`** — emitted by the HOST after the turn, NOT here (this
//!   port only persists rows; the host owns the sync + SSE surface).
//!
//! # Reconstruction fidelity
//!
//! `load` replicates `streaming::convert_history_to_messages_with_extensions`
//! block-for-block (registry-driven per-block conversion + the pure
//! `group_assistant_blocks` grouping). The pre-wire `dedup_tool_results_by_id`
//! checkpoint is intentionally NOT applied here — in the original flow it runs as
//! the LAST step before the wire, AFTER `before_llm_call` extensions push additional
//! tool_results, so it belongs to the loop's pre-wire assembly, not to transcript
//! load. Keeping it out preserves the original placement.

use std::sync::Arc;

use agent_core::{ToolCallRecord, TranscriptStore};
use ai_providers::{ChatMessage, ContentBlock, Role};
use async_trait::async_trait;
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::chat::core::extension::{ExtensionRegistry, StreamContext};
use crate::modules::chat::core::models::{MessageContentData, MessageRole};
use crate::modules::chat::core::services::streaming::group_assistant_blocks;
use crate::modules::chat::extensions::text::types::ThinkingMetadata;
use crate::modules::mcp::chat_extension::content::McpContentData;

/// The chat-flavored [`TranscriptStore`]. Turn-scoped: one instance per assistant
/// turn, constructed by the host with the ids + registry it already has in hand.
pub struct ChatTranscriptStore {
    pub pool: sqlx::PgPool,
    /// Branch whose full history `load` reconstructs.
    pub branch_id: Uuid,
    /// The per-turn assistant message that `append`/`replace_head` write onto.
    pub assistant_message_id: Uuid,
    /// Registry that drives per-block `MessageContentData` ↔ provider `ContentBlock`
    /// conversion. `None` degrades exactly like the original conversion path with no
    /// registry (produces no blocks); the host always supplies `Some`.
    pub registry: Option<Arc<ExtensionRegistry>>,
    /// The transform context seeded by the host for this turn (metadata +
    /// iteration), passed to the registry's per-block hooks so file-recency /
    /// attachment replay behaves identically to `streaming.rs`.
    pub transform_context: StreamContext,
}

impl ChatTranscriptStore {
    /// Convert one provider `ContentBlock` back into a persistable
    /// `MessageContentData`, preferring the extension registry (the text extension
    /// owns text/thinking/redacted-thinking) and falling back to the registry-free
    /// construction — byte-identical to `DeltaAccumulator::finalize`'s conversion.
    /// Tool blocks (`ToolUse`/`ToolResult`), which no extension's
    /// `convert_from_content_block` claims, are mapped via
    /// [`McpContentData::from_content_block`] → `to_message_content()` (the exact
    /// serde bridge the MCP extension uses). Returns `None` for block kinds that
    /// never appear at the top level of a loop-produced assistant/tool message
    /// (Image/Document) — the caller skips them with a warning rather than
    /// swallowing silently.
    fn block_to_content_data(&self, block: &ContentBlock) -> Option<MessageContentData> {
        // 1. Registry-owned conversion (text ext: Text / Thinking / RedactedThinking).
        if let Some(registry) = &self.registry
            && let Some(content) = registry.convert_from_content_block(block)
        {
            return Some(content);
        }

        // 2. Tool blocks via the MCP content bridge (server_id__name parsing,
        //    tool_result text flattening) — identical to the streaming persist path.
        if let Some(mcp) = McpContentData::from_content_block(block) {
            return Some(mcp.to_message_content());
        }

        // 3. Registry-free fallback for text/thinking (mirrors both `finalize`'s
        //    fallback and the text extension's converter exactly).
        match block {
            ContentBlock::Text { text } => Some(MessageContentData::Text { text: text.clone() }),
            ContentBlock::Thinking { thinking, signature } => Some(MessageContentData::Thinking {
                thinking: thinking.clone(),
                metadata: signature.as_ref().map(|sig| ThinkingMetadata {
                    token_count: None,
                    signature: Some(sig.clone()),
                    redacted_data: None,
                }),
            }),
            ContentBlock::RedactedThinking { data } => Some(MessageContentData::Thinking {
                thinking: String::new(),
                metadata: Some(ThinkingMetadata {
                    token_count: None,
                    signature: None,
                    redacted_data: Some(data.clone()),
                }),
            }),
            _ => None,
        }
    }

    /// Persist an assistant turn's blocks onto `assistant_message_id` in ONE
    /// transaction, reproducing `DeltaAccumulator::finalize`: read `base =
    /// COALESCE(MAX(sequence_order),-1)+1` once, then assign a single monotonic
    /// counter so streamed + extension block indices (independent sources) never
    /// collide on the UNIQUE(message_id, sequence_order) constraint.
    async fn append_assistant_blocks(&self, blocks: &[ContentBlock]) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        let base: i32 = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence_order), -1) + 1 AS "next!"
               FROM message_contents WHERE message_id = $1"#,
            self.assistant_message_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        let mut next_seq = base;
        for block in blocks {
            let Some(content_data) = self.block_to_content_data(block) else {
                tracing::warn!(
                    "ChatTranscriptStore: skipping non-persistable assistant block on message {}",
                    self.assistant_message_id
                );
                continue;
            };
            let content_type = content_data.content_type();
            let content_json = content_data.to_api_content();

            sqlx::query!(
                r#"
                INSERT INTO message_contents (message_id, content_type, content, sequence_order)
                VALUES ($1, $2, $3, $4)
                "#,
                self.assistant_message_id,
                content_type,
                content_json,
                next_seq
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
            next_seq += 1;
        }

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(())
    }

    /// Append a `Role::Tool` message's tool_result blocks onto
    /// `assistant_message_id` via `Repos.chat.core.append_content` — the MAX+1
    /// -inside-the-INSERT path chat already uses for tool results (same slot
    /// computation, per-block).
    async fn append_tool_blocks(&self, blocks: &[ContentBlock]) -> Result<(), AppError> {
        for block in blocks {
            let Some(content_data) = self.block_to_content_data(block) else {
                tracing::warn!(
                    "ChatTranscriptStore: skipping non-persistable tool block on message {}",
                    self.assistant_message_id
                );
                continue;
            };
            let content_type = content_data.content_type();
            Repos
                .chat
                .core
                .append_content(self.assistant_message_id, &content_type, content_data)
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl TranscriptStore for ChatTranscriptStore {
    /// Reconstruct the full branch history into provider `ChatMessage`s, replicating
    /// `streaming::convert_history_to_messages_with_extensions`: skip system
    /// messages; for assistant messages, convert each stored block through the
    /// registry then group per-iteration pairs via `group_assistant_blocks`; for
    /// user messages, convert all blocks into one `User` message.
    async fn load(&self, run_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        debug_assert_eq!(
            run_id, self.assistant_message_id,
            "ChatTranscriptStore is turn-scoped; run_id should equal assistant_message_id"
        );
        let _ = run_id;

        let history = Repos.chat.core.get_conversation_history(self.branch_id).await?;

        let mut messages: Vec<ChatMessage> = Vec::new();

        for msg_with_content in &history {
            let role = msg_with_content
                .message
                .role_enum()
                .map_err(|e| AppError::internal_error(format!("Invalid message role: {}", e)))?;

            if role == MessageRole::System {
                continue;
            }

            if role == MessageRole::Assistant {
                // Invariant (mirrors streaming.rs): blocks arrive in strictly
                // increasing sequence_order (repository MAX+1). The pairing walk in
                // group_assistant_blocks relies on it — surface a regression loudly.
                let monotonic = msg_with_content
                    .contents
                    .windows(2)
                    .all(|w| w[0].sequence_order < w[1].sequence_order);
                if !monotonic {
                    tracing::warn!(
                        "non-monotonic sequence_order in assistant message {}; tool_use/tool_result pairing may be unreliable",
                        msg_with_content.message.id
                    );
                }

                let mut blocks: Vec<ContentBlock> = Vec::new();
                for content in &msg_with_content.contents {
                    let content_data = content.parse_content()?;

                    // Let an extension claim this block as skip-from-forwarding
                    // (the file extension drops UI-only FileAttachment artifacts).
                    if let Some(registry) = &self.registry
                        && registry
                            .should_skip_in_assistant_forwarding(
                                &content_data,
                                &self.transform_context,
                            )
                            .await?
                    {
                        continue;
                    }

                    let block = if let Some(registry) = &self.registry {
                        match registry
                            .process_content_for_llm(&content_data, &self.transform_context)
                            .await?
                        {
                            Some(transformed_block) => Some(transformed_block),
                            None => {
                                let ext_content =
                                    serde_json::to_value(&content_data).map_err(|e| {
                                        AppError::internal_error(format!(
                                            "Failed to serialize content: {}",
                                            e
                                        ))
                                    })?;
                                registry.convert_extension_to_content_block(&ext_content)
                            }
                        }
                    } else {
                        None
                    };

                    if let Some(b) = block {
                        blocks.push(b);
                    }
                }

                messages.extend(group_assistant_blocks(blocks));
                continue;
            }

            // Non-assistant (user) messages: convert all blocks normally.
            let mut all_blocks = Vec::new();
            for content in &msg_with_content.contents {
                let content_data = content.parse_content()?;

                let block = if let Some(registry) = &self.registry {
                    match registry
                        .process_content_for_llm(&content_data, &self.transform_context)
                        .await?
                    {
                        Some(transformed_block) => Some(transformed_block),
                        None => {
                            let ext_content = serde_json::to_value(&content_data).map_err(|e| {
                                AppError::internal_error(format!(
                                    "Failed to serialize content: {}",
                                    e
                                ))
                            })?;
                            registry.convert_extension_to_content_block(&ext_content)
                        }
                    }
                } else {
                    None
                };

                if let Some(b) = block {
                    all_blocks.push(b);
                }
            }

            if !all_blocks.is_empty() {
                let ai_role = match role {
                    MessageRole::User => Role::User,
                    MessageRole::Assistant => Role::Assistant,
                    MessageRole::System => continue,
                };
                messages.push(ChatMessage {
                    role: ai_role,
                    content: all_blocks,
                });
            }
        }

        Ok(messages)
    }

    /// Persist ONE assistant/tool turn as per-block `message_contents` rows on
    /// `assistant_message_id`. Assistant turns use the `finalize` tx + monotonic
    /// MAX+1 counter; tool turns append each result via the `append_content` MAX+1
    /// path. (User/System turns are not produced by the loop; their blocks would be
    /// persisted via the tool path as a defensive fallback.)
    async fn append(&self, run_id: Uuid, msg: ChatMessage) -> Result<(), AppError> {
        debug_assert_eq!(run_id, self.assistant_message_id);
        let _ = run_id;

        match msg.role {
            Role::Assistant => self.append_assistant_blocks(&msg.content).await,
            _ => self.append_tool_blocks(&msg.content).await,
        }
    }

    /// Compaction sink. Chat messages are IMMUTABLE — compaction is a
    /// context-window transform, never a data deletion — so this does NOT rewrite or
    /// remove the head `message_contents` rows. Instead it upserts the summary text
    /// into `conversation_summaries` (PK `branch_id`), the same durable rolling-
    /// summary record the summarization module owns, with `message_count = upto`.
    ///
    /// `summarized_up_to_id` is left NULL: `upto` is an index into the loop's flat
    /// `Vec<ChatMessage>`, which does NOT map 1:1 to DB message ids (one assistant DB
    /// message expands into an `Assistant`+`Tool` pair during load), so there is no
    /// faithful id to record; the column is nullable (`ON DELETE SET NULL`) and NULL
    /// is a legitimate value. The next turn's `load` still returns full history and
    /// the compactor re-derives the summary — matching chat's per-turn (not
    /// destructive) summarization semantics today.
    async fn replace_head(
        &self,
        run_id: Uuid,
        summary: ChatMessage,
        upto: usize,
    ) -> Result<(), AppError> {
        debug_assert_eq!(run_id, self.assistant_message_id);
        let _ = run_id;

        // Flatten the summary message's text blocks into the stored summary_text.
        let summary_text: String = summary
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let message_count = i32::try_from(upto).unwrap_or(i32::MAX);

        sqlx::query!(
            r#"
            INSERT INTO conversation_summaries
                (branch_id, summary_text, summarized_up_to_id, message_count, updated_at)
            VALUES ($1, $2, NULL, $3, now())
            ON CONFLICT (branch_id) DO UPDATE
                SET summary_text = EXCLUDED.summary_text,
                    summarized_up_to_id = EXCLUDED.summarized_up_to_id,
                    message_count = EXCLUDED.message_count,
                    updated_at = now()
            "#,
            self.branch_id,
            summary_text,
            message_count
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }

    /// No-op: the durable tool-call journal row is already written by the recording
    /// chokepoint inside `McpSession::call_tool` (one row per invocation in
    /// `mcp_tool_calls`), so an extra insert here would double-record. The transcript
    /// itself (the tool_result appended via `append`) is the resume source. Matches
    /// `WorkflowTranscriptStore::journal_tool_call`'s documented stance.
    async fn journal_tool_call(&self, _run_id: Uuid, _rec: ToolCallRecord) -> Result<(), AppError> {
        Ok(())
    }

    /// Empty: idempotent resume-replay by key (ITEM-16) is a later durability stage;
    /// the base loop replays via the transcript, never consulting this. Matches
    /// `WorkflowTranscriptStore::completed_tool_calls`.
    async fn completed_tool_calls(
        &self,
        _run_id: Uuid,
    ) -> Result<Vec<ToolCallRecord>, AppError> {
        Ok(Vec::new())
    }
}

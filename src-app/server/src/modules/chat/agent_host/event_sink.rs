//! Chat host port: `EventSink` (agent-core → chat SSE frames). Re-home wave 5.
//!
//! Maps the shared [`agent_core::AgentEvent`] loop stream onto the chat token
//! stream (`GET /api/chat/stream`), reproducing — 1:1 — the mapping the legacy
//! consumer in `core/services/streaming.rs` (~L854-951) performs when it turned
//! provider chunks into `started`/`content`/`complete` frames. The WORKFLOW twin
//! is `WorkflowEventSink` (`modules/workflow/agent_dispatch.rs`); this is its
//! chat-flavored counterpart. The `started` frame + the terminal
//! `sync:conversation` refetch are the HOST's job (`ChatAgentDispatcher`), not
//! this sink — see the infra-integration walk.
//!
//! ## UX walk (what the user experiences)
//! The user sends a message and watches the assistant answer materialize token
//! by token — every [`AgentEvent::ContentDelta`] becomes an
//! `SSEChatStreamEvent::Content` frame carrying a single text/thinking delta,
//! stamped with the assistant `message_id` so the receiving device attributes it
//! to the right bubble (and a mid-stream joiner replays it from the buffer).
//! Tool activity surfaces as it happens ([`AgentEvent::ToolNotification`] →
//! a raw `mcpToolProgress` event routed to whatever conversation the connection
//! is viewing). When the loop stops, a single `SSEChatStreamEvent::Complete`
//! frame closes the turn with a finish reason (+ folded token usage) — a clean
//! completion identical to today's `/api/chat/stream`. This is byte-for-byte the
//! experience the pre-migration streaming consumer produced.
//!
//! ## Infra-integration walk (every seam this touches)
//! - **Per-user SSE registry** — frames are addressed by `owner_id` (the user)
//!   and fanned to that user's connections currently subscribed to
//!   `conversation_id` via the free fn [`publish_frame`]
//!   (`chat/stream/registry.rs`). No per-request channel; this is the
//!   multiplexed device stream.
//! - **Per-conversation replay `GenerationBuffer`** — `publish_frame` appends
//!   `content` frames to the conversation's replay buffer (opened by the host's
//!   `started` frame, dropped on the terminal `complete`/`error`), so a device
//!   that joins mid-generation catches up. This sink emits the `content` +
//!   `complete` frames that drive that buffer; it never emits `started` (host)
//!   or `error` (the host's terminal guard owns the failure path).
//! - **`started`/`content`/`complete` frame contract** — preserved exactly:
//!   `content` chunks carry `message_id`/`conversation_id`/`branch_id` and NO
//!   `finish_reason`/`usage`; the single terminal `complete` carries the
//!   finish reason + usage. Usage is *folded* across [`AgentEvent::Usage`]
//!   events and rides the Complete frame (mirrors the legacy terminal chunk),
//!   never its own frame.
//! - **`from_ai_providers_delta` → `None` for signature/redacted deltas** — the
//!   core converter drops `ThinkingSignatureDelta`/`RedactedThinkingDelta`
//!   (not user-visible); this sink does NOT stream those, matching the legacy
//!   `DeltaAccumulator::process_chunk` behavior. Tool-use deltas also convert to
//!   `None` here and are not streamed — the finalized tool request rides the
//!   re-homed MCP extension's own raw lifecycle events, not this delta path.
//! - **`sync:conversation` terminal refetch** — emitted by the HOST after the
//!   turn commits (so the sidebar + other devices refetch the persisted turn),
//!   exactly as the legacy consumer did at its tail. NOT this sink's job.
//! - **Fire-and-forget streaming contract** — `emit` returns `()` and never
//!   errors; a dead/stalled connection is pruned inside the registry (the
//!   sender is dropped and the client reconnects + resyncs). A frame that can't
//!   be delivered is never surfaced back into the loop.


use agent_core::{AgentEvent, EventSink, StopReason, Usage as AgentUsage};
use async_trait::async_trait;
use uuid::Uuid;

use crate::modules::chat::core::types::streaming::{
    ChatStreamChunk, ContentBlockDelta, SSEChatStreamEvent, SSEChatStreamTaskListChangedData,
    TaskListItemDto, Usage,
};
use crate::modules::chat::stream::{publish_frame, publish_raw_event, ChatStreamFrame};

/// Chat-flavored [`EventSink`]: routes loop events to the per-user chat token
/// stream for one assistant turn. Constructed per turn by `ChatAgentDispatcher`.
pub struct ChatEventSink {
    /// Stream owner (the user) — addresses the per-user SSE registry.
    owner_id: Uuid,
    /// Routing key for every frame + the replay-buffer key.
    conversation_id: Uuid,
    /// Branch the turn belongs to — echoed on `content` chunks for wire parity
    /// with the legacy consumer (the client primarily learns it from `started`).
    branch_id: Uuid,
    /// The assistant message these deltas belong to — stamped as
    /// `ChatStreamChunk.message_id` so receivers attribute the tokens (the
    /// client learns `assistant_message_id` from the first `content` chunk).
    assistant_message_id: Uuid,
}

impl ChatEventSink {
    /// Build the sink for one assistant turn. `owner_id` is the acting user.
    pub fn new(
        owner_id: Uuid,
        conversation_id: Uuid,
        branch_id: Uuid,
        assistant_message_id: Uuid,
    ) -> Self {
        Self {
            owner_id,
            conversation_id,
            branch_id,
            assistant_message_id,
        }
    }

    /// Publish one generation frame to the owner's subscribed connections.
    fn publish(&self, event: SSEChatStreamEvent) {
        publish_frame(
            self.owner_id,
            ChatStreamFrame::new(self.conversation_id, event),
        );
    }

    /// Map the loop's `StopReason` to the chat `finish_reason` wire string.
    /// `NoToolCall` is the normal end (`"stop"`, the legacy value); `Halted` is
    /// a host cancellation (`"cancelled"`, matching the legacy cancel path). The
    /// cap stops carry descriptive-but-non-error reasons — the turn still
    /// completed cleanly with output; only `"cancelled"` has special client
    /// meaning, everything else renders as a normal completion.
    pub fn finish_reason(reason: StopReason) -> &'static str {
        match reason {
            StopReason::NoToolCall => "stop",
            StopReason::Halted => "cancelled",
            StopReason::IterationCap => "max_steps",
            StopReason::TokenCap => "token_cap",
            StopReason::WallClock => "timeout",
        }
    }

    /// Fold an accumulated agent usage into the chat `Usage` wire type, or `None`
    /// when the loop reported no usage (parity with the legacy terminal chunk,
    /// which omits `usage` when the provider sent none). `agent_core::Usage`
    /// carries only input/output/total totals, so reasoning/cache fields stay
    /// `None`. The dispatcher folds the turn's `AgentEvent::Usage` events and calls
    /// this to build the terminal `complete` frame.
    pub fn fold_usage(acc: AgentUsage) -> Option<Usage> {
        if acc.input_tokens == 0 && acc.output_tokens == 0 && acc.total_tokens == 0 {
            return None;
        }
        Some(Usage {
            input_tokens: Some(acc.input_tokens as u32),
            output_tokens: Some(acc.output_tokens as u32),
            reasoning_tokens: None,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
        })
    }
}

#[async_trait]
impl EventSink for ChatEventSink {
    async fn emit(&self, ev: AgentEvent) {
        match ev {
            // LIVE per-token stream — the whole point of the streaming seam. One
            // provider delta → one `content` frame carrying that single delta,
            // stamped with the assistant message id. A delta the core converter
            // returns `None` for (thinking-signature / redacted-thinking, and
            // tool-use deltas) is NOT streamed — exactly the legacy behavior.
            AgentEvent::ContentDelta(delta) => {
                if let Some(core_delta) = ContentBlockDelta::from_ai_providers_delta(&delta) {
                    self.publish(SSEChatStreamEvent::Content(ChatStreamChunk {
                        content: vec![core_delta],
                        message_id: Some(self.assistant_message_id),
                        conversation_id: Some(self.conversation_id),
                        branch_id: Some(self.branch_id),
                        finish_reason: None,
                        usage: None,
                        error: None,
                    }));
                }
            }

            // No-op: usage rides the terminal `complete` frame, which the host
            // (dispatcher) now emits — it folds the turn's `Usage` events itself
            // (via `Self::fold_usage`), so the sink doesn't accumulate here.
            AgentEvent::Usage(_u) => {}

            // The agent's live task list changed (ITEM-36 / DEC-56). Mirror the
            // `mcpToolProgress` side-channel: build the `taskListChanged` frame
            // (full current list, snake_case DTO the FE `TaskListChecklist`
            // reads) and deliver it via `publish_raw_event` — NOT `self.publish`
            // — so it rides the same raw/ephemeral channel as tool progress and
            // is not appended to the per-conversation content replay buffer (it
            // is not part of the assistant message content; a mid-join catches
            // up on the next change, and the durable TaskListStore is the
            // reload-time source of truth).
            AgentEvent::TaskListChanged { run_id, items } => {
                let event = SSEChatStreamEvent::TaskListChanged(SSEChatStreamTaskListChangedData {
                    run_id,
                    items: items.into_iter().map(TaskListItemDto::from).collect(),
                });
                publish_raw_event(self.owner_id, self.conversation_id, event.into());
            }

            // No-op: the terminal `complete` frame is emitted by the HOST
            // (dispatcher) AFTER the extension-event channel is drained, so a
            // late `titleUpdated` / tool event can't land after the terminal (the
            // sink's out-of-band drain would otherwise race the Complete). The
            // dispatcher computes the finish reason + usage from the returned event
            // stream via `Self::finish_reason` / `Self::fold_usage`.
            AgentEvent::Stopped(_reason) => {}

            // Coarse tool progress note → a raw `mcpToolProgress` event on the
            // per-conversation stream (routed to whichever conversation the
            // connection is viewing), the same channel the legacy consumer used
            // to forward the MCP extension's tool lifecycle events. `progress`
            // is 0.0 (a note, not a measured bar); `note` is the display text.
            AgentEvent::ToolNotification { server, note } => {
                use crate::modules::mcp::chat_extension::extension::SSEChatStreamMcpToolProgressData;
                let event = SSEChatStreamEvent::McpToolProgress(SSEChatStreamMcpToolProgressData {
                    message_id: Some(self.assistant_message_id.to_string()),
                    server,
                    progress_token: None,
                    progress: 0.0,
                    total: None,
                    message: Some(note),
                });
                publish_raw_event(self.owner_id, self.conversation_id, event.into());
            }

            // No-op: the finalized round message is already persisted by the
            // TranscriptStore, and its user-visible content was ALREADY streamed
            // token-by-token via `ContentDelta` above — re-emitting it here would
            // double the assistant text on the wire. Tool-use requests inside the
            // message ride the re-homed MCP extension's own raw lifecycle events,
            // not this sink.
            AgentEvent::Message(_msg) => {}

            // No-op: chat has no user-facing "context compacted" SSE variant (the
            // summarization/compaction path is silent to the token stream today);
            // the workflow host surfaces this as a progress log line, chat does
            // not. Left explicit so the intent is auditable.
            AgentEvent::HistoryReplaced { .. } => {}

            // No-op: the human-approval prompt is emitted by the HumanGate port
            // (the `McpApprovalRequired` / durable-elicit path), not the sink —
            // surfacing it here too would double the approval UI.
            AgentEvent::GateOpened(_) => {}
        }
    }
}

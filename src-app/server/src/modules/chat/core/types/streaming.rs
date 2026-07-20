// Chat streaming types

// Streaming API types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A chunk of streamed chat content (core streaming response from LLM)
///
/// IMPORTANT: Extensions should NOT add fields to this struct.
/// Instead, extensions should:
/// - Send their own SSE events via SSEChatStreamEvent variants
/// - Add new ContentBlockDelta variants if needed
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct ChatStreamChunk {
    /// Content block deltas
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentBlockDelta>,

    /// Message ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,

    /// Conversation ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<Uuid>,

    /// Branch ID (sent in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_id: Option<Uuid>,

    /// Finish reason (when stream completes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Usage metadata (when stream completes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// Error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StreamError>,
}

/// Content block delta - Base types (extensions CAN add more variants)
///
/// EXTENSIONS MAY extend this enum with new variants using the
/// compose_content_block_delta_variants macro. For example, the MCP extension
/// adds ToolUseDelta and ToolResultDelta variants.
///
/// Extension variants are automatically added by the compose_content_block_delta_variants macro.
#[macros::compose_content_block_delta_variants]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text content delta
    TextDelta {
        index: usize,
        delta: String,
    },

    /// Thinking content delta
    ThinkingDelta {
        index: usize,
        delta: String,
    },
}

impl ContentBlockDelta {
    /// Get the index of this content block
    pub fn index(&self) -> usize {
        match self {
            Self::TextDelta { index, .. } => *index,
            Self::ThinkingDelta { index, .. } => *index,
            Self::ToolUseDelta { index, .. } => *index,
        }
    }

    /// Convert from ai-providers ContentBlockDelta
    pub fn from_ai_providers_delta(delta: &ai_providers::ContentBlockDelta) -> Option<Self> {
        match delta {
            ai_providers::ContentBlockDelta::TextDelta { index, delta } => Some(Self::TextDelta {
                index: *index,
                delta: delta.clone(),
            }),
            ai_providers::ContentBlockDelta::ThinkingDelta { index, delta } => {
                Some(Self::ThinkingDelta {
                    index: *index,
                    delta: delta.clone(),
                })
            }
            // Tool-related deltas handled by MCP extension
            _ => None,
        }
    }
}

/// Usage metadata from AI provider
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Usage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    /// Reasoning/thinking tokens (if the provider reports them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
    /// Prompt tokens served from cache (the prompt-cache hit signal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
    /// Prompt tokens written to cache this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
}

impl From<ai_providers::StreamUsage> for Usage {
    fn from(usage: ai_providers::StreamUsage) -> Self {
        Self {
            input_tokens: Some(usage.prompt_tokens),
            output_tokens: Some(usage.completion_tokens),
            reasoning_tokens: usage.reasoning_tokens,
            cache_read_input_tokens: usage.cache_read_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
        }
    }
}

/// Stream error information
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StreamError {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

// ===================================================================
// Server-Sent Event Types
// ===================================================================

/// Data for the Started SSE event
/// Sent before content streaming begins to communicate conversation context
/// Client learns assistant_message_id from content chunks (message_id field)
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamStartedData {
    /// User message ID (None if resuming with tool approvals or regenerating)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_message_id: Option<Uuid>,

    /// Conversation ID
    pub conversation_id: Uuid,

    /// Branch ID
    pub branch_id: Uuid,
}

/// Data for the Complete SSE event
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamCompleteData {
    /// Finish reason
    pub finish_reason: String,

    /// Usage metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Data for the Error SSE event
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamErrorData {
    /// Error message
    pub message: String,

    /// Error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// The lifecycle status of one agent task-list item on the wire (ITEM-36 /
/// DEC-54). A thin server-side mirror of `agent_core::TaskStatus` — kept
/// separate so it can `#[derive(schemars::JsonSchema)]` (agent-core's enum
/// deliberately does not depend on schemars) and so the crate boundary stays
/// clean. Snake-case on the wire so `in_progress` reaches the FE
/// `TaskItemStatus` union (`pending | in_progress | completed`) unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskListItemStatus {
    Pending,
    InProgress,
    Completed,
}

impl From<agent_core::TaskStatus> for TaskListItemStatus {
    fn from(s: agent_core::TaskStatus) -> Self {
        match s {
            agent_core::TaskStatus::Pending => Self::Pending,
            agent_core::TaskStatus::InProgress => Self::InProgress,
            agent_core::TaskStatus::Completed => Self::Completed,
        }
    }
}

/// One agent task-list item on the wire (ITEM-36 / DEC-54) — the server-side DTO
/// mirror of `agent_core::TaskItem`. `content` is the imperative form
/// ("Run tests"); `active_form` the present-continuous form ("Running tests")
/// the FE emphasises while the item is `in_progress`. A thin DTO (with a
/// `From<agent_core::TaskItem>`) rather than putting the agent-core type on the
/// wire keeps the crates decoupled and lets it carry `schemars::JsonSchema`.
/// Fields are snake_case to match the FE `TaskItemVM` (`agentActivity.ts`).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct TaskListItemDto {
    pub id: Uuid,
    pub content: String,
    pub active_form: String,
    pub status: TaskListItemStatus,
}

impl From<agent_core::TaskItem> for TaskListItemDto {
    fn from(t: agent_core::TaskItem) -> Self {
        Self {
            id: t.id,
            content: t.content,
            active_form: t.active_form,
            status: t.status.into(),
        }
    }
}

/// Data for the `taskListChanged` SSE event (ITEM-36 / DEC-56). Emitted live
/// during the turn each time the agent's `task_*` core tools mutate the durable
/// list, carrying the FULL current list (small) so a surface — the chat
/// `TaskListChecklist` — re-renders in place without a refetch. Mirrors the
/// `mcpToolProgress` live side-channel event (routed via `publish_raw_event`,
/// not the replay-buffered generation frames). The `run_id` keys the run-scoped
/// surfaces; the chat FE additionally attaches it to the in-flight assistant
/// message it is currently streaming.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamTaskListChangedData {
    /// The agent run whose task list changed.
    pub run_id: Uuid,
    /// The full current task list (idempotent snapshot; not a delta).
    pub items: Vec<TaskListItemDto>,
}

/// Data for the `historyReplaced` SSE event (ITEM-61 / DEC-137). Emitted when the
/// conversation's context is COMPACTED — either the agent loop's automatic
/// compaction (`AgentEvent::HistoryReplaced`, forwarded by `event_sink.rs`) or the
/// manual `POST /conversations/{id}/compact` affordance — so the chat timeline
/// renders a "context compacted" marker in place. Compaction is OUTBOUND-ONLY (the
/// stored `message_contents` are never rewritten/deleted; only the rolling
/// `conversation_summaries` row is upserted), so this is a display signal, not a
/// data mutation. Routed via `publish_raw_event` (the raw side-channel).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamHistoryReplacedData {
    /// The conversation whose context was compacted.
    pub conversation_id: Uuid,
    /// How many leading transcript messages were folded into the rolling summary
    /// (0 when the manual endpoint summarized without a loop-relative index).
    pub summary_upto: usize,
}

/// The live status of one delegated sub-agent on the wire (Group A / ITEM-4 /
/// DEC-65). A thin server-side mirror of `agent_core::SubAgentChildStatus`
/// (kept separate so it can `#[derive(schemars::JsonSchema)]` and keep the
/// crate boundary clean). Snake-case so the FE `SubAgentChildStatus` union
/// (`agentActivity.ts`) consumes it unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentActivityChildStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl From<agent_core::SubAgentChildStatus> for SubAgentActivityChildStatus {
    fn from(s: agent_core::SubAgentChildStatus) -> Self {
        match s {
            agent_core::SubAgentChildStatus::Pending => Self::Pending,
            agent_core::SubAgentChildStatus::Running => Self::Running,
            agent_core::SubAgentChildStatus::Completed => Self::Completed,
            agent_core::SubAgentChildStatus::Failed => Self::Failed,
        }
    }
}

/// One delegated sub-agent on the wire (ITEM-4 / DEC-65) — the server-side DTO
/// mirror of `agent_core::SubAgentChild`. `id` is the child's run id; `label`
/// the friendly per-child descriptor (its objective / role). A thin DTO (with a
/// `From<agent_core::SubAgentChild>`) rather than putting the agent-core type on
/// the wire keeps the crates decoupled and lets it carry `schemars::JsonSchema`.
/// Fields are snake_case to match the FE `SubAgentChildVM` (`agentActivity.ts`).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SubAgentActivityChildDto {
    pub id: String,
    pub label: String,
    pub status: SubAgentActivityChildStatus,
}

impl From<agent_core::SubAgentChild> for SubAgentActivityChildDto {
    fn from(c: agent_core::SubAgentChild) -> Self {
        Self {
            id: c.id,
            label: c.label,
            status: c.status.into(),
        }
    }
}

/// Data for the `subAgentActivity` SSE event (ITEM-4 / DEC-65). Emitted live
/// during the turn when a `delegate` fan-out spawns its children (all
/// running/pending) and again as each child settles (completed/failed),
/// carrying the FULL current child list (idempotent last-wins snapshot, like
/// `taskListChanged`) so the timeline `SubAgentActivityCard` re-renders in
/// place. Delivered via `publish_raw_event` (the raw/ephemeral side-channel,
/// not the replay-buffered generation frames). `run_id` is the PARENT agent
/// run; the chat FE attaches it to the in-flight assistant message.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SSEChatStreamSubAgentActivityData {
    /// The parent agent run whose fan-out this is.
    pub run_id: Uuid,
    /// The full current sub-agent list (idempotent snapshot; not a delta).
    pub children: Vec<SubAgentActivityChildDto>,
}

/// SSE event enum for chat streaming
///
/// This enum represents all possible Server-Sent Events that can be streamed
/// during a chat message request.
///
/// # Extension Architecture
///
/// **EXTENSIONS SHOULD send their own SSE events** instead of adding fields to ChatStreamChunk.
/// Extensions add new event variants through the SSEChatStreamEventVariants enum using
/// the compose_chat_stream_events macro.
///
/// Example: The title extension sends a separate `TitleUpdated` event instead of
/// adding a title field to ChatStreamChunk.
///
/// Events are sent with proper `event:` names (e.g., "started", "content", "complete", "error", "titleUpdated")
/// for type-safe client-side handling.
#[macros::compose_chat_stream_events]
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SSEChatStreamEvent {
    /// Streaming started event (sent before content with message IDs)
    Started(SSEChatStreamStartedData),

    /// Content chunk event (streamed content deltas)
    Content(ChatStreamChunk),

    /// Stream completion event
    Complete(SSEChatStreamCompleteData),

    /// Error event
    Error(SSEChatStreamErrorData),

    /// The agent's live task list changed (ITEM-36 / DEC-56) — carries the full
    /// current list so the `TaskListChecklist` re-renders in place. A CORE
    /// variant (not an extension `SSEChatStreamEventVariants` one) because the
    /// agent-core loop, not a chat extension, is its source (`event_sink.rs`
    /// maps `AgentEvent::TaskListChanged` here).
    TaskListChanged(SSEChatStreamTaskListChangedData),

    /// A `delegate` fan-out's per-child status changed (ITEM-4 / DEC-65) —
    /// carries the full current sub-agent list so the timeline
    /// `SubAgentActivityCard` re-renders in place. A CORE variant (like
    /// `TaskListChanged`) because the agent-core loop's `fan_out`, not a chat
    /// extension, is its source (`event_sink.rs` maps
    /// `AgentEvent::SubAgentActivity` here).
    SubAgentActivity(SSEChatStreamSubAgentActivityData),

    /// The conversation's context was COMPACTED (ITEM-61 / DEC-137) — the manual
    /// `/compact` affordance or the loop's automatic compaction folded leading
    /// history into the rolling summary. A CORE variant (its sources are the
    /// agent-core loop's `AgentEvent::HistoryReplaced` via `event_sink.rs` and the
    /// `POST /conversations/{id}/compact` handler), so the chat timeline can render
    /// a "context compacted" marker in place.
    HistoryReplaced(SSEChatStreamHistoryReplacedData),
}

// Generic implementation that works for all variants (including extension-added ones)
impl SSEChatStreamEvent {
    /// Get the event name for this SSE event
    /// Uses serde's tag to extract the variant name in camelCase format
    pub fn event_name(&self) -> &'static str {
        // For core variants, return static strings (avoids allocation/conversion overhead)
        // Extension variants are handled dynamically
        // Extract variant name from Debug representation
        let debug_str = format!("{:?}", self);
        let variant_name = debug_str.split('(').next().unwrap_or("unknown");

        // Return static strings for known core variants only
        match variant_name {
            "Started" => "started",
            "Content" => "content",
            "Complete" => "complete",
            "Error" => "error",
            "TaskListChanged" => "taskListChanged",
            "SubAgentActivity" => "subAgentActivity",
            // Extension variants: convert PascalCase to camelCase dynamically
            _ => {
                // Convert first character to lowercase for camelCase
                // Note: This leaks a small amount of memory for each unique extension variant
                // but is only called once per variant type
                Box::leak(
                    variant_name
                        .chars()
                        .enumerate()
                        .map(|(i, c)| if i == 0 { c.to_lowercase().to_string() } else { c.to_string() })
                        .collect::<String>()
                        .into_boxed_str()
                )
            }
        }
    }

    /// Serialize the inner event data to JSON
    pub fn data(&self) -> Result<String, serde_json::Error> {
        // Serialize the entire variant - serde will handle it correctly with the tag
        serde_json::to_string(self)
    }
}

impl From<SSEChatStreamEvent> for axum::response::sse::Event {
    fn from(val: SSEChatStreamEvent) -> Self {
        axum::response::sse::Event::default()
            .event(val.event_name())
            .data(val.data().unwrap_or_default())
    }
}

#[cfg(test)]
mod compose_guard {
    //! Guard against the `compose_chat_stream_events` codegen silently dropping an
    //! extension-declared SSE variant. Each `SSEChatStreamEventVariants` variant an
    //! extension declares must appear on the composed `SSEChatStreamEvent` enum
    //! (macros/build.rs scans the per-module enums into `chat_extensions.rs`).
    //!
    //! Historically a stale `chat_extensions.rs` (from a `cargo:rerun-if-changed`
    //! DIRECTORY watch not re-firing on an IN-PLACE edit — now fixed with per-file
    //! watches in macros/build.rs) let a NEW variant be referenced by a handler yet
    //! be absent from `SSEChatStreamEvent`, so the tree built on a warm cache but
    //! FAILED a clean/CI build with `E0599: no variant …`. Referencing each variant
    //! constructor as a value here turns that into a LOUD, named compile error at a
    //! canonical location instead of a surprise deep in a handler.
    use super::SSEChatStreamEvent;

    #[test]
    fn extension_stream_variants_are_composed() {
        // Base variants.
        let _ = SSEChatStreamEvent::Started;
        // mcp/chat_extension variants (the class that regressed).
        let _ = SSEChatStreamEvent::McpApprovalRequired;
        let _ = SSEChatStreamEvent::McpElicitationRequired;
        let _ = SSEChatStreamEvent::RunJsApprovalRequired;
        // chat-internal extension variant.
        let _ = SSEChatStreamEvent::TitleUpdated;
        // ITEM-36 core variant (declared inline, not codegen'd — so it can't be
        // dropped by stale codegen, but keep it named here for symmetry).
        let _ = SSEChatStreamEvent::TaskListChanged;
        // ITEM-4 core variant (same rationale as TaskListChanged).
        let _ = SSEChatStreamEvent::SubAgentActivity;
    }
}

#[cfg(test)]
mod tasklist_frame {
    //! ITEM-36 / DEC-56 wire contract: the `taskListChanged` SSE frame the FE
    //! `TaskListChecklist` renders. Asserts the `agent_core::TaskItem` → wire
    //! DTO conversion preserves every field, and that the composed frame
    //! serializes to the exact shape the FE adapter (`taskItemsFromFrame`)
    //! reads — internally `type`-tagged, snake_case `run_id` / `active_form`,
    //! and the snake_case status token (`in_progress`).
    use super::{
        SSEChatStreamEvent, SSEChatStreamTaskListChangedData, TaskListItemDto,
    };
    use uuid::Uuid;

    #[test]
    fn task_item_converts_and_frame_serializes_to_fe_shape() {
        let run_id = Uuid::from_u128(0xABCD);
        let item_id = Uuid::from_u128(1);
        let core_items = vec![
            agent_core::TaskItem {
                id: item_id,
                content: "Run tests".into(),
                active_form: "Running tests".into(),
                status: agent_core::TaskStatus::InProgress,
                owner: Some("planner".into()),
                deps: vec![Uuid::from_u128(2)],
            },
            agent_core::TaskItem {
                id: Uuid::from_u128(3),
                content: "Write report".into(),
                active_form: "Writing report".into(),
                status: agent_core::TaskStatus::Pending,
                owner: None,
                deps: vec![],
            },
        ];

        let data = SSEChatStreamTaskListChangedData {
            run_id,
            items: core_items.into_iter().map(TaskListItemDto::from).collect(),
        };

        // Conversion preserved every wire field.
        assert_eq!(data.items.len(), 2);
        assert_eq!(data.items[0].id, item_id);
        assert_eq!(data.items[0].content, "Run tests");
        assert_eq!(data.items[0].active_form, "Running tests");

        let ev = SSEChatStreamEvent::TaskListChanged(data);
        assert_eq!(ev.event_name(), "taskListChanged");

        let v: serde_json::Value = serde_json::from_str(&ev.data().unwrap()).unwrap();
        assert_eq!(v["type"], "taskListChanged");
        assert_eq!(v["run_id"].as_str(), Some(run_id.to_string().as_str()));
        assert_eq!(v["items"].as_array().map(|a| a.len()), Some(2));
        assert_eq!(v["items"][0]["content"], "Run tests");
        assert_eq!(v["items"][0]["active_form"], "Running tests");
        assert_eq!(v["items"][0]["status"], "in_progress");
        assert_eq!(v["items"][1]["status"], "pending");
    }
}

#[cfg(test)]
mod subagent_activity_frame {
    //! ITEM-4 / DEC-65 wire contract: the `subAgentActivity` SSE frame the FE
    //! `SubAgentActivityCard` renders. Asserts the `agent_core::SubAgentChild`
    //! → wire DTO conversion preserves every field, and that the composed frame
    //! serializes to the exact shape the FE adapter
    //! (`subAgentActivityFromChildren`) reads — internally `type`-tagged,
    //! snake_case `run_id` / per-child `{id,label,status}`, and the snake_case
    //! status tokens (`running` / `completed` / `failed` / `pending`).
    use super::{
        SSEChatStreamEvent, SSEChatStreamSubAgentActivityData, SubAgentActivityChildDto,
    };
    use uuid::Uuid;

    #[test]
    fn child_converts_and_frame_serializes_to_fe_shape() {
        let run_id = Uuid::from_u128(0xF00D);
        let core_children = vec![
            agent_core::SubAgentChild {
                id: "child-a".into(),
                label: "Research angle one".into(),
                status: agent_core::SubAgentChildStatus::Running,
            },
            agent_core::SubAgentChild {
                id: "child-b".into(),
                label: "Research angle two".into(),
                status: agent_core::SubAgentChildStatus::Completed,
            },
            agent_core::SubAgentChild {
                id: "child-c".into(),
                label: "Research angle three".into(),
                status: agent_core::SubAgentChildStatus::Failed,
            },
        ];

        let data = SSEChatStreamSubAgentActivityData {
            run_id,
            children: core_children
                .into_iter()
                .map(SubAgentActivityChildDto::from)
                .collect(),
        };

        // Conversion preserved every wire field.
        assert_eq!(data.children.len(), 3);
        assert_eq!(data.children[0].id, "child-a");
        assert_eq!(data.children[0].label, "Research angle one");

        let ev = SSEChatStreamEvent::SubAgentActivity(data);
        assert_eq!(ev.event_name(), "subAgentActivity");

        let v: serde_json::Value = serde_json::from_str(&ev.data().unwrap()).unwrap();
        assert_eq!(v["type"], "subAgentActivity");
        assert_eq!(v["run_id"].as_str(), Some(run_id.to_string().as_str()));
        assert_eq!(v["children"].as_array().map(|a| a.len()), Some(3));
        assert_eq!(v["children"][0]["id"], "child-a");
        assert_eq!(v["children"][0]["label"], "Research angle one");
        assert_eq!(v["children"][0]["status"], "running");
        assert_eq!(v["children"][1]["status"], "completed");
        assert_eq!(v["children"][2]["status"], "failed");
    }
}

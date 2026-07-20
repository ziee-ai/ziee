//! Core value types for the agent loop (ITEM-3). Domain-neutral in spirit but
//! (per DEC-15) this is a ziee crate, so no N9 constraint. Messages/tools are
//! `ai-providers` types; a crate-local `ToolResult` carries `structured_content`
//! (which `ai_providers::ContentBlock::ToolResult` lacks).

use ai_providers::{ChatMessage, ContentBlock, ContentBlockDelta};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable per-tool-call idempotency key `<run_id>:<turn>:<ordinal>` (P6/INV-11).
pub type IdempotencyKey = String;

/// A tool invocation the model requested (extracted from a `ToolUse` block).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    /// Server (MCP) the tool belongs to, if known — drives approval/trust.
    pub server: Option<String>,
    pub name: String,
    pub input: serde_json::Value,
}

/// A completed tool result. The crate's own type: `ai_providers::ContentBlock::
/// ToolResult` has no `structured_content`, which the model recalls separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub structured_content: Option<serde_json::Value>,
    /// The tool's output is the FINAL turn answer — do not re-call the model after
    /// it (e.g. an MCP result annotated `audience: ["user"]` only). Default false.
    #[serde(default)]
    pub terminal: bool,
}

/// One journaled tool call (P5) — the durable record for resume replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub key: IdempotencyKey,
    pub call: ToolCall,
    pub result: ToolResult,
}

/// Token usage for a model call.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// Why the loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// Model emitted no tool call — final answer.
    NoToolCall,
    /// `max_steps` reached.
    IterationCap,
    /// Per-run or per-step token cap breached.
    TokenCap,
    /// Wall-clock deadline.
    WallClock,
    /// Cancelled / halted by the host.
    Halted,
}

/// Human review outcome (Codex `ReviewDecision`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDecision {
    Approved,
    ApprovedForSession,
    Denied,
    Abort,
}

/// What the `ApprovalPolicy` decides for a tool call *before* it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Run without asking (read-only / trusted).
    Auto,
    /// Ask a human (via the `HumanGate`).
    Prompt,
    /// Send to the reviewer agent first, which may auto-resolve or escalate.
    Review,
    /// Reject; the denial is returned to the model.
    Deny,
}

/// The technical sandbox boundary (Codex `SandboxPolicy`). Per DEC-2 the
/// per-call bwrap enforcement is descoped; this is carried as policy metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxMode {
    ReadOnly { network: bool },
    WorkspaceWrite { network: bool },
    DangerFullAccess,
}

/// The approval gate (Codex `AskForApproval`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalMode {
    UnlessTrusted,
    OnRequest,
    Granular,
    Never,
}

/// A ticket for a suspended human gate (durable in the workflow host).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateTicket {
    pub id: Uuid,
}

/// What a human is asked to decide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateAsk {
    pub call: ToolCall,
    pub reason: String,
}

/// The gate's answer: resolved live, or suspended (host must park the run).
#[derive(Debug, Clone)]
pub enum GateOutcome {
    Decided(ReviewDecision),
    Suspended(GateTicket),
}

/// Bounded parallel-fan-out limits (Codex `[agents]`; DEC-11).
#[derive(Debug, Clone, Copy)]
pub struct SubagentLimits {
    pub max_depth: u8,
    pub max_threads: u8,
    /// Max children accepted in ONE `delegate` call (DEC-1). `max_threads` bounds
    /// concurrency; this bounds the COUNT — over-cap truncates with an explicit
    /// "capped at N" note (never a silent drop). Taken as data (the crate is
    /// domain-free); the host threads it from `agent_admin_settings`.
    pub max_children_per_call: u16,
}

impl Default for SubagentLimits {
    fn default() -> Self {
        Self {
            max_depth: 1,
            max_threads: 6,
            max_children_per_call: 8,
        }
    }
}

/// One child in a fan-out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSpec {
    /// Different model per child; `None` inherits the parent (resolved via the
    /// `ModelResolver` port, RBAC-bound).
    pub model_id: Option<Uuid>,
    pub system: String,
    pub tool_scope: ToolScope,
    pub reasoning_effort: Option<String>,
}

/// A subagent returns a SUMMARY, never its transcript (P9).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSummary {
    pub summary: String,
}

/// The live status of one delegated sub-agent in a `delegate` fan-out
/// (Group A / ITEM-4 / DEC-65). Snake-case on the wire so the FE
/// `SubAgentChildStatus` union (`agentActivity.ts`) consumes it directly.
/// `pending` = spawned but queued behind the concurrency limit; `running` =
/// executing; `completed`/`failed` are terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentChildStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// One delegated sub-agent's activity snapshot entry (Group A / ITEM-4 /
/// DEC-65). `id` is the child's fresh run id; `label` is the friendly per-child
/// descriptor (its objective / role, derived from the child's system
/// instruction). Mirrors the FE `SubAgentChildVM`; a thin server DTO carries it
/// to the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentChild {
    pub id: String,
    pub label: String,
    pub status: SubAgentChildStatus,
}

// ---------------------------------------------------------------------------
// Group G — agent self-task-management (Claude-Code `Task`-tools-style)
// (ITEM-34/35, DEC-49/54). The item shape + status mirror CC's CURRENT Task
// tools (per-item create + patch-by-id + read-back), NOT legacy `TodoWrite`.
// ---------------------------------------------------------------------------

/// A task-list item's status (DEC-54). Snake-case on the wire so the model's
/// `status: "in_progress"` deserializes directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

/// One agent task-list item (ITEM-34 / DEC-54). `content` is the imperative
/// form ("Run tests"); `active_form` the present-continuous form rendered while
/// the item is `in_progress` ("Running tests") — CC's Anthropic-specific dual
/// form. `owner`/`deps` mirror CC's current Task tools (carried as data; the
/// crate does not hard-enforce dependency ordering — the model is guided by the
/// tool descriptions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: Uuid,
    pub content: String,
    pub active_form: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<Uuid>,
}

/// The fields to create a task item (the store assigns the `id`). `status`
/// defaults to `pending` when the model omits it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItemCreate {
    pub content: String,
    pub active_form: String,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub deps: Vec<Uuid>,
}

/// A partial patch to an existing task item — only the supplied fields change
/// (per-item patch-by-id, the CC `TaskUpdate` shape). `deps: Some(vec![])`
/// clears deps; `deps: None` leaves them untouched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskItemPatch {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub active_form: Option<String>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub deps: Option<Vec<Uuid>>,
}

/// The set of tool servers a turn may call (RBAC-resolved by the host).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolScope {
    pub servers: Vec<String>,
    /// Whether the core-injected `delegate` tool is offered (false in children
    /// → enforces `max_depth = 1`).
    pub allow_delegate: bool,
}

/// A self-paced agent's proposed next-fire signal (Group E / DEC-42), produced
/// by the model-facing `schedule_next` core tool and recorded through the
/// [`SchedulePort`](crate::ports::SchedulePort). The host (the scheduler's
/// self-paced dispatch) reads it AFTER the turn and feeds it to its existing
/// clamp + write-back (`next_self_paced_fire` → `arm_self_paced`); the crate
/// itself never clamps or persists.
///
/// * `stop == true` ends the self-paced loop (self-complete) — `delay_seconds`
///   is then irrelevant.
/// * otherwise `delay_seconds` is the model's requested wait until the next
///   turn (`None` ⇒ "as soon as allowed"; the host floors it to its
///   min-interval). `reason` is a short free-text rationale (surfaced by the
///   host, not part of the clamp).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleProposal {
    pub delay_seconds: Option<u64>,
    pub reason: Option<String>,
    pub stop: bool,
}

/// How a turn is seeded — a new message, or a resume of a persisted transcript.
#[derive(Debug, Clone)]
pub enum TurnSeed {
    NewMessage(ChatMessage),
    Resume,
}

/// The request driving one agent turn.
#[derive(Debug, Clone)]
pub struct AgentTurnRequest {
    pub run_id: Uuid,
    pub user_id: Uuid,
    pub seed: TurnSeed,
    pub system: Vec<ContentBlock>,
    pub tool_scope: ToolScope,
    pub start_iteration: u32,
    /// Opaque per-turn input bag (DEC-19) surfaced to extensions via
    /// `TurnContext.inputs`. `Null` for hosts (workflow) that don't use it.
    pub inputs: serde_json::Value,
}

/// The coarse event stream the loop yields (Goose `AgentEvent`; tool requests
/// ride INSIDE `Message` blocks — INV-7).
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Message(ChatMessage),
    /// A live streaming delta of the in-progress assistant message (ITEM-26 — the
    /// chat host maps these to `SSEChatStreamEvent::Content` frames; the workflow
    /// host ignores them). Emitted DURING the model call, before the final
    /// `Message`. Only the `ProviderModelClient` produces these; fake models don't.
    ContentDelta(ContentBlockDelta),
    Usage(Usage),
    ToolNotification { server: String, note: String },
    HistoryReplaced { summary_upto: usize },
    /// The agent's task list changed (Group G / ITEM-36) — emitted by the
    /// `task_*` core tools after a create/update mutates the durable store,
    /// carrying the full current list (small) so a surface renders without a
    /// refetch. A later server/FE tranche maps this to an SSE frame +
    /// content-block (mirroring `mcpToolProgress`); the workflow host maps it to
    /// a per-run progress track. Hosts that don't surface it ignore it.
    TaskListChanged { run_id: Uuid, items: Vec<TaskItem> },
    /// A `delegate` fan-out's per-child status changed (Group A / ITEM-4 /
    /// DEC-65) — emitted by `fan_out_inner` when the children are spawned (all
    /// `running`/`pending`) and again as each child settles
    /// (`completed`/`failed`), carrying the FULL current child list (a small,
    /// idempotent last-wins snapshot, exactly like `TaskListChanged`). `run_id`
    /// is the PARENT agent run. The chat host maps this to a `subAgentActivity`
    /// SSE frame (`event_sink.rs`) that drives the timeline
    /// `SubAgentActivityCard`; the workflow host rolls it up to a progress log
    /// line; hosts that don't surface it ignore it.
    SubAgentActivity { run_id: Uuid, children: Vec<SubAgentChild> },
    GateOpened(GateTicket),
    Stopped(StopReason),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_reason_roundtrips() {
        for r in [
            StopReason::NoToolCall,
            StopReason::IterationCap,
            StopReason::TokenCap,
            StopReason::WallClock,
            StopReason::Halted,
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: StopReason = serde_json::from_str(&s).unwrap();
            assert_eq!(r, back);
        }
    }

    #[test]
    fn review_and_decision_variants_present() {
        assert_eq!(
            serde_json::to_string(&ReviewDecision::ApprovedForSession).unwrap(),
            "\"ApprovedForSession\""
        );
        assert_eq!(serde_json::to_string(&Decision::Review).unwrap(), "\"Review\"");
    }

    #[test]
    fn subagent_limits_default_codex() {
        let l = SubagentLimits::default();
        assert_eq!(l.max_depth, 1);
        assert_eq!(l.max_threads, 6);
        assert_eq!(l.max_children_per_call, 8);
    }

    #[test]
    fn sub_agent_child_status_snake_case_wire() {
        // The FE `SubAgentChildStatus` union (agentActivity.ts) reads these
        // snake_case tokens verbatim.
        assert_eq!(serde_json::to_string(&SubAgentChildStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&SubAgentChildStatus::Running).unwrap(), "\"running\"");
        assert_eq!(
            serde_json::to_string(&SubAgentChildStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(serde_json::to_string(&SubAgentChildStatus::Failed).unwrap(), "\"failed\"");
    }

    #[test]
    fn tool_result_carries_structured_content() {
        let tr = ToolResult {
            content: vec![],
            is_error: false,
            structured_content: Some(serde_json::json!({"k": 1})),
            terminal: false,
        };
        let s = serde_json::to_string(&tr).unwrap();
        assert!(s.contains("structured_content"));
    }
}

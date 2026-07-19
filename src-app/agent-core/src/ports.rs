//! The six ports (ITEM-2) — the agent core's pluggable seams, the exact pattern
//! `ziee-identity`/`ziee-framework` use (traits here; the loop is generic over an
//! injected `Arc<P>`; the app supplies impls). The driver surfaces
//! `ziee_core::AppError`; pure ports use it too for host-mappability.

use std::sync::Arc;

use ai_providers::{Provider, Tool};
use async_trait::async_trait;
use uuid::Uuid;
use ziee_core::AppError;

use crate::types::{
    AgentEvent, Decision, GateAsk, GateOutcome, IdempotencyKey, SandboxMode, TaskItem,
    TaskItemCreate, TaskItemPatch, ToolCall, ToolCallRecord, ToolResult, ToolScope,
};

/// Turn history + the durable journal (chat: repos/`conversation_summaries`;
/// workflow: `agent_transcript_json` + `mcp_tool_calls`).
#[async_trait]
pub trait TranscriptStore: Send + Sync {
    async fn load(&self, run_id: Uuid) -> Result<Vec<ai_providers::ChatMessage>, AppError>;
    async fn append(&self, run_id: Uuid, msg: ai_providers::ChatMessage) -> Result<(), AppError>;
    /// Compaction sink: replace the head `upto` messages with a summary block.
    async fn replace_head(
        &self,
        run_id: Uuid,
        summary: ai_providers::ChatMessage,
        upto: usize,
    ) -> Result<(), AppError>;
    /// Journal a completed tool call (P5) — the resume replay record.
    async fn journal_tool_call(&self, run_id: Uuid, rec: ToolCallRecord) -> Result<(), AppError>;
    /// The replay set on resume (P6): tool calls already completed this turn.
    async fn completed_tool_calls(&self, run_id: Uuid)
        -> Result<Vec<ToolCallRecord>, AppError>;
}

/// Push loop events out (chat: SSE registry; workflow: `ProgressEmitter`).
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, ev: AgentEvent);
}

/// Enumerate + call tools, unifying built-in + external MCP (chat + workflow both
/// wrap `McpSession::call_tool`). `control_mcp` is reached here like any built-in.
#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list(&self, scope: &ToolScope) -> Result<Vec<Tool>, AppError>;
    async fn call(
        &self,
        run_id: Uuid,
        call: ToolCall,
        idem: IdempotencyKey,
    ) -> Result<ToolResult, AppError>;
    /// Read-only / trusted built-in? (drives auto-approve).
    fn is_trusted(&self, server: &str) -> bool;
}

/// Request a human decision. The impl decides DURABILITY: chat = live pause;
/// workflow = the durable `elicit` `waiting` gate.
#[async_trait]
pub trait HumanGate: Send + Sync {
    async fn request(&self, run_id: Uuid, ask: GateAsk) -> Result<GateOutcome, AppError>;
}

/// Decide what happens to a tool call BEFORE it runs (SandboxMode × ApprovalMode).
#[async_trait]
pub trait ApprovalPolicy: Send + Sync {
    async fn decide(&self, call: &ToolCall, trusted: bool, sandbox: &SandboxMode) -> Decision;
}

/// Durable per-run agent task list (Group G / ITEM-35 / DEC-50) — sibling to
/// [`TranscriptStore`]. The crate defines the port; the server owns the
/// DB-backed impl (a table mirroring `assistant_core_memory`, FK-cascaded).
/// **Keyed by `run_id`** — chat keys by branch_id, the workflow-agent step by
/// run_id, and each fan-out child gets a FRESH `run_id`, so every agent /
/// sub-agent has its OWN run-scoped list and the parent never reads a child's
/// (ITEM-37 / DEC-53 — structural isolation, no rollup). The store is the
/// SOURCE OF TRUTH for the list (DEC-52): the re-injection extension re-renders
/// from here, not from the transcript, which is what makes "survive compaction"
/// trivially true.
#[async_trait]
pub trait TaskListStore: Send + Sync {
    /// The full current list for a run (empty when none) — the `TaskList`
    /// read-back and the re-injection source.
    async fn load(&self, run_id: Uuid) -> Result<Vec<TaskItem>, AppError>;
    /// Create one item (`TaskCreate`), returning it with its assigned `id`.
    async fn create(&self, run_id: Uuid, item: TaskItemCreate) -> Result<TaskItem, AppError>;
    /// Patch an existing item by id (`TaskUpdate`) — only supplied fields change.
    /// A missing id is an error (surfaced to the model as an `is_error` result).
    async fn update(
        &self,
        run_id: Uuid,
        item_id: Uuid,
        patch: TaskItemPatch,
    ) -> Result<TaskItem, AppError>;
    /// Read back one item by id (`TaskGet`); `None` when absent.
    async fn get(&self, run_id: Uuid, item_id: Uuid) -> Result<Option<TaskItem>, AppError>;
}

/// Resolve a `model_id` → a `Provider` under the user's RBAC (DEC-16). Lets
/// `fan_out`/reviewer mint a per-child/reviewer provider without the crate
/// touching the DB. Direct analog of `ziee-framework`'s `IdentityResolver`.
#[async_trait]
pub trait ModelResolver: Send + Sync {
    async fn resolve(&self, model_id: Uuid, user_id: Uuid) -> Result<Arc<Provider>, AppError>;
}

//! # agent-core — ziee's shared agent loop primitive
//!
//! A ziee-only crate (NOT an SDK crate): the agent loop + six ports +
//! compaction / fan-out / reviewer, hosted app-side by chat, the workflow
//! `kind: agent` step, and parallel fan-out. Built ON the SDK (deps
//! `ziee-core` for `AppError`, `ziee-identity` for permissions) + `ai-providers`.
//!
//! The loop is generic over injected `Arc<P>` ports — the same pattern
//! `ziee-framework`'s `RequirePermissions<R: IdentityResolver>` uses. Design:
//! `.lifecycle/agent-core/DESIGN_REFERENCE.md`.
//!
//! Stage 1: the pure foundation — types, ports, budget, token estimation, the
//! approval matrix, and the extension pipeline. Stage 2 (this module set): the
//! loop driver ([`AgentCore`]), the `ModelClient` seam, compaction, fan-out,
//! and the reviewer — all unit-tested against in-memory fakes.

pub mod budget;
pub mod compaction;
pub mod core;
pub mod core_tools;
pub mod extension;
pub mod fanout;
pub mod guard;
pub mod policy;
pub mod ports;
pub mod reviewer;
pub mod tasklist;
pub mod tokens;
pub mod types;

#[cfg(test)]
mod test_fakes;

pub use budget::Budget;
pub use compaction::{CompactionExtension, CompactionResult, Compactor};
pub use core::{
    AgentCore, CancelToken, DeltaSink, ModelClient, ModelClientFactory, NoopDeltaSink,
    ProviderModelClient, ProviderModelClientFactory,
};
pub use core_tools::{
    core_tool_defs, prepare_child_specs, CoreTool, DelegateChildSpec, DelegateInput,
    DelegateToolScope, DELEGATE_TOOL,
};
pub use extension::{sorted_extensions, AgentExtension, Flow, TurnContext};
pub use guard::neutralize_untrusted;
pub use policy::TrustedAutoApprovePolicy;
pub use ports::{
    ApprovalPolicy, EventSink, HumanGate, ModelResolver, TaskListStore, ToolProvider,
    TranscriptStore,
};
pub use reviewer::{
    map_risk, ModelRiskClassifier, Reviewer, Risk, RiskClassifier, RiskThresholds,
};
pub use tasklist::{
    task_tool_defs, TaskListExtension, TASK_CREATE_TOOL, TASK_GET_TOOL, TASK_LIST_ORDER,
    TASK_LIST_TOOL, TASK_UPDATE_TOOL,
};
pub use types::{
    AgentEvent, AgentTurnRequest, ApprovalMode, Decision, GateAsk, GateOutcome, GateTicket,
    IdempotencyKey, ReviewDecision, SandboxMode, StopReason, SubagentLimits, SubagentSpec,
    SubagentSummary, TaskItem, TaskItemCreate, TaskItemPatch, TaskStatus, ToolCall,
    ToolCallRecord, ToolResult, ToolScope, TurnSeed, Usage,
};

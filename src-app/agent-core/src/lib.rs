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
//! Stage 1 (this module set): the pure foundation — types, ports, budget,
//! token estimation, the approval matrix, and the extension pipeline, all
//! unit-tested against in-memory fakes. The loop driver, compaction, fan-out,
//! and reviewer land in stage 2.

pub mod budget;
pub mod extension;
pub mod policy;
pub mod ports;
pub mod tokens;
pub mod types;

pub use budget::Budget;
pub use extension::{AgentExtension, Flow, TurnContext};
pub use policy::TrustedAutoApprovePolicy;
pub use ports::{
    ApprovalPolicy, EventSink, HumanGate, ModelResolver, ToolProvider, TranscriptStore,
};
pub use types::{
    AgentEvent, AgentTurnRequest, ApprovalMode, Decision, GateAsk, GateOutcome, GateTicket,
    IdempotencyKey, ReviewDecision, SandboxMode, StopReason, SubagentLimits, SubagentSpec,
    SubagentSummary, ToolCall, ToolCallRecord, ToolResult, ToolScope, TurnSeed, Usage,
};

//! The workflow `kind: agent` step host (ITEM-18..23).
//!
//! Wires the shared [`agent_core::AgentCore`] loop into the workflow runner as a
//! new [`StepDispatcher`]. The crate stays domain-free behind its six ports; this
//! module supplies the concrete WORKFLOW-flavored port impls:
//!
//! - [`McpToolProvider`] — enumerate + call MCP tools via the shared
//!   `dispatch::call_mcp_tool` path (`enforce_conversation_disabled = true`);
//!   `is_trusted` = built-in server.
//! - [`WorkflowEventSink`] — map each [`agent_core::AgentEvent`] to a live
//!   `SSEWorkflowRunEvent::StepProgress` track via the `ProgressEmitter`.
//! - [`WorkflowTranscriptStore`] — durable transcript on
//!   `workflow_runs.agent_transcript_json` (DEC-8); tool-call journaling reuses
//!   the `mcp_tool_calls` chokepoint inside `McpSession::call_tool`.
//! - [`WorkflowHumanGate`] — the durable `elicit` `waiting` gate (DEC-9/13/15),
//!   mirroring `ElicitDispatcher`.
//! - [`WorkflowModelResolver`] — `create_provider_from_model_id` + the model-
//!   access RBAC (DEC-16/B); DENIES an inaccessible model.
//!
//! [`AgentDispatcher`] assembles an `AgentCore` from these ports (+ the core
//! `CompactionExtension`), runs it, folds tokens into `ctx.total_tokens`, honors
//! the per-step token cap, writes the final answer via `file_io::write_step_output`,
//! and maps the loop's stop reason to a [`StepResult`].

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use agent_core::{
    AgentCore, AgentEvent, AgentTurnRequest, ApprovalMode, ApprovalPolicy, Budget, CancelToken,
    CompactionExtension, Compactor, Decision, EventSink, GateAsk, GateOutcome, GateTicket,
    HumanGate, IdempotencyKey, ModelClient, ModelClientFactory, ModelResolver,
    ModelRiskClassifier, ProviderModelClient, ProviderModelClientFactory, Reviewer, Risk,
    RiskClassifier, SandboxMode, StopReason, SubagentLimits, ToolCall, ToolProvider, ToolResult,
    ToolScope, TranscriptStore, TrustedAutoApprovePolicy, TurnSeed,
};
use ai_providers::{ChatMessage, ContentBlock, Provider, Role, Tool};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
// Shared MCP tool-call chokepoint now lives in `mcp::agent_tool_call` (§9 DAG:
// shared infra, imported from `mcp/` by both this host and the chat host).
use crate::modules::mcp::agent_tool_call::{
    builtin_server_id_by_name, call_mcp_tool, mcp_to_agent_result, split_tool_name, McpCallScope,
    McpToolCallError,
};
use crate::modules::workflow::dispatch::{resolve_prompt, StepDispatcher};
use crate::modules::workflow::events::{
    ProgressEmitter, ProgressKind, ProgressTrack, SSEElicitationRequiredData, SSEStepProgressData,
    SSEWorkflowRunEvent,
};
use crate::modules::workflow::file_io;
use crate::modules::workflow::models::WorkflowRunStatus;
use crate::modules::workflow::registry;
use crate::modules::workflow::repository;
use crate::modules::workflow::types::{ParsedAs, RunContext, StepKindTag, StepResult};
use crate::modules::workflow::validate::{OutputFormat, StepConfig, StepDef};

/// Window-relative soft limit (tokens) above which the core compaction extension
/// fires. Deliberately high so v1 agent steps rarely summarize (the per-step
/// token cap is the real ceiling); the machinery is wired regardless (ITEM-6).
const AGENT_COMPACTION_SOFT_LIMIT_TOKENS: usize = 100_000;

// ============================================================
// Approved-for-session allow-rules (ITEM-13 / DEC-2)
// ============================================================

/// Process-global `ApprovedForSession` allow-rules, scoped by conversation (or
/// run, when standalone). A rule is the namespaced tool name `"<server>__<tool>"`.
/// When a human approves a mutating/external call "for the session" via the
/// durable review gate, the rule is recorded here; [`ConversationApprovalPolicy`]
/// consults it so the NEXT matching call auto-approves without re-prompting (no
/// silent escalation — the first call still went through the gate).
static APPROVED_FOR_SESSION: OnceLock<Mutex<HashMap<Uuid, HashSet<String>>>> = OnceLock::new();

fn approvals() -> &'static Mutex<HashMap<Uuid, HashSet<String>>> {
    APPROVED_FOR_SESSION.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The allow-rule key for a call — the crate emits `call.name` already namespaced
/// (`"<server>__<tool>"`), so it is the rule verbatim.
fn approval_rule(call: &ToolCall) -> String {
    call.name.clone()
}

fn mark_approved_for_session(scope: Uuid, rule: &str) {
    if let Ok(mut g) = approvals().lock() {
        g.entry(scope).or_default().insert(rule.to_string());
    }
}

fn is_approved_for_session(scope: Uuid, rule: &str) -> bool {
    approvals()
        .lock()
        .map(|g| g.get(&scope).map(|s| s.contains(rule)).unwrap_or(false))
        .unwrap_or(false)
}

/// Wraps the crate's `TrustedAutoApprovePolicy` and short-circuits to `Auto` for
/// any call whose rule the human already approved-for-session in this scope.
struct ConversationApprovalPolicy {
    inner: TrustedAutoApprovePolicy,
    /// Conversation id (or run id, standalone) — the allow-rule scope key.
    scope: Uuid,
}

#[async_trait]
impl ApprovalPolicy for ConversationApprovalPolicy {
    async fn decide(&self, call: &ToolCall, trusted: bool, sandbox: &SandboxMode) -> Decision {
        if is_approved_for_session(self.scope, &approval_rule(call)) {
            return Decision::Auto;
        }
        self.inner.decide(call, trusted, sandbox).await
    }
}

// ============================================================
// Recording reviewer classifier (ITEM-12 / DEC-12)
// ============================================================

/// Delegates to the crate's `ModelRiskClassifier` and, on success, records the
/// resulting class (`low`/`high`/`critical`) keyed by the call's id so the
/// `McpToolProvider` can stamp it onto the `mcp_tool_calls` journal row when the
/// call executes. Fail-closed is preserved — an inner error propagates unchanged
/// (the crate's `Reviewer` maps it to `Deny`) and nothing is recorded.
struct RecordingRiskClassifier {
    /// The wrapped classifier — normally the crate's `ModelRiskClassifier`, or a
    /// deterministic `ForcedRiskClassifier` under the debug-only test seam below.
    inner: Arc<dyn RiskClassifier>,
    /// call.id → classification label; shared with the `McpToolProvider`.
    map: Arc<Mutex<HashMap<String, String>>>,
}

/// **Debug-only test seam.** A deterministic `RiskClassifier` that returns a
/// fixed `Risk` without a model call, so the reviewer-escalation + durable-gate
/// resume paths can be tested without depending on a model actually classifying a
/// call `High`. Constructed ONLY under `cfg!(debug_assertions)` when
/// `ZIEE_AGENT_FORCE_RISK` is set (see the reviewer build site); it is physically
/// unreachable in a release build. Mirrors the `CODE_SANDBOX_ROOTFS_MIRROR` /
/// `LLM_RUNTIME_*_MIRROR` debug-seam pattern.
struct ForcedRiskClassifier {
    risk: Risk,
}

#[async_trait]
impl RiskClassifier for ForcedRiskClassifier {
    async fn classify(&self, _call: &ToolCall, _policy: &str) -> Result<Risk, AppError> {
        Ok(self.risk)
    }
}

/// Parse `ZIEE_AGENT_FORCE_RISK` → a forced classifier, ONLY in a debug build.
/// Returns `None` in release (the env var is ignored) or when unset/unrecognized.
fn forced_risk_classifier() -> Option<Arc<dyn RiskClassifier>> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let risk = match std::env::var("ZIEE_AGENT_FORCE_RISK").ok()?.as_str() {
        "low" => Risk::Low,
        "high" => Risk::High,
        "critical" => Risk::Critical,
        _ => return None,
    };
    Some(Arc::new(ForcedRiskClassifier { risk }))
}

#[async_trait]
impl RiskClassifier for RecordingRiskClassifier {
    async fn classify(&self, call: &ToolCall, policy: &str) -> Result<Risk, AppError> {
        let risk = self.inner.classify(call, policy).await?;
        let label = match risk {
            Risk::Low => "low",
            Risk::High => "high",
            Risk::Critical => "critical",
        };
        if let Ok(mut g) = self.map.lock() {
            g.insert(call.id.clone(), label.to_string());
        }
        Ok(risk)
    }
}

// ============================================================
// Tool provider (ITEM-20)
// ============================================================

/// The agent's tool surface — the step's `servers` allow-list, resolved to MCP
/// tools and routed through the shared `call_mcp_tool` path.
struct McpToolProvider {
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    /// The run's cancel handle (threaded into each MCP call's `tokio::select`).
    cancel: Arc<registry::RunHandle>,
    /// ITEM-12: call.id → reviewer classification, populated by the
    /// [`RecordingRiskClassifier`]; stamped onto the journal row on execution.
    classifications: Arc<Mutex<HashMap<String, String>>>,
}

// `split_tool_name` + `mcp_to_agent_result` now live in the shared
// `mcp::agent_tool_call` (de-duplicated with the chat host; the workflow host's
// previous `terminal: false` hardcode — a latent bug that ignored the
// audience-terminal signal — is reconciled there onto the audience-computed value).

#[async_trait]
impl ToolProvider for McpToolProvider {
    async fn list(&self, scope: &ToolScope) -> Result<Vec<Tool>, AppError> {
        let manager = crate::modules::mcp::client::manager::global()
            .ok_or_else(|| AppError::internal_error("MCP session manager not initialized"))?;
        let mut tools = Vec::new();
        for server_name in &scope.servers {
            // A server the user can't reach (or that fails to list) contributes
            // no tools rather than failing the whole turn.
            let server_id =
                match crate::modules::workflow::dispatch::resolve_tool_server(self.user_id, server_name)
                    .await
                {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::warn!("agent: server '{server_name}' not accessible: {e}");
                        continue;
                    }
                };
            let session = match manager
                .get_or_create_with_context(
                    server_id,
                    self.user_id,
                    self.conversation_id,
                    None,
                    None,
                    None,
                    crate::modules::mcp::tool_calls::models::McpToolCallSource::Workflow,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("agent: open session for '{server_name}': {e}");
                    continue;
                }
            };
            let listed = {
                let mut guard = session.write().await;
                guard.list_tools().await
            };
            match listed {
                Ok(list) => {
                    for t in list {
                        // Convert the MCP tool descriptor → `ai_providers::Tool`,
                        // namespacing the name by server NAME so `call`/`is_trusted`
                        // can route back (the crate sets `ToolCall.server = None`).
                        let name = format!("{server_name}__{}", t.name);
                        tools.push(Tool::function(
                            name,
                            t.description.unwrap_or_default(),
                            t.input_schema,
                        ));
                    }
                }
                Err(e) => tracing::warn!("agent: list tools for '{server_name}': {e}"),
            }
        }
        Ok(tools)
    }

    async fn call(
        &self,
        run_id: Uuid,
        call: ToolCall,
        idem: IdempotencyKey,
    ) -> Result<ToolResult, AppError> {
        let (server_name, tool_name) = split_tool_name(&call.name);
        let scope = McpCallScope {
            user_id: self.user_id,
            conversation_id: self.conversation_id,
            run_id,
        };
        // ITEM-12: if the reviewer classified this call, stamp the class onto the
        // `mcp_tool_calls` journal row (DEC-12).
        let classification = self
            .classifications
            .lock()
            .ok()
            .and_then(|g| g.get(&call.id).cloned());
        // ITEM-16: the stable `<run_id>:<turn>:<ordinal>` idempotency key rides
        // with the call so an in-flight side-effecting call is identifiable on
        // resume (best-effort — carried through the session context).
        match call_mcp_tool(
            &scope,
            &server_name,
            &tool_name,
            call.input,
            true,
            self.cancel.as_ref(),
            None, // chat_ctx — workflow agent host has no chat sampling/journal context
            classification,
            Some(idem),
            crate::modules::mcp::tool_calls::models::McpToolCallSource::Workflow,
        )
        .await
        {
            Ok((_server_id, result)) => Ok(mcp_to_agent_result(result)),
            Err(McpToolCallError::Cancelled) => {
                Err(AppError::internal_error("agent: tool call cancelled"))
            }
            Err(McpToolCallError::Failed(m)) => Err(AppError::internal_error(m)),
        }
    }

    fn is_trusted(&self, server: &str) -> bool {
        // The loop passes `call.server.unwrap_or(call.name)`; since the crate sets
        // `server = None`, `server` is the namespaced tool name — parse its prefix.
        //
        // SECURITY: resolve the NAME to a server_id and gate on the READ-ONLY
        // approval-bypass set (`is_builtin_server_id`), which deliberately EXCLUDES
        // the mutating built-in `code_sandbox` — it MUST go through the reviewer/
        // human gate. `builtin_server_id_by_name` MAPS `code_sandbox` (to its id),
        // so a bare name-match would auto-approve sandbox code execution in a
        // `kind: agent` step; round-tripping name→id→`is_builtin_server_id` gates it
        // (parity with the chat twin's guard in `resolver.rs::is_trusted`).
        // Conservative-by-omission: a read-only built-in NOT in `builtin_server_id_by_name`
        // resolves to None → untrusted → routed through review (safe; pre-existing).
        let (server_name, _) = split_tool_name(server);
        match builtin_server_id_by_name(&server_name) {
            Some(id) => crate::modules::mcp::chat_extension::mcp::is_builtin_server_id(id),
            None => false,
        }
    }
}

// ============================================================
// Event sink (ITEM-20)
// ============================================================

/// Maps the loop's coarse `AgentEvent` stream to live `StepProgress` log tracks.
struct WorkflowEventSink {
    emit: Arc<dyn ProgressEmitter>,
    run_id: Uuid,
    step_id: String,
}

impl WorkflowEventSink {
    fn push_line(&self, line: String) {
        self.emit.emit(SSEWorkflowRunEvent::StepProgress(SSEStepProgressData {
            run_id: self.run_id,
            step_id: self.step_id.clone(),
            tracks: vec![ProgressTrack {
                id: "agent".to_string(),
                label: Some("Agent".to_string()),
                done: false,
                kind: ProgressKind::Log { line },
            }],
        }));
    }
}

#[async_trait]
impl EventSink for WorkflowEventSink {
    async fn emit(&self, ev: AgentEvent) {
        match ev {
            AgentEvent::Message(msg) => {
                // Surface tool requests + a short assistant-text preview.
                for b in &msg.content {
                    if let ContentBlock::ToolUse { name, .. } = b {
                        self.push_line(format!("→ tool: {name}"));
                    }
                }
                if msg.role == Role::Assistant {
                    let text: String = msg
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if !text.is_empty() {
                        self.push_line(text.chars().take(200).collect::<String>());
                    }
                }
            }
            AgentEvent::ToolNotification { server, note } => {
                self.push_line(format!("{server}: {note}"));
            }
            AgentEvent::HistoryReplaced { summary_upto } => {
                self.push_line(format!("context compacted ({summary_upto} messages summarized)"));
            }
            // ContentDelta is the chat host's live token stream; the workflow
            // host surfaces only the finalized `Message`, so it's ignored here.
            AgentEvent::ContentDelta(_) => {}
            // Usage / GateOpened / Stopped are handled by the dispatcher's
            // result-folding + the gate's own ElicitationRequired emit.
            AgentEvent::Usage(_) | AgentEvent::GateOpened(_) | AgentEvent::Stopped(_) => {}
        }
    }
}

// ============================================================
// Transcript store (ITEM-20, DEC-8)
// ============================================================

/// Durable transcript on `workflow_runs.agent_transcript_json` (whole-array
/// read-modify-write; a single run's steps are sequential so no concurrency).
struct WorkflowTranscriptStore {
    pool: PgPool,
}

impl WorkflowTranscriptStore {
    async fn read(&self, run_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        match repository::get_agent_transcript(&self.pool, run_id).await? {
            Some(v) => serde_json::from_value(v)
                .map_err(|e| AppError::internal_error(format!("agent transcript decode: {e}"))),
            None => Ok(Vec::new()),
        }
    }

    async fn write(&self, run_id: Uuid, msgs: &[ChatMessage]) -> Result<(), AppError> {
        let v = serde_json::to_value(msgs)
            .map_err(|e| AppError::internal_error(format!("agent transcript encode: {e}")))?;
        repository::set_agent_transcript(&self.pool, run_id, v).await
    }
}

#[async_trait]
impl TranscriptStore for WorkflowTranscriptStore {
    async fn load(&self, run_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        self.read(run_id).await
    }

    async fn append(&self, run_id: Uuid, msg: ChatMessage) -> Result<(), AppError> {
        let mut msgs = self.read(run_id).await?;
        msgs.push(msg);
        self.write(run_id, &msgs).await
    }

    async fn replace_head(
        &self,
        run_id: Uuid,
        summary: ChatMessage,
        upto: usize,
    ) -> Result<(), AppError> {
        let msgs = self.read(run_id).await?;
        let upto = upto.min(msgs.len());
        let mut new_msgs = Vec::with_capacity(msgs.len() - upto + 1);
        new_msgs.push(summary);
        new_msgs.extend_from_slice(&msgs[upto..]);
        self.write(run_id, &new_msgs).await
    }

    async fn journal_tool_call(
        &self,
        _run_id: Uuid,
        _rec: agent_core::ToolCallRecord,
    ) -> Result<(), AppError> {
        // The `mcp_tool_calls` journal row is already written by the recording
        // chokepoint inside `McpSession::call_tool` (one row per invocation), so
        // an extra insert here would double-record. The transcript itself (which
        // carries the tool_result message via `append`) is the resume source.
        Ok(())
    }

    async fn completed_tool_calls(
        &self,
        _run_id: Uuid,
    ) -> Result<Vec<agent_core::ToolCallRecord>, AppError> {
        // Idempotent resume-replay by key (ITEM-16) is a later durability stage;
        // the base loop never consults this (it replays via the transcript).
        Ok(Vec::new())
    }
}

// ============================================================
// Human gate (ITEM-20, DEC-9/13/15)
// ============================================================

/// The durable review gate — persists a pending `elicit` record, parks the run
/// as `waiting`, and returns `Suspended` (mirrors `ElicitDispatcher`'s durable
/// path). Resumes when the human submits (`submit_elicit` → `resume_run`).
struct WorkflowHumanGate {
    pool: PgPool,
    emit: Arc<dyn ProgressEmitter>,
    step_id: String,
    /// ITEM-12: call.id → classification, shared with the tool provider. The
    /// reviewer classifies during the pre-park turn, but the tool only EXECUTES on
    /// resume (a fresh invocation with an empty map where the reviewer is skipped
    /// via the session allow-rule) — so the class must be persisted into the
    /// durable gate record here and re-seeded on resume, or it never reaches the
    /// `mcp_tool_calls` journal row.
    classifications: Arc<Mutex<HashMap<String, String>>>,
}

#[async_trait]
impl HumanGate for WorkflowHumanGate {
    async fn request(&self, run_id: Uuid, ask: GateAsk) -> Result<GateOutcome, AppError> {
        let elicitation_id = Uuid::new_v4();
        let message = format!(
            "The agent wants to run tool `{}`. {} Approve?",
            ask.call.name, ask.reason
        );
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "approve": { "type": "boolean", "title": "Approve this tool call" },
                "approve_for_session": {
                    "type": "boolean",
                    "title": "Approve this tool for the rest of the session",
                    "default": false
                }
            },
            "required": ["approve"]
        });
        // ITEM-12: carry the reviewer's classification + the call id through the
        // durable gate so the resumed loop can stamp it onto the journal row (the
        // reviewer is not re-consulted on resume). `null` when the reviewer is off.
        let classification = self
            .classifications
            .lock()
            .ok()
            .and_then(|g| g.get(&ask.call.id).cloned());
        let data = serde_json::json!({
            "tool": ask.call.name,
            "arguments": ask.call.input,
            "call_id": ask.call.id,
            "classification": classification,
        });
        // Far-future deadline: a durable, human-paced review (matches the
        // `timeout_ms: 0` elicit gate).
        let deadline = Utc::now() + chrono::Duration::days(365 * 100);

        let record = crate::modules::workflow::types::PendingElicitationRecord {
            run_id,
            elicitation_id,
            step_id: self.step_id.clone(),
            message: message.clone(),
            schema: schema.clone(),
            data: Some(data.clone()),
            deadline_at: deadline,
        };
        let json = serde_json::to_value(&record)
            .map_err(|e| AppError::internal_error(format!("serialize agent gate: {e}")))?;
        repository::set_pending_elicitation(&self.pool, run_id, Some(json)).await?;
        repository::mark_status(&self.pool, run_id, WorkflowRunStatus::Waiting, None).await?;

        self.emit.emit(SSEWorkflowRunEvent::ElicitationRequired(
            SSEElicitationRequiredData {
                run_id,
                step_id: self.step_id.clone(),
                elicitation_id,
                message,
                schema,
                data: Some(data),
                deadline_at: deadline,
            },
        ));

        Ok(GateOutcome::Suspended(GateTicket { id: elicitation_id }))
    }
}

// ============================================================
// Model resolver (ITEM-20, DEC-16/B)
// ============================================================

/// Resolves a per-child / reviewer `model_id` to a `Provider` under the user's
/// RBAC — `create_provider_from_model_id` + the model-access check. DENIES an
/// inaccessible model (the boundary the crate never crosses on its own).
struct WorkflowModelResolver;

#[async_trait]
impl ModelResolver for WorkflowModelResolver {
    async fn resolve(&self, model_id: Uuid, user_id: Uuid) -> Result<Arc<Provider>, AppError> {
        use crate::core::Repos;
        let model = Repos
            .llm_model
            .get_by_id(model_id)
            .await?
            .ok_or_else(|| AppError::not_found("Model"))?;
        if !model.enabled {
            return Err(AppError::bad_request(
                "MODEL_DISABLED",
                "this model is currently disabled and cannot be used",
            ));
        }
        let has_access = Repos
            .user_group_llm_provider
            .user_has_access_to_provider(user_id, model.provider_id)
            .await
            .map_err(AppError::from)?;
        if !has_access {
            return Err(AppError::forbidden(
                "ACCESS_DENIED",
                "you do not have access to this model",
            ));
        }
        let (provider, ..) =
            crate::modules::chat::core::ai_provider::create_provider_from_model_id(model_id, user_id)
                .await?;
        Ok(provider)
    }
}

// ============================================================
// The dispatcher (ITEM-19/23)
// ============================================================

pub struct AgentDispatcher {
    provider: Arc<Provider>,
}

impl AgentDispatcher {
    pub fn new(provider: Arc<Provider>) -> Self {
        Self { provider }
    }
}

/// Map an admin `default_sandbox_mode` string to the crate enum (DEC-2 metadata).
fn sandbox_mode_from_str(s: &str) -> SandboxMode {
    match s {
        "read-only" => SandboxMode::ReadOnly { network: false },
        "danger-full-access" => SandboxMode::DangerFullAccess,
        // "workspace-write" + any unknown → the sensible default.
        _ => SandboxMode::WorkspaceWrite { network: true },
    }
}

/// Map an admin `unattended_approval_policy` string to the crate `ApprovalMode`.
fn approval_mode_from_str(s: &str) -> ApprovalMode {
    match s {
        "untrusted" => ApprovalMode::UnlessTrusted,
        "never" => ApprovalMode::Never,
        // "on-request" / "on-failure" → route mutating calls through the gate.
        _ => ApprovalMode::OnRequest,
    }
}

#[async_trait]
impl StepDispatcher for AgentDispatcher {
    async fn dispatch(
        &self,
        step: &StepDef,
        ctx: &mut RunContext,
        cancel: Arc<registry::RunHandle>,
        emit: Arc<dyn ProgressEmitter>,
    ) -> StepResult {
        let started = Instant::now();

        let (prompt, prompt_file, system, servers, max_steps, output_format) = match &step.config {
            StepConfig::Agent {
                prompt,
                prompt_file,
                system,
                servers,
                max_steps,
                output_format,
            } => (
                prompt.clone(),
                prompt_file.clone(),
                system.clone(),
                servers.clone(),
                *max_steps,
                *output_format,
            ),
            _ => {
                return StepResult::Failed {
                    error: "AgentDispatcher called on non-agent step".into(),
                    tokens_used: 0,
                };
            }
        };

        // Resolve the initial user task (inline `prompt:` or bundle `prompt_file:`)
        // + the optional system directive, both template-rendered against `ctx`.
        let user_prompt = match resolve_prompt(step, ctx, &prompt, &prompt_file).await {
            Ok(p) => p,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("agent prompt render: {e}"),
                    tokens_used: 0,
                };
            }
        };
        let system_blocks: Vec<ContentBlock> = match system.as_deref() {
            Some(raw) => match crate::modules::workflow::template::render(raw, ctx) {
                Ok(s) => vec![ContentBlock::Text { text: s }],
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("agent system render: {e}"),
                        tokens_used: 0,
                    };
                }
            },
            None => Vec::new(),
        };

        // Admin policy (DEC-6) — approval mode, token caps, sandbox, fan-out
        // limits. Fall back to sane defaults if the row can't be read.
        let settings = crate::core::Repos.agent.get_admin_settings().await.ok();
        let approval_mode = settings
            .as_ref()
            .map(|s| approval_mode_from_str(&s.unattended_approval_policy))
            .unwrap_or(ApprovalMode::OnRequest);
        let sandbox = settings
            .as_ref()
            .map(|s| sandbox_mode_from_str(&s.default_sandbox_mode))
            .unwrap_or(SandboxMode::WorkspaceWrite { network: true });
        let limits = settings
            .as_ref()
            .map(|s| SubagentLimits {
                max_depth: s.fan_out_max_depth.clamp(1, 255) as u8,
                max_threads: s.fan_out_max_threads.clamp(1, 255) as u8,
            })
            .unwrap_or_default();

        // The agent is ONE workflow step, so its whole-run token budget is bounded
        // by the per-STEP cap: it self-stops with `TokenCap` before it can breach
        // the runner's post-step `check_step_caps` (which would fail the run).
        let per_step_cap = crate::modules::workflow::runner::PER_STEP_TOKEN_CAP.min(
            settings
                .as_ref()
                .map(|s| s.per_step_token_cap.max(0) as u64)
                .unwrap_or(crate::modules::workflow::runner::PER_STEP_TOKEN_CAP),
        );
        let budget = Budget::new(max_steps, per_step_cap, per_step_cap);

        // Shared ports.
        let pool = crate::core::Repos.pool().clone();
        let transcript: Arc<dyn TranscriptStore> =
            Arc::new(WorkflowTranscriptStore { pool: pool.clone() });
        let sink: Arc<dyn EventSink> = Arc::new(WorkflowEventSink {
            emit: emit.clone(),
            run_id: ctx.run_id,
            step_id: step.id.clone(),
        });
        let model_client: Arc<dyn ModelClient> =
            Arc::new(ProviderModelClient::new(self.provider.clone()));

        // ITEM-13: allow-rule scope key — the conversation (or run, standalone).
        let approval_scope = ctx.conversation_id.unwrap_or(ctx.run_id);
        // ITEM-12: shared call.id → classification map (reviewer → journal row).
        let classifications: Arc<Mutex<HashMap<String, String>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // ITEM-12 (DEC-3): build the reviewer when the admin enabled it. Its
        // classifier runs on `reviewer_model_id` (nullable → the run's model,
        // resolved under the user's RBAC via `WorkflowModelResolver`), seeded with
        // the admin `reviewer_policy`. Fail-closed is the crate's default.
        let reviewer: Option<Reviewer> = match settings.as_ref() {
            Some(s) if s.reviewer_enabled => {
                let policy = s.reviewer_policy.clone().unwrap_or_default();
                let (rev_client, rev_model_name): (Arc<dyn ModelClient>, String) =
                    match s.reviewer_model_id {
                        Some(mid) => match WorkflowModelResolver.resolve(mid, ctx.user_id).await {
                            Ok(provider) => {
                                let name = crate::core::Repos
                                    .llm_model
                                    .get_by_id(mid)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|m| m.name)
                                    .unwrap_or_else(|| ctx.model_name.clone());
                                (
                                    ProviderModelClientFactory.for_provider(provider),
                                    name,
                                )
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "agent reviewer: model {mid} resolve failed ({e}); \
                                     using the run's model"
                                );
                                (model_client.clone(), ctx.model_name.clone())
                            }
                        },
                        None => (model_client.clone(), ctx.model_name.clone()),
                    };
                // Debug-only seam: a forced classifier makes reviewer escalation
                // deterministic in tests; otherwise the real model classifier.
                let inner: Arc<dyn RiskClassifier> = forced_risk_classifier()
                    .unwrap_or_else(|| Arc::new(ModelRiskClassifier::new(rev_client, rev_model_name)));
                let recording = RecordingRiskClassifier {
                    inner,
                    map: classifications.clone(),
                };
                Some(Reviewer::new(Arc::new(recording), policy))
            }
            _ => None,
        };

        let core = AgentCore {
            transcript: transcript.clone(),
            sink: sink.clone(),
            tools: Arc::new(McpToolProvider {
                user_id: ctx.user_id,
                conversation_id: ctx.conversation_id,
                cancel: cancel.clone(),
                classifications: classifications.clone(),
            }),
            gate: Arc::new(WorkflowHumanGate {
                pool: pool.clone(),
                emit: emit.clone(),
                step_id: step.id.clone(),
                classifications: classifications.clone(),
            }),
            // ITEM-13: consult per-conversation `ApprovedForSession` rules first,
            // else fall back to the admin approval matrix.
            policy: Arc::new(ConversationApprovalPolicy {
                inner: TrustedAutoApprovePolicy::new(approval_mode),
                scope: approval_scope,
            }),
            models: Arc::new(WorkflowModelResolver),
            model: model_client.clone(),
            model_factory: Arc::new(ProviderModelClientFactory),
            extensions: vec![Arc::new(CompactionExtension::new(
                Compactor::new(
                    model_client.clone(),
                    ctx.model_name.clone(),
                    AGENT_COMPACTION_SOFT_LIMIT_TOKENS,
                ),
                transcript.clone(),
                sink.clone(),
                ctx.run_id,
            ))],
            // ITEM-12: the reviewer resolves a `Review` outcome; with `None` a
            // `Review` escalates straight to the human gate (safe default).
            reviewer,
            budget,
            limits,
            sandbox,
            model_name: ctx.model_name.clone(),
            resume_executes_pending: true,
        };

        // ITEM-16: resume-replay. A non-empty persisted transcript means this is a
        // crash / gate resume — seed `Resume` (do NOT re-append the user prompt),
        // so the loop continues from the durable transcript without re-calling the
        // tool_results already in it.
        let existing_transcript = repository::get_agent_transcript(&pool, ctx.run_id)
            .await
            .ok()
            .flatten();
        let is_resume = existing_transcript
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);

        // ITEM-13: on a durable review-gate resume, if the human approved the
        // pending call "for the session", record the allow-rule so the resumed
        // loop auto-approves it (consulted by `ConversationApprovalPolicy`).
        if is_resume {
            if let Ok(Some(resp)) = repository::get_elicit_response(&pool, ctx.run_id).await {
                let inner = resp.get("response");
                let approved = inner
                    .and_then(|r| r.get("approve"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let for_session = inner
                    .and_then(|r| r.get("approve_for_session"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if approved && for_session {
                    if let Ok(Some(run_row)) = repository::find_run(&pool, ctx.run_id).await {
                        if let Some(tool) = run_row
                            .pending_elicitation_json
                            .as_ref()
                            .and_then(|p| p.get("data"))
                            .and_then(|d| d.get("tool"))
                            .and_then(|v| v.as_str())
                        {
                            mark_approved_for_session(approval_scope, tool);
                        }
                    }
                }
            }

            // ITEM-12: re-seed the reviewer classification persisted into the gate
            // record so the tool, when it executes on resume, still stamps its
            // class onto the `mcp_tool_calls` journal row (the reviewer is not
            // re-run on resume). Keyed by the SAME call.id the pending tool_use
            // carries in the reloaded transcript.
            if let Ok(Some(run_row)) = repository::find_run(&pool, ctx.run_id).await {
                if let Some(data) = run_row
                    .pending_elicitation_json
                    .as_ref()
                    .and_then(|p| p.get("data"))
                {
                    if let (Some(call_id), Some(class)) = (
                        data.get("call_id").and_then(|v| v.as_str()),
                        data.get("classification").and_then(|v| v.as_str()),
                    ) {
                        if let Ok(mut g) = classifications.lock() {
                            g.insert(call_id.to_string(), class.to_string());
                        }
                    }
                }
            }
        }

        let req = AgentTurnRequest {
            run_id: ctx.run_id,
            user_id: ctx.user_id,
            seed: if is_resume {
                TurnSeed::Resume
            } else {
                TurnSeed::NewMessage(ChatMessage::user(user_prompt))
            },
            system: system_blocks,
            tool_scope: ToolScope {
                servers,
                allow_delegate: false,
            },
            start_iteration: 1,
            inputs: serde_json::Value::Null,
        };

        // ITEM-17: flag the run as inside an agent step so the boot sweep spares +
        // resumes (rather than fails) a crash here. Best-effort.
        let _ = repository::set_resumable_agent(&pool, ctx.run_id, true).await;

        // Bridge the workflow cancel handle into the crate's cooperative token.
        let cancel_token = CancelToken::new();
        let bridge = {
            let ct = cancel_token.clone();
            let h = cancel.clone();
            tokio::spawn(async move {
                h.await_cancel().await;
                ct.cancel();
            })
        };
        let run_result = core.run(req, cancel_token).await;
        bridge.abort();

        let _ = repository::set_resumable_agent(&pool, ctx.run_id, false).await;

        let events = match run_result {
            Ok(ev) => ev,
            Err(e) => {
                return StepResult::Failed {
                    error: format!("agent loop: {e}"),
                    tokens_used: 0,
                };
            }
        };

        // Fold token usage across every model call the loop made.
        let tokens: u64 = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::Usage(u) => Some(u.total_tokens),
                _ => None,
            })
            .sum();

        // A durable gate opened → the run is parked `waiting`; suspend the step.
        if events
            .iter()
            .any(|e| matches!(e, AgentEvent::GateOpened(_)))
        {
            ctx.total_tokens += tokens;
            return StepResult::Suspended;
        }

        // A `Halted` stop with no gate means the run was cancelled.
        let last_stop = events.iter().rev().find_map(|e| match e {
            AgentEvent::Stopped(r) => Some(*r),
            _ => None,
        });
        if last_stop == Some(StopReason::Halted) {
            ctx.total_tokens += tokens;
            return StepResult::Cancelled;
        }

        // The final answer is the loop's last assistant text.
        let final_text = events
            .iter()
            .rev()
            .find_map(|e| match e {
                AgentEvent::Message(msg) if msg.role == Role::Assistant => {
                    let text: String = msg
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if text.is_empty() {
                        None
                    } else {
                        Some(text)
                    }
                }
                _ => None,
            })
            .unwrap_or_default();

        let (value, parsed_as) = match output_format {
            OutputFormat::Json => {
                match crate::modules::workflow::dispatch::parse_llm_output(
                    &final_text,
                    OutputFormat::Json,
                ) {
                    Ok(vp) => vp,
                    Err(error) => {
                        return StepResult::Failed {
                            error,
                            tokens_used: tokens,
                        };
                    }
                }
            }
            OutputFormat::Text => (Value::String(final_text), ParsedAs::Text),
        };

        let meta =
            match file_io::write_step_output(ctx, &step.id, &value, parsed_as, StepKindTag::Agent)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    return StepResult::Failed {
                        error: format!("persist step output: {e}"),
                        tokens_used: tokens,
                    };
                }
            };
        ctx.step_outputs.insert(step.id.clone(), meta);
        ctx.total_tokens += tokens;

        StepResult::Completed {
            output: value,
            parsed_as,
            tokens_used: tokens,
            ms_elapsed: started.elapsed().as_millis() as u64,
        }
    }
}

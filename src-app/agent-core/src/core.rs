//! The agent loop driver (ITEM-4/5) — `AgentCore` + `AgentCore::run`.
//!
//! `AgentCore` owns NO I/O of its own: every side effect goes through one of the
//! injected `Arc<dyn Port>` ports (`crate::ports`) plus the `ModelClient` seam
//! below. `run` drives one turn to a `StopReason`, collecting the coarse
//! `AgentEvent` stream (also pushed to the `EventSink` out-of-band, P10) and
//! journaling every completed tool call (P5).
//!
//! ## The `ModelClient` seam (testability)
//! The loop never touches `ai_providers::Provider` directly — it calls a
//! `ModelClient::call(ChatRequest) -> (ChatMessage, Usage)`. The REAL impl
//! ([`ProviderModelClient`]) wraps `Provider::chat_stream`, accumulating text +
//! `ToolUseDelta`s into an assistant message and reading the trailing usage
//! frame. Unit tests inject a FAKE `ModelClient` (see `crate::test_fakes`), so
//! the whole loop is exercised WITHOUT a real LLM.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ai_providers::{
    ChatMessage, ChatRequest, ContentBlock, ContentBlockDelta, Provider, Role,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use ziee_core::AppError;

use crate::budget::Budget;
use crate::extension::{run_before_model, run_contribute, AgentExtension, Flow, TurnContext};
use crate::ports::{
    ApprovalPolicy, EventSink, HumanGate, ModelResolver, ToolProvider, TranscriptStore,
};
use crate::reviewer::Reviewer;
use crate::types::{
    AgentEvent, AgentTurnRequest, Decision, GateAsk, GateOutcome, GateTicket, ReviewDecision,
    SandboxMode, StopReason, ToolCall, ToolCallRecord, ToolResult, TurnSeed, Usage,
};

/// A cooperative cancellation flag (kept dep-minimal — no `tokio-util`). Cloned
/// cheaply; a `cancel()` on any clone is observed by all via `is_cancelled()`.
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal cancellation. The loop stops at its next checkpoint with `Halted`;
    /// an in-flight model call awaiting [`CancelToken::cancelled`] aborts promptly.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// Resolve when cancellation is requested (or immediately if already set) —
    /// lets the loop race an in-flight model stream against cancel so a mid-stream
    /// stop aborts the turn instead of waiting for the whole response.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

/// A sink for live streaming deltas (ITEM-26). The chat host forwards each delta
/// to SSE as an `SSEChatStreamEvent::Content` frame; non-streaming hosts use the
/// [`NoopDeltaSink`].
#[async_trait]
pub trait DeltaSink: Send + Sync {
    async fn on_delta(&self, delta: &ContentBlockDelta);
}

/// A no-op delta sink — for non-streaming callers and fake models.
pub struct NoopDeltaSink;
#[async_trait]
impl DeltaSink for NoopDeltaSink {
    async fn on_delta(&self, _delta: &ContentBlockDelta) {}
}

/// Adapter: forwards each streamed delta to the loop's [`EventSink`] as an
/// [`AgentEvent::ContentDelta`], so a host (chat) can stream tokens live.
struct EventDeltaSink {
    sink: Arc<dyn EventSink>,
}
#[async_trait]
impl DeltaSink for EventDeltaSink {
    async fn on_delta(&self, delta: &ContentBlockDelta) {
        self.sink
            .emit(AgentEvent::ContentDelta(delta.clone()))
            .await;
    }
}

/// One model call → an assistant `ChatMessage` + `Usage`. The seam that makes
/// the loop unit-testable without a real LLM (see the module docs).
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn call(&self, req: ChatRequest) -> Result<(ChatMessage, Usage), AppError>;

    /// Streaming variant (ITEM-26): forwards each `ContentBlockDelta` to `sink`
    /// as it arrives, then returns the accumulated assistant message. The DEFAULT
    /// is non-streaming (delegates to [`ModelClient::call`], ignoring the sink) so
    /// fake models need not implement it; the real [`ProviderModelClient`]
    /// overrides it to stream live tokens.
    async fn call_streaming(
        &self,
        req: ChatRequest,
        _sink: &dyn DeltaSink,
    ) -> Result<(ChatMessage, Usage), AppError> {
        self.call(req).await
    }
}

/// The REAL `ModelClient` — wraps a resolved `ai_providers::Provider`, draining
/// `chat_stream` into a single assistant message (accumulating `TextDelta` +
/// `ToolUseDelta`) and reading the trailing usage frame.
pub struct ProviderModelClient {
    provider: Arc<Provider>,
}

impl ProviderModelClient {
    pub fn new(provider: Arc<Provider>) -> Self {
        Self { provider }
    }
}

#[derive(Default)]
struct ToolAcc {
    id: String,
    name: String,
    input: String,
}

#[async_trait]
impl ModelClient for ProviderModelClient {
    async fn call(&self, req: ChatRequest) -> Result<(ChatMessage, Usage), AppError> {
        self.call_streaming(req, &NoopDeltaSink).await
    }

    async fn call_streaming(
        &self,
        req: ChatRequest,
        sink: &dyn DeltaSink,
    ) -> Result<(ChatMessage, Usage), AppError> {
        let mut stream = self
            .provider
            .chat_stream(req)
            .await
            .map_err(|e| AppError::internal_error(format!("chat_stream failed: {e}")))?;

        let mut text = String::new();
        let mut tools: BTreeMap<usize, ToolAcc> = BTreeMap::new();
        let mut usage = Usage::default();

        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| AppError::internal_error(format!("stream error: {e}")))?;
            for delta in chunk.content {
                // Forward EVERY delta live (text/thinking/tool) so the host can
                // stream tokens; then accumulate for the final message.
                sink.on_delta(&delta).await;
                match delta {
                    ContentBlockDelta::TextDelta { delta, .. } => text.push_str(&delta),
                    ContentBlockDelta::ToolUseDelta {
                        index,
                        id,
                        name,
                        input_delta,
                    } => {
                        let acc = tools.entry(index).or_default();
                        if let Some(id) = id {
                            acc.id = id;
                        }
                        if let Some(name) = name {
                            acc.name = name;
                        }
                        if let Some(d) = input_delta {
                            acc.input.push_str(&d);
                        }
                    }
                    // Thinking / redacted-thinking deltas don't affect the loop's
                    // decision surface (tool extraction) — ignored here.
                    _ => {}
                }
            }
            if let Some(u) = chunk.usage {
                usage = Usage {
                    input_tokens: u.prompt_tokens as u64,
                    output_tokens: u.completion_tokens as u64,
                    total_tokens: u.total_tokens as u64,
                };
            }
        }

        let mut content = Vec::new();
        if !text.is_empty() {
            content.push(ContentBlock::Text { text });
        }
        for (_idx, acc) in tools {
            let input = if acc.input.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&acc.input).unwrap_or_else(|_| serde_json::json!({}))
            };
            content.push(ContentBlock::ToolUse {
                id: acc.id,
                name: acc.name,
                input,
            });
        }

        Ok((ChatMessage::with_blocks(Role::Assistant, content), usage))
    }
}

/// Builds a per-child/reviewer `ModelClient` from a resolved `Provider`. The
/// `fan_out` seam that lets tests substitute a fake model without a network
/// call while still exercising the `ModelResolver` (ITEM-7).
pub trait ModelClientFactory: Send + Sync {
    fn for_provider(&self, provider: Arc<Provider>) -> Arc<dyn ModelClient>;
}

/// The production factory — wraps the provider in a [`ProviderModelClient`].
pub struct ProviderModelClientFactory;

impl ModelClientFactory for ProviderModelClientFactory {
    fn for_provider(&self, provider: Arc<Provider>) -> Arc<dyn ModelClient> {
        Arc::new(ProviderModelClient::new(provider))
    }
}

/// The shared agent core. Constructed by a HOST (chat / workflow-step /
/// subagent-orchestrator) with host-flavored ports, then driven via [`run`].
///
/// [`run`]: AgentCore::run
#[derive(Clone)]
pub struct AgentCore {
    pub transcript: Arc<dyn TranscriptStore>,
    pub sink: Arc<dyn EventSink>,
    pub tools: Arc<dyn ToolProvider>,
    pub gate: Arc<dyn HumanGate>,
    pub policy: Arc<dyn ApprovalPolicy>,
    pub models: Arc<dyn ModelResolver>,
    /// This core's own model client (built from the resolved `Arc<Provider>`).
    pub model: Arc<dyn ModelClient>,
    /// Mints a per-child model client during `fan_out` (ITEM-7).
    pub model_factory: Arc<dyn ModelClientFactory>,
    /// Ordered extension pipeline (incl. the core `CompactionExtension`).
    pub extensions: Vec<Arc<dyn AgentExtension>>,
    /// Optional reviewer resolving a `Decision::Review` (ITEM-12).
    pub reviewer: Option<Reviewer>,
    pub budget: Budget,
    pub limits: crate::types::SubagentLimits,
    pub sandbox: SandboxMode,
    /// Model name written into each `ChatRequest.model`.
    pub model_name: String,
}

/// What to do with one tool call after the approval gate has decided.
enum Act {
    Execute,
    Deny(String),
    Suspend(GateTicket),
}

impl AgentCore {
    async fn push_emit(&self, events: &mut Vec<AgentEvent>, ev: AgentEvent) {
        events.push(ev.clone());
        self.sink.emit(ev).await;
    }

    /// Drive one agent turn to a stop condition, returning the collected coarse
    /// event stream (also pushed to the `EventSink`). Errors from ports surface
    /// as `AppError`; a clean stop is always an `AgentEvent::Stopped(_)` tail
    /// (except the durable-gate suspend, which tails `GateOpened` + `Stopped`).
    pub async fn run(
        &self,
        req: AgentTurnRequest,
        cancel: CancelToken,
    ) -> Result<Vec<AgentEvent>, AppError> {
        let mut events = Vec::new();
        let mut budget = self.budget.clone();
        let mut iteration = req.start_iteration.max(1);

        // Seed a fresh user message into the transcript (a Resume reads what's
        // already persisted).
        if let TurnSeed::NewMessage(msg) = &req.seed {
            self.transcript.append(req.run_id, msg.clone()).await?;
        }

        loop {
            if cancel.is_cancelled() {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::Halted))
                    .await;
                break;
            }
            if let Some(reason) = budget.stop_before(iteration) {
                self.push_emit(&mut events, AgentEvent::Stopped(reason)).await;
                break;
            }

            // Rebuild the per-turn context via the extension `contribute` phase.
            let mut tctx = TurnContext {
                system: req.system.clone(),
                tool_scope: req.tool_scope.clone(),
                inputs: req.inputs.clone(),
                ..Default::default()
            };
            run_contribute(&self.extensions, &mut tctx).await?;

            let history = self.transcript.load(req.run_id).await?;
            let tools = self.tools.list(&tctx.tool_scope).await?;
            let mut chat_req = ChatRequest {
                model: self.model_name.clone(),
                messages: assemble_messages(&tctx.system, &history),
                tools,
                ..Default::default()
            };

            // `before_model` hooks (compaction runs here at a late order; the chat
            // registry bridge flips approval rows on a resume). A veto stops the turn.
            if run_before_model(&self.extensions, &mut chat_req).await? == Flow::ShortCircuit {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::NoToolCall))
                    .await;
                break;
            }

            // Normalize: MERGE every System message into a single system message at
            // the front. The re-homed context extensions each insert their own system
            // prompt (assistant / project / memory / MCP tool guidance), so a turn can
            // carry several; strict providers (vllm/qwen) accept only ONE system
            // message, and it must be first. Concatenating the text is semantically
            // identical (all are turn-level instructions) and valid for every provider.
            if chat_req.messages.iter().filter(|m| m.role == Role::System).count() > 1
                || chat_req
                    .messages
                    .first()
                    .map(|m| m.role != Role::System)
                    .unwrap_or(false)
                    && chat_req.messages.iter().any(|m| m.role == Role::System)
            {
                let mut sys_text: Vec<String> = Vec::new();
                let mut rest: Vec<ChatMessage> = Vec::new();
                for m in std::mem::take(&mut chat_req.messages) {
                    if m.role == Role::System {
                        for b in &m.content {
                            if let ContentBlock::Text { text } = b {
                                if !text.trim().is_empty() {
                                    sys_text.push(text.clone());
                                }
                            }
                        }
                    } else {
                        rest.push(m);
                    }
                }
                let mut merged = Vec::with_capacity(rest.len() + 1);
                if !sys_text.is_empty() {
                    merged.push(ChatMessage::with_blocks(
                        Role::System,
                        vec![ContentBlock::Text {
                            text: sys_text.join("\n\n"),
                        }],
                    ));
                }
                merged.extend(rest);
                chat_req.messages = merged;
            }

            // Resume mid-tool-execution: on the FIRST iteration of a `Resume`, if the
            // transcript already ends with an assistant message carrying unexecuted
            // `tool_use` blocks (a turn suspended awaiting human approval), execute
            // THOSE — do NOT call the model again. A fresh call would re-emit tool
            // requests with new ids, losing the human's decision; the `before_model`
            // hooks above already flipped the approval rows so the policy resolves
            // them. (Domain-neutral: purely a transcript shape, no chat types.)
            let is_first = iteration == req.start_iteration.max(1);
            let resume_msg = if is_first && matches!(req.seed, TurnSeed::Resume) {
                last_pending_assistant(&history)
            } else {
                None
            };
            let from_model = resume_msg.is_none();

            let assistant_msg = match resume_msg {
                Some(msg) => msg,
                None => {
                    // Stream tokens live to the sink as they arrive, then get the
                    // accumulated assistant message + usage (ITEM-26). Race against
                    // cancellation so a mid-stream stop aborts promptly.
                    let delta_sink = EventDeltaSink {
                        sink: self.sink.clone(),
                    };
                    let (assistant_msg, usage) = tokio::select! {
                        r = self.model.call_streaming(chat_req, &delta_sink) => r?,
                        _ = cancel.cancelled() => {
                            self.push_emit(&mut events, AgentEvent::Stopped(StopReason::Halted))
                                .await;
                            break;
                        }
                    };
                    budget.add_tokens(usage.total_tokens);
                    self.transcript
                        .append(req.run_id, assistant_msg.clone())
                        .await?;
                    self.push_emit(&mut events, AgentEvent::Message(assistant_msg.clone()))
                        .await;
                    self.push_emit(&mut events, AgentEvent::Usage(usage)).await;
                    assistant_msg
                }
            };

            // Post-round extension hooks (e.g. background memory extract) run only for
            // a fresh model message — a resume-executed message already had its
            // after-hooks on the turn that produced it. A short-circuit ends the turn.
            let mut short_circuit = false;
            if from_model {
                for ext in &self.extensions {
                    if ext.after_round(&assistant_msg).await? == Flow::ShortCircuit {
                        short_circuit = true;
                        break;
                    }
                }
            }
            if short_circuit {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::NoToolCall))
                    .await;
                break;
            }

            let tool_calls = extract_tool_calls(&assistant_msg);
            if tool_calls.is_empty() {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::NoToolCall))
                    .await;
                break;
            }

            // The model requested tools. Decide whether we may run ANOTHER round;
            // if not, synthesize `is_error` results for the unexecuted calls so
            // the transcript never carries an orphan `ToolUse` (ITEM-5).
            let stop_reason = if cancel.is_cancelled() {
                Some(StopReason::Halted)
            } else if budget.run_tokens() > budget.per_run_token_cap {
                Some(StopReason::TokenCap)
            } else if iteration >= budget.max_steps {
                Some(StopReason::IterationCap)
            } else {
                None
            };
            if let Some(reason) = stop_reason {
                for call in &tool_calls {
                    let result = error_tool_result(
                        format!("tool not executed: agent stopped ({reason:?})"),
                    );
                    let msg = tool_result_message(call, &result);
                    self.transcript.append(req.run_id, msg.clone()).await?;
                    self.push_emit(&mut events, AgentEvent::Message(msg)).await;
                }
                self.push_emit(&mut events, AgentEvent::Stopped(reason)).await;
                break;
            }

            // Execute each requested tool through the approval gate.
            let mut suspended = false;
            let mut executed = 0usize;
            let mut terminal_count = 0usize;
            for (ordinal, call) in tool_calls.iter().enumerate() {
                let server_key = call
                    .server
                    .clone()
                    .unwrap_or_else(|| call.name.clone());
                let trusted = self.tools.is_trusted(&server_key);

                let mut decision = self.policy.decide(call, trusted, &self.sandbox).await;
                if decision == Decision::Review {
                    decision = match &self.reviewer {
                        Some(rev) => rev.review(call).await,
                        // No reviewer wired → escalate to a human (safe default).
                        None => Decision::Prompt,
                    };
                }

                let act = match decision {
                    Decision::Auto => Act::Execute,
                    Decision::Prompt | Decision::Review => {
                        let ask = GateAsk {
                            call: call.clone(),
                            reason: "tool call requires approval".to_string(),
                        };
                        match self.gate.request(req.run_id, ask).await? {
                            GateOutcome::Decided(ReviewDecision::Approved)
                            | GateOutcome::Decided(ReviewDecision::ApprovedForSession) => {
                                Act::Execute
                            }
                            GateOutcome::Decided(_) => {
                                Act::Deny("denied by human reviewer".to_string())
                            }
                            GateOutcome::Suspended(ticket) => Act::Suspend(ticket),
                        }
                    }
                    Decision::Deny => Act::Deny("denied by approval policy".to_string()),
                };

                match act {
                    Act::Suspend(ticket) => {
                        self.push_emit(&mut events, AgentEvent::GateOpened(ticket))
                            .await;
                        self.push_emit(&mut events, AgentEvent::Stopped(StopReason::Halted))
                            .await;
                        suspended = true;
                        break;
                    }
                    Act::Deny(reason) => {
                        let result = error_tool_result(reason);
                        let msg = tool_result_message(call, &result);
                        self.transcript.append(req.run_id, msg.clone()).await?;
                        self.push_emit(&mut events, AgentEvent::Message(msg)).await;
                    }
                    Act::Execute => {
                        let idem = format!("{}:{}:{}", req.run_id, iteration, ordinal);
                        // A tool execution FAILURE must not abort the turn: that would
                        // leave the already-persisted `tool_use` block with no matching
                        // `tool_result`, corrupting the next turn's history (the model
                        // rejects an orphan tool_use). Feed an `is_error` result back so
                        // the model can react — parity with the legacy loop.
                        let result = match self
                            .tools
                            .call(req.run_id, call.clone(), idem.clone())
                            .await
                        {
                            Ok(r) => r,
                            Err(e) => error_tool_result(format!("tool execution failed: {e}")),
                        };
                        executed += 1;
                        if result.terminal {
                            terminal_count += 1;
                        }
                        self.transcript
                            .journal_tool_call(
                                req.run_id,
                                ToolCallRecord {
                                    key: idem,
                                    call: call.clone(),
                                    result: result.clone(),
                                },
                            )
                            .await?;
                        let msg = tool_result_message(call, &result);
                        self.transcript.append(req.run_id, msg.clone()).await?;
                        self.push_emit(&mut events, AgentEvent::Message(msg)).await;
                    }
                }
            }
            if suspended {
                break;
            }
            // When EVERY executed tool was terminal (user-audience output / built-in
            // side-effect self-save), the turn's answer is already produced — finalize
            // without a no-op continuation model call (parity with the MCP extension's
            // `CompleteWithContent` / Track-B inline self-save). A mix still continues
            // so the model can reason about the non-terminal results.
            if executed > 0 && terminal_count == executed {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::NoToolCall))
                    .await;
                break;
            }

            iteration += 1;
        }

        Ok(events)
    }
}

/// Assemble the wire messages: the contributed system blocks as one `System`
/// message (when non-empty), then the loaded history verbatim.
fn assemble_messages(system: &[ContentBlock], history: &[ChatMessage]) -> Vec<ChatMessage> {
    let mut msgs = Vec::with_capacity(history.len() + 1);
    if !system.is_empty() {
        msgs.push(ChatMessage::with_blocks(Role::System, system.to_vec()));
    }
    msgs.extend_from_slice(history);
    msgs
}

/// Pull the model's `ToolUse` blocks out of an assistant message (P2 — tool
/// requests ride inside message content).
/// If the transcript ends with an assistant message that carries `ToolUse` blocks
/// (i.e. it is the LAST message, so no tool results follow), return it — the turn
/// was suspended mid-tool-execution and should resume by executing those calls
/// rather than issuing a fresh model call. Domain-neutral: pure transcript shape.
fn last_pending_assistant(history: &[ChatMessage]) -> Option<ChatMessage> {
    let last = history.last()?;
    if last.role == Role::Assistant
        && last
            .content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    {
        Some(last.clone())
    } else {
        None
    }
}

fn extract_tool_calls(msg: &ChatMessage) -> Vec<ToolCall> {
    msg.content
        .iter()
        .filter_map(|b| match b {
            ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                id: id.clone(),
                server: None,
                name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn error_tool_result(message: impl Into<String>) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::Text {
            text: message.into(),
        }],
        is_error: true,
        structured_content: None,
        terminal: false,
    }
}

fn tool_result_message(call: &ToolCall, result: &ToolResult) -> ChatMessage {
    ChatMessage::with_blocks(
        Role::Tool,
        vec![ContentBlock::ToolResult {
            tool_use_id: call.id.clone(),
            name: Some(call.name.clone()),
            content: result.content.clone(),
            is_error: Some(result.is_error),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::TrustedAutoApprovePolicy;
    use crate::types::ToolScope;
    use uuid::Uuid;
    use crate::test_fakes::{
        assistant_tool, core_with, GateBehavior, ScriptedModel,
    };
    use crate::types::ApprovalMode;

    fn new_req() -> AgentTurnRequest {
        AgentTurnRequest {
            run_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            seed: TurnSeed::NewMessage(ChatMessage::user("hi")),
            system: vec![ContentBlock::Text {
                text: "you are helpful".into(),
            }],
            tool_scope: ToolScope::default(),
            start_iteration: 1,
            inputs: serde_json::Value::Null,
        }
    }

    fn last_stop(events: &[AgentEvent]) -> StopReason {
        events
            .iter()
            .rev()
            .find_map(|e| match e {
                AgentEvent::Stopped(r) => Some(*r),
                _ => None,
            })
            .expect("a Stopped event")
    }

    /// A model that streams two text deltas through the sink, then returns a
    /// final message — proves the loop wires `call_streaming` → the `EventSink`.
    struct StreamingModel;
    #[async_trait]
    impl ModelClient for StreamingModel {
        async fn call(&self, _req: ChatRequest) -> Result<(ChatMessage, Usage), AppError> {
            Ok((ChatMessage::assistant("hello world"), Usage::default()))
        }
        async fn call_streaming(
            &self,
            req: ChatRequest,
            sink: &dyn DeltaSink,
        ) -> Result<(ChatMessage, Usage), AppError> {
            sink.on_delta(&ContentBlockDelta::TextDelta {
                index: 0,
                delta: "hello ".into(),
            })
            .await;
            sink.on_delta(&ContentBlockDelta::TextDelta {
                index: 0,
                delta: "world".into(),
            })
            .await;
            self.call(req).await
        }
    }

    #[tokio::test]
    async fn streaming_deltas_forwarded_to_sink() {
        use crate::test_fakes::{FakeGate, FakeResolver, FakeSink, FakeTools, FakeTranscript};
        let sink = Arc::new(FakeSink::default());
        let core = AgentCore {
            transcript: Arc::new(FakeTranscript::default()),
            sink: sink.clone(),
            tools: Arc::new(FakeTools::new(true)),
            gate: Arc::new(FakeGate {
                behavior: GateBehavior::Approve,
            }),
            policy: Arc::new(TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest)),
            models: Arc::new(FakeResolver::default()),
            model: Arc::new(StreamingModel),
            model_factory: Arc::new(ProviderModelClientFactory),
            extensions: vec![],
            reviewer: None,
            budget: Budget::new(2, 1_000_000, 1_000_000),
            limits: Default::default(),
            sandbox: SandboxMode::WorkspaceWrite { network: false },
            model_name: "test".into(),
        };
        core.run(new_req(), CancelToken::new()).await.unwrap();
        let deltas = sink
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::ContentDelta(_)))
            .count();
        assert_eq!(
            deltas, 2,
            "the loop must forward call_streaming deltas to the EventSink as ContentDelta events"
        );
    }

    #[tokio::test]
    async fn stops_on_no_tool_call() {
        let model = Arc::new(ScriptedModel::final_text("final answer"));
        let harness = core_with(model, true, GateBehavior::Approve, TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest));
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();
        assert_eq!(last_stop(&events), StopReason::NoToolCall);
        // No tool was executed.
        assert!(harness.transcript.journal.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn executes_trusted_tool_then_stops() {
        // Round 1: a tool call; round 2: a final answer.
        let model = Arc::new(ScriptedModel::script(vec![
            assistant_tool("t1", "search", serde_json::json!({"q": "x"})),
            ChatMessage::assistant("done"),
        ]));
        // trusted=true → auto-approve; TrustedAutoApprovePolicy returns Auto.
        let harness = core_with(model, true, GateBehavior::Approve, TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest));
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();

        assert_eq!(last_stop(&events), StopReason::NoToolCall);
        // Exactly one journaled tool call (P5) + the tool was actually invoked.
        assert_eq!(harness.transcript.journal.lock().unwrap().len(), 1);
        assert_eq!(harness.tools.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn iteration_cap_synthesizes_error_results() {
        // Model always wants a tool; max_steps = 1 → cap after the first call.
        let model = Arc::new(ScriptedModel::always_tool("t", "loop_tool"));
        let mut harness = core_with(model, true, GateBehavior::Approve, TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest));
        harness.core.budget = Budget::new(1, 1_000_000, 1_000_000);
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();

        assert_eq!(last_stop(&events), StopReason::IterationCap);
        // The unexecuted tool got a synthesized is_error result (no orphan) and
        // was never actually invoked/journaled.
        assert!(harness.transcript.journal.lock().unwrap().is_empty());
        assert!(harness.tools.calls.lock().unwrap().is_empty());
        let has_error_result = harness
            .transcript
            .msgs
            .lock()
            .unwrap()
            .values()
            .flatten()
            .any(|m| {
                m.content.iter().any(|b| {
                    matches!(b, ContentBlock::ToolResult { is_error: Some(true), .. })
                })
            });
        assert!(has_error_result);
    }

    #[tokio::test]
    async fn denied_tool_returns_error_and_continues() {
        // Round 1: a tool call from an UNtrusted server under `Never` → Deny.
        let model = Arc::new(ScriptedModel::script(vec![
            assistant_tool("t1", "danger", serde_json::json!({})),
            ChatMessage::assistant("ok"),
        ]));
        let harness = core_with(model, false, GateBehavior::Approve, TrustedAutoApprovePolicy::new(ApprovalMode::Never));
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();

        assert_eq!(last_stop(&events), StopReason::NoToolCall);
        // Denied → never executed / journaled, but an error result was appended.
        assert!(harness.tools.calls.lock().unwrap().is_empty());
        assert!(harness.transcript.journal.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn gate_denial_returns_error_and_continues() {
        // Untrusted + UnlessTrusted → Prompt → the human denies → error result,
        // then round 2 returns a final answer.
        let model = Arc::new(ScriptedModel::script(vec![
            assistant_tool("t1", "mutate", serde_json::json!({})),
            ChatMessage::assistant("ok"),
        ]));
        let harness = core_with(
            model,
            false,
            GateBehavior::Deny,
            TrustedAutoApprovePolicy::new(ApprovalMode::UnlessTrusted),
        );
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();

        assert_eq!(last_stop(&events), StopReason::NoToolCall);
        // Denied by the human → never executed / journaled.
        assert!(harness.tools.calls.lock().unwrap().is_empty());
        assert!(harness.transcript.journal.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn gate_suspend_halts_the_turn() {
        // Untrusted + UnlessTrusted → Prompt → the gate suspends durably.
        let model = Arc::new(ScriptedModel::always_tool("t", "mutate"));
        let harness = core_with(model, false, GateBehavior::Suspend, TrustedAutoApprovePolicy::new(ApprovalMode::UnlessTrusted));
        let events = harness.core.run(new_req(), CancelToken::new()).await.unwrap();

        assert!(events.iter().any(|e| matches!(e, AgentEvent::GateOpened(_))));
        assert_eq!(last_stop(&events), StopReason::Halted);
        assert!(harness.tools.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancel_before_start_halts() {
        let model = Arc::new(ScriptedModel::final_text("unused"));
        let harness = core_with(model, true, GateBehavior::Approve, TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest));
        let cancel = CancelToken::new();
        cancel.cancel();
        let events = harness.core.run(new_req(), cancel).await.unwrap();
        assert_eq!(last_stop(&events), StopReason::Halted);
        // The model was never called.
        assert_eq!(*harness.model.calls.lock().unwrap(), 0);
    }
}

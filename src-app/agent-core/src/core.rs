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
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Signal cancellation. The loop stops at its next checkpoint with `Halted`.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

/// One model call → an assistant `ChatMessage` + `Usage`. The seam that makes
/// the loop unit-testable without a real LLM (see the module docs).
#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn call(&self, req: ChatRequest) -> Result<(ChatMessage, Usage), AppError>;
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

            // `before_model` hooks (compaction runs here at a late order). A veto
            // stops the turn with a final answer.
            if run_before_model(&self.extensions, &mut chat_req).await? == Flow::ShortCircuit {
                self.push_emit(&mut events, AgentEvent::Stopped(StopReason::NoToolCall))
                    .await;
                break;
            }

            let (assistant_msg, usage) = self.model.call(chat_req).await?;
            budget.add_tokens(usage.total_tokens);
            self.transcript
                .append(req.run_id, assistant_msg.clone())
                .await?;
            self.push_emit(&mut events, AgentEvent::Message(assistant_msg.clone()))
                .await;
            self.push_emit(&mut events, AgentEvent::Usage(usage)).await;

            // Post-round extension hooks (e.g. background memory extract); a
            // short-circuit ends the turn.
            let mut short_circuit = false;
            for ext in &self.extensions {
                if ext.after_round(&assistant_msg).await? == Flow::ShortCircuit {
                    short_circuit = true;
                    break;
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
                        let result = self
                            .tools
                            .call(req.run_id, call.clone(), idem.clone())
                            .await?;
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

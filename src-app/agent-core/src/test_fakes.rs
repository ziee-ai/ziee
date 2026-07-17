//! Shared in-memory fake ports + a fake `ModelClient` for the crate's unit
//! tests (compiled only under `#[cfg(test)]`). These let the loop / compaction /
//! fan-out / reviewer be exercised WITHOUT a real LLM or a database.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role, Tool};
use async_trait::async_trait;
use uuid::Uuid;
use ziee_core::AppError;

use crate::budget::Budget;
use crate::core::{AgentCore, ModelClient, ModelClientFactory, ProviderModelClientFactory};
use crate::ports::{
    ApprovalPolicy, EventSink, HumanGate, ModelResolver, ToolProvider, TranscriptStore,
};
use crate::types::{
    AgentEvent, GateAsk, GateOutcome, GateTicket, ReviewDecision, SandboxMode, SubagentLimits,
    ToolCall, ToolCallRecord, ToolResult, ToolScope, Usage,
};

/// Build an assistant message carrying a single `ToolUse` block.
pub fn assistant_tool(
    id: impl Into<String>,
    name: impl Into<String>,
    input: serde_json::Value,
) -> ChatMessage {
    ChatMessage::with_blocks(
        Role::Assistant,
        vec![ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }],
    )
}

// ---------------------------------------------------------------------------
// Fake ModelClient
// ---------------------------------------------------------------------------

/// A scriptable fake model: pops queued responses, or (when `always_tool` is
/// set) returns an endless tool call, or falls back to `default_final`. Tracks
/// call count + peak concurrency (for the fan-out semaphore test).
pub struct ScriptedModel {
    pub script: Mutex<VecDeque<ChatMessage>>,
    pub default_final: ChatMessage,
    pub always_tool: Option<(String, String)>,
    pub calls: Mutex<usize>,
    pub active: AtomicUsize,
    pub peak: AtomicUsize,
    pub delay_ms: u64,
}

impl ScriptedModel {
    fn base() -> Self {
        Self {
            script: Mutex::new(VecDeque::new()),
            default_final: ChatMessage::assistant("done"),
            always_tool: None,
            calls: Mutex::new(0),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            delay_ms: 0,
        }
    }

    /// Always returns a final text answer (no tool call).
    pub fn final_text(text: impl Into<String>) -> Self {
        Self {
            default_final: ChatMessage::assistant(text),
            ..Self::base()
        }
    }

    /// Returns each scripted response in turn, then `default_final`.
    pub fn script(responses: Vec<ChatMessage>) -> Self {
        Self {
            script: Mutex::new(responses.into()),
            ..Self::base()
        }
    }

    /// Always requests the same tool (drives the iteration-cap / gate tests).
    pub fn always_tool(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            always_tool: Some((id.into(), name.into())),
            ..Self::base()
        }
    }

    /// A final-text model with a per-call delay, to observe concurrency.
    pub fn concurrent(text: impl Into<String>, delay_ms: u64) -> Self {
        Self {
            default_final: ChatMessage::assistant(text),
            delay_ms,
            ..Self::base()
        }
    }
}

#[async_trait]
impl ModelClient for ScriptedModel {
    async fn call(&self, _req: ChatRequest) -> Result<(ChatMessage, Usage), AppError> {
        let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        *self.calls.lock().unwrap() += 1;
        let msg = if let Some((id, name)) = &self.always_tool {
            assistant_tool(id.clone(), name.clone(), serde_json::json!({}))
        } else {
            self.script
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| self.default_final.clone())
        };
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok((msg, Usage::default()))
    }
}

// ---------------------------------------------------------------------------
// Fake TranscriptStore
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeTranscript {
    pub msgs: Mutex<HashMap<Uuid, Vec<ChatMessage>>>,
    pub journal: Mutex<Vec<ToolCallRecord>>,
    pub replaced: Mutex<Vec<(Uuid, usize)>>,
}

#[async_trait]
impl TranscriptStore for FakeTranscript {
    async fn load(&self, run_id: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        Ok(self
            .msgs
            .lock()
            .unwrap()
            .get(&run_id)
            .cloned()
            .unwrap_or_default())
    }

    async fn append(&self, run_id: Uuid, msg: ChatMessage) -> Result<(), AppError> {
        self.msgs.lock().unwrap().entry(run_id).or_default().push(msg);
        Ok(())
    }

    async fn replace_head(
        &self,
        run_id: Uuid,
        summary: ChatMessage,
        upto: usize,
    ) -> Result<(), AppError> {
        self.replaced.lock().unwrap().push((run_id, upto));
        let mut g = self.msgs.lock().unwrap();
        let v = g.entry(run_id).or_default();
        let tail = v.split_off(upto.min(v.len()));
        *v = std::iter::once(summary).chain(tail).collect();
        Ok(())
    }

    async fn journal_tool_call(&self, _run_id: Uuid, rec: ToolCallRecord) -> Result<(), AppError> {
        self.journal.lock().unwrap().push(rec);
        Ok(())
    }

    async fn completed_tool_calls(
        &self,
        _run_id: Uuid,
    ) -> Result<Vec<ToolCallRecord>, AppError> {
        Ok(self.journal.lock().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------
// Fake EventSink
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeSink {
    pub events: Mutex<Vec<AgentEvent>>,
}

#[async_trait]
impl EventSink for FakeSink {
    async fn emit(&self, ev: AgentEvent) {
        self.events.lock().unwrap().push(ev);
    }
}

// ---------------------------------------------------------------------------
// Fake ToolProvider
// ---------------------------------------------------------------------------

pub struct FakeTools {
    pub trusted: bool,
    pub tools: Vec<Tool>,
    pub calls: Mutex<Vec<ToolCall>>,
    pub result: ToolResult,
}

impl FakeTools {
    pub fn new(trusted: bool) -> Self {
        Self {
            trusted,
            tools: vec![Tool::function(
                "search",
                "search the web",
                serde_json::json!({"type": "object"}),
            )],
            calls: Mutex::new(Vec::new()),
            result: ToolResult {
                content: vec![ContentBlock::Text {
                    text: "tool ok".into(),
                }],
                is_error: false,
                structured_content: None,
                terminal: false,
            },
        }
    }
}

#[async_trait]
impl ToolProvider for FakeTools {
    async fn list(&self, _scope: &ToolScope) -> Result<Vec<Tool>, AppError> {
        Ok(self.tools.clone())
    }

    async fn call(
        &self,
        _run_id: Uuid,
        call: ToolCall,
        _idem: crate::types::IdempotencyKey,
    ) -> Result<ToolResult, AppError> {
        self.calls.lock().unwrap().push(call);
        Ok(self.result.clone())
    }

    fn is_trusted(&self, _server: &str) -> bool {
        self.trusted
    }
}

// ---------------------------------------------------------------------------
// Fake HumanGate
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum GateBehavior {
    Approve,
    Deny,
    Suspend,
}

pub struct FakeGate {
    pub behavior: GateBehavior,
}

#[async_trait]
impl HumanGate for FakeGate {
    async fn request(&self, _run_id: Uuid, _ask: GateAsk) -> Result<GateOutcome, AppError> {
        Ok(match self.behavior {
            GateBehavior::Approve => GateOutcome::Decided(ReviewDecision::Approved),
            GateBehavior::Deny => GateOutcome::Decided(ReviewDecision::Denied),
            GateBehavior::Suspend => GateOutcome::Suspended(GateTicket { id: Uuid::new_v4() }),
        })
    }
}

// ---------------------------------------------------------------------------
// Fake ModelResolver + a fake ModelClientFactory (fan-out)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeResolver {
    /// The model_ids `resolve` was asked for, in order.
    pub asked: Mutex<Vec<Uuid>>,
    /// A model_id that resolution rejects (RBAC-denied).
    pub reject: Option<Uuid>,
}

#[async_trait]
impl ModelResolver for FakeResolver {
    async fn resolve(&self, model_id: Uuid, _user_id: Uuid) -> Result<Arc<Provider>, AppError> {
        self.asked.lock().unwrap().push(model_id);
        if self.reject == Some(model_id) {
            return Err(AppError::forbidden(
                "MODEL_ACCESS_DENIED",
                "model not accessible",
            ));
        }
        // Distinct provider per id (api_key varies); `Provider::new` does no I/O.
        Ok(Arc::new(
            Provider::new("openai", model_id.to_string(), "").unwrap(),
        ))
    }
}

/// A factory that ignores the resolved `Provider` and returns a fixed fake
/// model client — keeps the fan-out resolution test network-free.
pub struct FakeFactory {
    pub inner: Arc<dyn ModelClient>,
}

impl ModelClientFactory for FakeFactory {
    fn for_provider(&self, _provider: Arc<Provider>) -> Arc<dyn ModelClient> {
        self.inner.clone()
    }
}

// ---------------------------------------------------------------------------
// Harness builder
// ---------------------------------------------------------------------------

pub struct Harness {
    pub core: AgentCore,
    pub transcript: Arc<FakeTranscript>,
    pub tools: Arc<FakeTools>,
    pub model: Arc<ScriptedModel>,
}

/// Assemble an `AgentCore` over in-memory fakes for a loop unit test.
pub fn core_with(
    model: Arc<ScriptedModel>,
    trusted: bool,
    gate: GateBehavior,
    policy: impl ApprovalPolicy + 'static,
) -> Harness {
    let transcript = Arc::new(FakeTranscript::default());
    let tools = Arc::new(FakeTools::new(trusted));
    let core = AgentCore {
        transcript: transcript.clone(),
        sink: Arc::new(FakeSink::default()),
        tools: tools.clone(),
        gate: Arc::new(FakeGate { behavior: gate }),
        policy: Arc::new(policy),
        models: Arc::new(FakeResolver::default()),
        model: model.clone(),
        model_factory: Arc::new(ProviderModelClientFactory),
        extensions: vec![],
        reviewer: None,
        budget: Budget::new(10, 1_000_000, 1_000_000),
        limits: SubagentLimits::default(),
        sandbox: SandboxMode::WorkspaceWrite { network: false },
        model_name: "test-model".into(),
    };
    Harness {
        core,
        transcript,
        tools,
        model,
    }
}

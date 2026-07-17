//! Real-LLM behavioral verification of the `AgentCore` loop (TEST — proves the
//! loop does a genuine tool-calling round-trip against a live tool-capable model
//! via the LiteLLM bridge). Soft-skips unless `ZIEE_TEST_LLM_BASE_URL` is set.
//!
//! Run: `source server/tests/.env.test && cargo test -p agent-core --test
//! real_llm_loop -- --nocapture`.

use std::sync::{Arc, Mutex};

use ai_providers::{ChatMessage, ContentBlock, Provider, Role, Tool};
use async_trait::async_trait;
use uuid::Uuid;
use ziee_core::AppError;

use agent_core::{
    AgentCore, AgentEvent, AgentTurnRequest, ApprovalMode, ApprovalPolicy, Budget, CancelToken,
    EventSink, GateAsk, GateOutcome, HumanGate, ModelResolver, ProviderModelClient,
    ProviderModelClientFactory, SandboxMode, StopReason, ToolCall, ToolCallRecord, ToolProvider,
    ToolResult, ToolScope, TranscriptStore, TrustedAutoApprovePolicy, TurnSeed,
};

// --- minimal inline fakes (the crate's own test_fakes are #[cfg(test)]) ---

#[derive(Default)]
struct MemTranscript {
    msgs: Mutex<Vec<ChatMessage>>,
}
#[async_trait]
impl TranscriptStore for MemTranscript {
    async fn load(&self, _r: Uuid) -> Result<Vec<ChatMessage>, AppError> {
        Ok(self.msgs.lock().unwrap().clone())
    }
    async fn append(&self, _r: Uuid, m: ChatMessage) -> Result<(), AppError> {
        self.msgs.lock().unwrap().push(m);
        Ok(())
    }
    async fn replace_head(&self, _r: Uuid, _s: ChatMessage, _u: usize) -> Result<(), AppError> {
        Ok(())
    }
    async fn journal_tool_call(&self, _r: Uuid, _rec: ToolCallRecord) -> Result<(), AppError> {
        Ok(())
    }
    async fn completed_tool_calls(&self, _r: Uuid) -> Result<Vec<ToolCallRecord>, AppError> {
        Ok(vec![])
    }
}

#[derive(Default)]
struct MemSink {
    events: Mutex<Vec<AgentEvent>>,
}
#[async_trait]
impl EventSink for MemSink {
    async fn emit(&self, ev: AgentEvent) {
        self.events.lock().unwrap().push(ev);
    }
}

struct WeatherTool {
    calls: Mutex<Vec<ToolCall>>,
}
#[async_trait]
impl ToolProvider for WeatherTool {
    async fn list(&self, _s: &ToolScope) -> Result<Vec<Tool>, AppError> {
        Ok(vec![Tool::function(
            "get_weather",
            "Get the current weather for a city.",
            serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string", "description": "City name" } },
                "required": ["city"]
            }),
        )])
    }
    async fn call(
        &self,
        _r: Uuid,
        call: ToolCall,
        _idem: String,
    ) -> Result<ToolResult, AppError> {
        self.calls.lock().unwrap().push(call);
        Ok(ToolResult {
            content: vec![ContentBlock::Text {
                text: "It is 20°C and sunny in Paris.".into(),
            }],
            is_error: false,
            structured_content: Some(serde_json::json!({"tempC": 20, "sky": "sunny"})),
        })
    }
    fn is_trusted(&self, _server: &str) -> bool {
        true // read-only → auto-approve, no human gate
    }
}

struct NoTools;
#[async_trait]
impl ToolProvider for NoTools {
    async fn list(&self, _s: &ToolScope) -> Result<Vec<Tool>, AppError> {
        Ok(vec![])
    }
    async fn call(&self, _r: Uuid, _c: ToolCall, _i: String) -> Result<ToolResult, AppError> {
        Err(AppError::internal_error("no tools"))
    }
    fn is_trusted(&self, _s: &str) -> bool {
        true
    }
}

struct NoGate;
#[async_trait]
impl HumanGate for NoGate {
    async fn request(&self, _r: Uuid, _a: GateAsk) -> Result<GateOutcome, AppError> {
        Err(AppError::internal_error("gate should not be reached"))
    }
}

struct NoResolver;
#[async_trait]
impl ModelResolver for NoResolver {
    async fn resolve(&self, _m: Uuid, _u: Uuid) -> Result<Arc<Provider>, AppError> {
        Err(AppError::internal_error("resolver not used in this test"))
    }
}

/// Proves the provider (via the bridge) streams tokens as deltas, and the loop
/// forwards them as `ContentDelta` events — the streaming seam chat needs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_streams_text_deltas_from_real_model() {
    let base_url = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_streams_text_deltas_from_real_model — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name =
        std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());

    let provider = Arc::new(Provider::new("openai", key, base_url).expect("provider"));
    let sink = Arc::new(MemSink::default());
    let core = AgentCore {
        transcript: Arc::new(MemTranscript::default()),
        sink: sink.clone(),
        tools: Arc::new(NoTools),
        gate: Arc::new(NoGate),
        policy: Arc::new(TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest))
            as Arc<dyn ApprovalPolicy>,
        models: Arc::new(NoResolver),
        model: Arc::new(ProviderModelClient::new(provider)),
        model_factory: Arc::new(ProviderModelClientFactory),
        extensions: vec![],
        reviewer: None,
        budget: agent_core::Budget::new(2, 2_000_000, 2_000_000),
        limits: Default::default(),
        sandbox: SandboxMode::ReadOnly { network: false },
        model_name,
    };
    let req = AgentTurnRequest {
        run_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        seed: TurnSeed::NewMessage(ChatMessage::user(
            "Write two short sentences about the city of Paris. Do not use any tools.",
        )),
        system: vec![ContentBlock::Text { text: "You are a helpful assistant.".into() }],
        tool_scope: ToolScope { servers: vec![], allow_delegate: false },
        start_iteration: 1,
        inputs: serde_json::Value::Null,
    };
    let events = core.run(req, CancelToken::new()).await.expect("run");
    // ContentDelta events flow to the SINK (EventDeltaSink), not the returned Vec
    // (which stays Message/Usage/Stopped only). Count them where they land.
    let deltas = sink
        .events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, agent_core::AgentEvent::ContentDelta(_)))
        .count();
    let answered = events.iter().any(|e| matches!(e,
        agent_core::AgentEvent::Message(m)
            if m.content.iter().any(|b| matches!(b, ContentBlock::Text { text } if !text.trim().is_empty()))));
    eprintln!(
        "real text turn: {deltas} ContentDelta events, {} total events, answered={answered}",
        events.len()
    );
    assert!(answered, "expected a non-empty text answer from the real model");
    // The provider forwards ≥1 delta for a non-empty answer (one big delta if the
    // non-streaming-to-stream workaround is active; many if truly streaming).
    assert!(
        deltas >= 1,
        "expected ≥1 streamed ContentDelta for the non-empty answer; got {deltas}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_loop_does_real_tool_call_round_trip() {
    let base_url = match std::env::var("ZIEE_TEST_LLM_BASE_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            eprintln!("SKIP agent_loop_does_real_tool_call_round_trip — ZIEE_TEST_LLM_BASE_URL unset");
            return;
        }
    };
    let key = std::env::var("ZIEE_TEST_LLM_KEY").unwrap_or_else(|_| "sk-local-audit".into());
    let model_name =
        std::env::var("ZIEE_TEST_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".into());

    let provider = Arc::new(Provider::new("openai", key, base_url).expect("provider"));
    let model = Arc::new(ProviderModelClient::new(provider));
    let transcript = Arc::new(MemTranscript::default());
    let sink = Arc::new(MemSink::default());
    let tools = Arc::new(WeatherTool {
        calls: Mutex::new(vec![]),
    });

    let core = AgentCore {
        transcript: transcript.clone(),
        sink: sink.clone(),
        tools: tools.clone(),
        gate: Arc::new(NoGate),
        policy: Arc::new(TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest))
            as Arc<dyn ApprovalPolicy>,
        models: Arc::new(NoResolver),
        model: model.clone(),
        model_factory: Arc::new(ProviderModelClientFactory),
        extensions: vec![],
        reviewer: None,
        budget: Budget::new(4, 2_000_000, 2_000_000),
        limits: Default::default(),
        sandbox: SandboxMode::ReadOnly { network: false },
        model_name,
    };

    let req = AgentTurnRequest {
        run_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        seed: TurnSeed::NewMessage(ChatMessage::user(
            "What is the weather in Paris right now? Use the get_weather tool to find out.",
        )),
        system: vec![ContentBlock::Text {
            text: "You are a helpful assistant. When asked about the weather you MUST call the \
                   get_weather tool with the city; do not guess."
                .into(),
        }],
        tool_scope: ToolScope {
            servers: vec!["test".into()],
            allow_delegate: false,
        },
        start_iteration: 1,
        inputs: serde_json::Value::Null,
    };

    let events = core
        .run(req, CancelToken::new())
        .await
        .expect("agent run should not error");

    // 1. The model REALLY routed the tool (Qwen emitted a get_weather ToolUse
    //    that the loop executed).
    let calls = tools.calls.lock().unwrap();
    assert!(
        calls.iter().any(|c| c.name == "get_weather"),
        "expected a real get_weather tool call from the model; got calls: {:?}",
        calls.iter().map(|c| &c.name).collect::<Vec<_>>()
    );

    // 2. The loop fed the result back and reached a clean final answer (no more
    //    tool calls) — a full round trip, not a truncated turn.
    let stop = events.iter().rev().find_map(|e| match e {
        AgentEvent::Stopped(r) => Some(*r),
        _ => None,
    });
    assert_eq!(
        stop,
        Some(StopReason::NoToolCall),
        "expected the loop to end with a final answer (NoToolCall) after the tool result"
    );

    // (Streaming-delta forwarding is proven deterministically by the core unit
    //  test `streaming_deltas_forwarded`; whether the bridge emits per-token
    //  deltas for a given turn is a provider detail, not asserted here.)

    // 3. A final assistant message exists in the transcript.
    let msgs = transcript.msgs.lock().unwrap();
    assert!(
        msgs.iter().any(|m| matches!(m.role, Role::Assistant)),
        "expected at least one assistant message persisted"
    );
    eprintln!(
        "OK — real tool round trip: {} tool call(s), {} events, {} transcript msgs",
        calls.len(),
        events.len(),
        msgs.len()
    );
}

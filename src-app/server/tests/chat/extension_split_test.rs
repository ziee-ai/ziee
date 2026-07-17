//! TEST-25 — the extension re-home preserves the chat extension pipeline on the
//! agent-core path: the assistant chat-extension still injects its `instructions`
//! as a labeled system message (run via the `RegistryBridge`'s `before_llm_call`),
//! and it lands BEFORE the user message (assistant→user layering). Driven by the
//! capturing `oai_capture_stub::StubChat`, asserting on the exact request the
//! provider produced. RUN ISOLATED (sets the cutover flag).

use std::time::Duration;

use serde_json::{json, Value};
use uuid::Uuid;

use crate::chat::helpers::{self, parse_uuid};
use crate::common::chat_stream_probe::ChatStreamProbe;
use crate::common::oai_capture_stub::{StubChat, StubPlan};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

const TURN_TIMEOUT: Duration = Duration::from_secs(30);

async fn stub_model(server: &TestServer, user_id: &str, base_url: &str) -> Value {
    let admin = create_user_with_permissions(
        server,
        "ext_split_admin",
        &[
            "llm_models::read",
            "llm_models::create",
            "llm_providers::read",
            "llm_providers::create",
            "llm_providers::edit",
        ],
    )
    .await;
    let client = reqwest::Client::new();
    let provider: Value = client
        .post(server.api_url("/llm-providers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": format!("ExtSplit {}", &Uuid::new_v4().to_string()[..8]),
            "provider_type": "custom", "enabled": true, "api_key": "test", "base_url": base_url,
        }))
        .send().await.unwrap().json().await.unwrap();
    let model: Value = client
        .post(server.api_url("/llm-models"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "provider_id": provider["id"], "name": "stub-ext-split",
            "display_name": "Stub Ext Split", "enabled": true, "engine_type": "none",
            "file_format": "gguf",
            "capabilities": { "chat": true, "completion": true, "embedding": false }
        }))
        .send().await.unwrap().json().await.unwrap();
    helpers::ensure_user_has_model_access(server, user_id, &model).await;
    model
}

#[tokio::test]
async fn assistant_extension_injects_system_prompt_on_agent_core() {
    unsafe { std::env::set_var("ZIEE_CHAT_AGENT_CORE", "1") };

    let stub = StubChat::start(StubPlan { text: "ok".into(), ..Default::default() }).await;
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "ext_split_user",
        &[
            "conversations::create", "conversations::read",
            "messages::create", "messages::read", "llm_models::read",
            "assistants::create", "assistants::read",
        ],
    )
    .await;
    let model = stub_model(&server, &user.user_id, &stub.base_url()).await;
    let model_id = parse_uuid(&model["id"]);

    // A user assistant with a UNIQUE instruction string.
    let marker = "ZIEE_EXT_SPLIT_MARKER_dark_theme_only";
    let assistant: Value = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "ext-split-asst", "description": "d", "instructions": marker }))
        .send().await.unwrap().json().await.unwrap();
    let assistant_id = parse_uuid(&assistant["id"]);

    let conv = helpers::create_conversation(&server, &user.token, Some(model_id), Some("es")).await;
    let conv_id = parse_uuid(&conv["id"]);
    let branch_id = parse_uuid(&conv["active_branch_id"]);

    // Send with the assistant bound; wait for the turn to finish (stub captures it).
    let mut probe = ChatStreamProbe::open(&server, &user.token).await;
    probe.subscribe(Some(conv_id)).await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "content": "hello", "model_id": model_id.to_string(),
            "branch_id": branch_id.to_string(), "assistant_id": assistant_id.to_string(),
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200, "send failed");
    let _ = probe.collect_until_terminal(conv_id, TURN_TIMEOUT).await;

    // The captured request must carry the assistant instruction in a SYSTEM message,
    // positioned before the user message (assistant→user layering preserved).
    // `requests()` returns the raw OpenAI request bodies (`{ messages: [{role, content}] }`).
    let reqs = stub.requests();
    let msg_text = |m: &Value| -> String {
        match &m["content"] {
            Value::String(s) => s.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        }
    };
    let req = reqs
        .iter()
        .find(|r| {
            r["messages"]
                .as_array()
                .map(|ms| ms.iter().any(|m| msg_text(m).contains(marker)))
                .unwrap_or(false)
        })
        .expect("a captured request carrying the assistant instruction");
    let messages = req["messages"].as_array().unwrap();
    let sys_idx = messages.iter().position(|m| {
        m["role"].as_str() == Some("system") && msg_text(m).contains(marker)
    });
    let user_idx = messages.iter().position(|m| m["role"].as_str() == Some("user"));
    let roles: Vec<&str> = messages.iter().filter_map(|m| m["role"].as_str()).collect();
    assert!(
        matches!((sys_idx, user_idx), (Some(s), Some(u)) if s < u),
        "assistant system prompt (with marker) must precede the user message; roles={roles:?}"
    );

    unsafe { std::env::remove_var("ZIEE_CHAT_AGENT_CORE") };
}

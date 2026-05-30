//! Gold smoke — REAL engine + REAL model, fully automated (no hand-placing).
//!
//! Heavy (real network + a ~670 MB model download + CPU inference) but NOT
//! `#[ignore]` — it runs as part of the llm_local_runtime suite. Requires
//! HUGGINGFACE_API_KEY, which lives in `src-app/server/tests/.env.test`:
//!
//! ```bash
//! source src-app/server/tests/.env.test   # HUGGINGFACE_API_KEY
//! cargo test --test integration_tests -- --nocapture \
//!     llm_local_runtime::gold_smoke
//! ```
//!
//! End to end, with nothing staged by hand:
//!  1. download the real `llama-server` from the published `ziee-ai`
//!     fork release (`v0.0.1-alpha` — cpu-only; v0.0.2-alpha adds the
//!     versioned cuda/rocm artifacts) via the production download path —
//!     this exercises the symlink-preserving extractor against the real
//!     archive (which ships SONAME symlinks);
//!  2. download a real tiny chat GGUF (TinyLlama-1.1B) from HuggingFace;
//!  3. let validation-by-loading load it in the real engine;
//!  4. chat through the proxy → real token generation.

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use reqwest::StatusCode;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

async fn validation_status(server: &TestServer, token: &str, model_id: Uuid) -> String {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/llm-models/{model_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    body["validation_status"].as_str().unwrap_or("").to_string()
}

#[tokio::test]
async fn real_release_download_and_infer() {
    // Plain TestServer — NO mirror env, so the download path hits the real
    // github.com/ziee-ai/* releases.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    // CPU model load + first token can be slow — widen the auto-start window.
    let r = lrt::update_runtime_settings(&server, &admin.token, json!({ "auto_start_timeout_secs": 180 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    // 1. Download the REAL engine from the published fork release.
    lrt::download_engine_release(&server, &admin.token, "llamacpp", "v0.0.1-alpha").await;

    // 2. Local provider + a REAL tiny chat GGUF pulled from HuggingFace.
    let (provider_id, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;
    let model = lrt::download_test_gguf_model(&server, &admin.token, provider_id).await;
    let model_name = model["name"].as_str().expect("model name").to_string();
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    // 3. Validation-by-loading: the real engine loads the real GGUF. Wait
    //    for it to finish (it also stops its probe instance) before we
    //    chat, so the two don't race on starting the same model.
    let mut status = String::new();
    for _ in 0..90 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        status = validation_status(&server, &admin.token, model_id).await;
        if matches!(status.as_str(), "valid" | "validation_warning" | "invalid" | "failed") {
            break;
        }
    }
    assert!(
        matches!(status.as_str(), "valid" | "validation_warning"),
        "model should validate against the real engine, got '{status}'"
    );

    // 4. Chat through the proxy → auto-starts the real engine → real inference.
    let resp = lrt::proxy_chat(
        &server,
        &proxy_token,
        json!({
            "model": model_name,
            "messages": [{"role": "user", "content": "Reply with the single word: ok"}],
            "max_tokens": 16,
            "stream": false
        }),
    )
    .await;
    let st = resp.status();
    let text = resp.text().await.unwrap();
    assert_eq!(st, StatusCode::OK, "real engine chat should 200; body: {text}");
    let body: serde_json::Value = serde_json::from_str(&text).unwrap();
    let content = body["choices"][0]["message"]["content"].as_str().unwrap_or("");
    assert!(!content.trim().is_empty(), "real engine should return a non-empty completion; got: {text}");
    eprintln!("real-release gold smoke: engine replied: {content:?}");
}

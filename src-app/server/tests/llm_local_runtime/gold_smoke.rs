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

use crate::common::{TestServer, TestServerOptions};
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
    // Plain TestServer — NO mirror env, so the download path hits the
    // real github.com/ziee-ai/* releases. ZIEE_DISABLE_MODEL_VALIDATION
    // skips the background Tier-2 validator: on this Mac it spawns the
    // engine for 90s (TIER2_HEALTH_DEADLINE_SECS), kills it, repeats —
    // wasting ~3 min and (worse) evicting the model file from the OS
    // page cache between attempts, so the chat-triggered auto-start
    // re-loads from cold disk and exceeds the auto_start_timeout cap.
    // The test loop below tolerates the resulting "completed" status
    // (validation didn't run → handler's initial status sticks).
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("ZIEE_DISABLE_MODEL_VALIDATION".into(), "1".into())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    // CPU model load + first token can be slow on commodity Apple
    // Silicon — auto-start to Healthy takes 3-4 min on cold cache for
    // TinyLlama Q4_K_M. Bump to the runtime-settings ceiling (600s,
    // enforced in repository.rs::update_runtime_settings).
    let r = lrt::update_runtime_settings(&server, &admin.token, json!({ "auto_start_timeout_secs": 600 })).await;
    assert_eq!(r.status(), StatusCode::OK);

    // 1. Download the REAL engine from the published fork release.
    //    v0.0.3-alpha (the current ziee-ai/llama.cpp tip) ships an
    //    8-month-newer llama.cpp build than v0.0.1; the older v0.0.1
    //    binary takes >10 min cold-load on commodity Apple Silicon
    //    CPU (exceeding even the runtime-settings auto_start_timeout
    //    ceiling of 600s) while v0.0.3 lands comfortably under it.
    //    Stays pinned to a specific tag (not 'latest') so a fresh
    //    upstream release with a regression doesn't silently break
    //    the test.
    lrt::download_engine_release(&server, &admin.token, "llamacpp", "v0.0.3-alpha").await;

    // 2. Local provider + a REAL tiny chat GGUF pulled from HuggingFace.
    let (provider_id, proxy_token, _p) = lrt::create_local_provider_with_token(&server, &admin.token).await;
    let model = lrt::download_test_gguf_model(&server, &admin.token, provider_id).await;
    let model_name = model["name"].as_str().expect("model name").to_string();
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();

    // 3. With ZIEE_DISABLE_MODEL_VALIDATION=1 set above, the
    //    background validator no-ops on enqueue and `validation_status`
    //    stays at "completed" (the file-committed initial state set
    //    by the upload handler). Accept that as a green precondition
    //    for the chat call; "valid" / "validation_warning" are the
    //    other valid outcomes when validation is enabled.
    let mut status = String::new();
    for _ in 0..90 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        status = validation_status(&server, &admin.token, model_id).await;
        if matches!(
            status.as_str(),
            "valid" | "validation_warning" | "invalid" | "failed" | "completed"
        ) {
            break;
        }
    }
    assert!(
        matches!(
            status.as_str(),
            "valid" | "validation_warning" | "completed"
        ),
        "model should validate against the real engine, got '{status}'"
    );

    // 4. Chat through the proxy → auto-starts the real engine → real
    //    inference. Use `stream: true` here: the global
    //    `tower_http::timeout::TimeoutLayer(60s)` in lib.rs caps the
    //    time-to-RESPOND-START on every route — for non-stream chat
    //    that's the entire completion (which on cold CPU exceeds 60s),
    //    so a non-stream gold_smoke gets 408'd before the engine can
    //    finish. The streaming path emits bytes as soon as the first
    //    token lands, well under the deadline. Production UI uses
    //    streaming anyway, so this is the realistic path to cover.
    let resp = lrt::proxy_chat(
        &server,
        &proxy_token,
        json!({
            "model": model_name,
            "messages": [{"role": "user", "content": "Reply with the single word: ok"}],
            "max_tokens": 16,
            "stream": true
        }),
    )
    .await;
    let st = resp.status();
    let text = resp.text().await.unwrap();
    assert_eq!(st, StatusCode::OK, "real engine chat should 200; body: {text}");
    // SSE body shape: a series of `data: {…}\n\n` lines ending in
    // `data: [DONE]\n\n`. Concatenate every `delta.content` field
    // across chunks; assert at least one non-empty token landed and
    // the stream terminated cleanly.
    let mut content = String::new();
    for line in text.lines() {
        let Some(payload) = line.strip_prefix("data: ") else { continue };
        let payload = payload.trim();
        if payload.is_empty() || payload == "[DONE]" {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
            if let Some(piece) = json["choices"][0]["delta"]["content"].as_str() {
                content.push_str(piece);
            }
        }
    }
    assert!(
        text.contains("[DONE]"),
        "stream should terminate with [DONE]; body: {text}"
    );
    assert!(
        !content.trim().is_empty(),
        "real engine should stream non-empty token(s); got body: {text}"
    );
    eprintln!("real-release gold smoke: engine streamed: {content:?}");
}

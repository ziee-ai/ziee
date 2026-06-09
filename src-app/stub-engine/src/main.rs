//! Minimal OpenAI-compatible stub engine for local-runtime integration
//! and E2E tests.
//!
//! It is launched by the *real* deployment path (`LocalDeployment::start`)
//! exactly as if it were `llama-server` / `mistralrs-server`, so the full
//! spawn → `/health` probe → proxy forward → bearer rewrite → SSE stream
//! path runs for real — only token generation is canned.
//!
//! ## Behaviour knobs (env is wiped by the deployment's `env_clear`, so
//! everything is driven through argv or the request body)
//!
//! - `--port <N>`        (required) bind 127.0.0.1:N
//! - `--api-key <TOK>`   (llama.cpp path) require `Authorization: Bearer
//!                       <TOK>`; when absent (mistral.rs path) accept any
//!                       request. A 200 round-trip therefore *proves* the
//!                       proxy rewrote the bearer to the per-instance token.
//! - any path argument containing the substring `stub-unhealthy` makes
//!   `/health` return 503 forever (drives the auto-start-timeout → 504 test).
//! - request-body field `"stub_hang_ms": N` on `/v1/chat/completions` sleeps
//!   N ms before responding (drives the drain / in-flight-blocks-reaper test).
//! - `--chunk-delay-ms <N>` paces the streaming deltas (N ms before each delta
//!   after the leading role chunk) so a chat turn is slow enough to be observed
//!   mid-flight / cancelled (drives the chat-stream cancel + replay tests).
//!
//! All other llama-server / mistralrs-server flags (`--model`,
//! `--model-path`, `--host`, `--ctx-size`, `--n-gpu-layers`, `--embeddings`,
//! `--model-type`, …) are ignored.

use std::convert::Infallible;
use std::time::Duration;

use axum::body::Bytes;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

#[derive(Clone)]
struct StubState {
    /// `Some` → require this bearer; `None` → accept any request.
    api_key: Option<String>,
    /// `/health` returns 503 when true.
    unhealthy: bool,
    /// Streaming `/v1/chat/completions`: sleep this long before each delta
    /// (the leading `role` chunk excepted) so generation is slow enough to be
    /// observed mid-flight / cancelled. Driven by `--chunk-delay-ms`.
    chunk_delay_ms: u64,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut port: u16 = 0;
    let mut api_key: Option<String> = None;
    let mut chunk_delay_ms: u64 = 0;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if let Some(v) = args.get(i + 1) {
                    port = v.parse().unwrap_or(0);
                    i += 1;
                }
            }
            "--api-key" => {
                api_key = args.get(i + 1).cloned();
                i += 1;
            }
            "--chunk-delay-ms" => {
                if let Some(v) = args.get(i + 1) {
                    chunk_delay_ms = v.parse().unwrap_or(0);
                    i += 1;
                }
            }
            // Every other flag (and its value) is ignored.
            _ => {}
        }
        i += 1;
    }

    if port == 0 {
        eprintln!("stub-engine: --port <N> is required");
        std::process::exit(2);
    }

    // Sentinel in any path-shaped argument (e.g. the `--model` value)
    // forces a permanently-unhealthy engine without needing env (which the
    // deployment clears).
    let unhealthy = args.iter().any(|a| a.contains("stub-unhealthy"));

    // The deployment pipes stdout into its log ring + SSE broadcast, so
    // this line is observable by the live-logs test. Rust's stdout is
    // line-buffered even when piped, so it flushes on the newline.
    println!(
        "stub-engine: listening on 127.0.0.1:{port} (auth={}, unhealthy={})",
        api_key.is_some(),
        unhealthy
    );

    let state = StubState {
        api_key,
        unhealthy,
        chunk_delay_ms,
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("stub-engine: bind 127.0.0.1");
    axum::serve(listener, app)
        .await
        .expect("stub-engine: serve");
}

async fn health(axum::extract::State(s): axum::extract::State<StubState>) -> Response {
    if s.unhealthy {
        return (StatusCode::SERVICE_UNAVAILABLE, "loading model").into_response();
    }
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"}))).into_response()
}

fn check_auth(s: &StubState, headers: &HeaderMap) -> bool {
    match &s.api_key {
        None => true,
        Some(key) => headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == format!("Bearer {key}"))
            .unwrap_or(false),
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "error": {"message": "invalid api key", "type": "authentication_error"}
        })),
    )
        .into_response()
}

async fn models(
    axum::extract::State(s): axum::extract::State<StubState>,
    headers: HeaderMap,
) -> Response {
    if !check_auth(&s, &headers) {
        return unauthorized();
    }
    Json(serde_json::json!({
        "object": "list",
        "data": [{"id": "stub-model", "object": "model", "owned_by": "stub"}]
    }))
    .into_response()
}

async fn embeddings(
    axum::extract::State(s): axum::extract::State<StubState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !check_auth(&s, &headers) {
        return unauthorized();
    }
    let model = model_field(&body);
    println!("stub-engine: POST /v1/embeddings model={model}");
    Json(serde_json::json!({
        "object": "list",
        "model": model,
        "data": [{"object": "embedding", "index": 0, "embedding": [0.01, 0.02, 0.03, 0.04]}],
        "usage": {"prompt_tokens": 1, "total_tokens": 1}
    }))
    .into_response()
}

async fn chat_completions(
    axum::extract::State(s): axum::extract::State<StubState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !check_auth(&s, &headers) {
        return unauthorized();
    }
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    let model = v
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("stub-model")
        .to_string();
    let stream = v.get("stream").and_then(|b| b.as_bool()).unwrap_or(false);
    let hang_ms = v.get("stub_hang_ms").and_then(|n| n.as_u64()).unwrap_or(0);
    let force_status = v.get("stub_force_status").and_then(|n| n.as_u64());

    println!("stub-engine: POST /v1/chat/completions model={model} stream={stream} hang_ms={hang_ms}");

    if hang_ms > 0 {
        tokio::time::sleep(Duration::from_millis(hang_ms)).await;
    }

    // Force a specific upstream status (drives the proxy status-passthrough
    // test). The engine's error body is returned verbatim by the proxy.
    if let Some(code) = force_status {
        let status = StatusCode::from_u16(code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        return (
            status,
            Json(serde_json::json!({
                "error": {"message": "forced upstream status", "type": "server_error"}
            })),
        )
            .into_response();
    }

    if stream {
        let events = vec![
            sse_chunk(&model, serde_json::json!({"role": "assistant"}), None),
            sse_chunk(&model, serde_json::json!({"content": "Hello"}), None),
            sse_chunk(&model, serde_json::json!({"content": " from stub"}), Some("stop")),
            Event::default().data("[DONE]"),
        ];
        let delay = s.chunk_delay_ms;
        // `unfold` lets each emission `.await` a sleep, pacing the deltas so a
        // test can subscribe / cancel mid-stream. The leading role chunk
        // (index 0) is sent immediately; every later chunk waits `delay`.
        let stream = futures::stream::unfold(
            (events.into_iter().enumerate(), delay),
            |(mut iter, delay)| async move {
                let (idx, ev) = iter.next()?;
                if delay > 0 && idx > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                Some((Ok::<Event, Infallible>(ev), (iter, delay)))
            },
        );
        return Sse::new(stream).into_response();
    }

    Json(serde_json::json!({
        "id": "chatcmpl-stub",
        "object": "chat.completion",
        "model": model,
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Hello from stub"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 3, "total_tokens": 4}
    }))
    .into_response()
}

fn sse_chunk(model: &str, delta: serde_json::Value, finish: Option<&str>) -> Event {
    let data = serde_json::json!({
        "id": "chatcmpl-stub",
        "object": "chat.completion.chunk",
        "model": model,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}]
    });
    Event::default().data(data.to_string())
}

fn model_field(body: &Bytes) -> String {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str()).map(String::from))
        .unwrap_or_else(|| "stub-model".to_string())
}

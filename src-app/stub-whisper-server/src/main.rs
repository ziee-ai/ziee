//! Minimal stub standing in for `whisper-server` in voice-runtime tests.
//!
//! Launched by the *real* deployment path (`voice::deployment::local`) exactly
//! as if it were the fork-built `whisper-server`, so the full spawn → `/` health
//! probe → `/inference` forward path runs for real; only the transcript is
//! canned.
//!
//! ## Behaviour (env is wiped by the deployment's `env_clear`, so knobs come via
//! argv):
//! - `--port <N>`       (required) bind 127.0.0.1:N
//! - `--host <H>`       accepted + ignored (always binds 127.0.0.1)
//! - `-m <PATH>` / `--model <PATH>`  the model path; if it contains the substring
//!                      `stub-unhealthy`, `/` returns 503 forever (drives the
//!                      auto-start-timeout test)
//! - `-l <LANG>` / `--language <LANG>`  echoed back in the response `language`
//! - all other whisper-server flags are ignored.
//!
//! Endpoints:
//! - `GET  /`            health — 200 unless the model path is `stub-unhealthy`
//! - `POST /inference`   returns `{"text": "<canned>", "language": "<lang>"}`
//!   for any body (multipart or raw), so the transcribe path runs end-to-end.

use std::net::SocketAddr;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

/// The canned transcript. Integration tests assert a case-insensitive substring
/// of this phrase, proving the real spawn → forward → parse path worked.
const CANNED_TRANSCRIPT: &str = "the quick brown fox jumps over the lazy dog";

#[derive(Clone)]
struct StubState {
    unhealthy: bool,
    language: String,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u16 = 0;
    let mut model_path = String::new();
    let mut language = String::from("en");

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                if let Some(v) = args.get(i + 1) {
                    port = v.parse().unwrap_or(0);
                    i += 1;
                }
            }
            "-m" | "--model" => {
                if let Some(v) = args.get(i + 1) {
                    model_path = v.clone();
                    i += 1;
                }
            }
            "-l" | "--language" => {
                if let Some(v) = args.get(i + 1) {
                    language = v.clone();
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    println!("stub-whisper-server: argv: {}", args[1..].join(" "));

    let state = StubState {
        unhealthy: model_path.contains("stub-unhealthy"),
        language,
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/inference", post(inference))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    println!(
        "stub-whisper-server: listening on {}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.expect("serve");
}

async fn health(
    axum::extract::State(state): axum::extract::State<StubState>,
) -> impl IntoResponse {
    if state.unhealthy {
        (StatusCode::SERVICE_UNAVAILABLE, "unhealthy")
    } else {
        (StatusCode::OK, "ok")
    }
}

async fn inference(
    axum::extract::State(state): axum::extract::State<StubState>,
    _body: axum::body::Bytes,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "text": CANNED_TRANSCRIPT,
        "language": state.language,
    }))
}

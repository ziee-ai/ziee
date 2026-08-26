//! TEST-10 — the download progress stream must not go silent.
//!
//! `GET /api/llm-models/downloads/subscribe` was the ONLY SSE route in the tree
//! without `KeepAlive` (`chat/stream/handler.rs`, the framework's
//! `sync/routes.rs`, `hardware/handlers.rs`, voice, workflow and code_sandbox
//! all set it). A download stream is idle by design between progress ticks, and
//! completely silent once the monitor loop exits — so with no heartbeat there is
//! nothing on the wire to distinguish "waiting" from "dead", and any
//! intermediary on the path (the ngrok tunnel this app supports, a reverse
//! proxy) is entitled to reap it.
//!
//! Asserted by READING THE WIRE rather than by checking the builder was called:
//! what matters is that a byte reaches the client, and `keep_alive(...)` not
//! being wired to the response is exactly the failure this must catch.
//!
//! Cost: axum's default keep-alive interval is 15s, so this test spends ~16s
//! waiting. That is the whole point — a shorter assertion would not be observing
//! the behaviour (DEC-12).

use std::time::Duration;

use tokio_stream::StreamExt;

/// Axum's `KeepAlive::default()` interval, plus headroom for a loaded box.
const KEEPALIVE_WAIT: Duration = Duration::from_secs(25);

#[tokio::test]
async fn download_progress_stream_sends_keepalives_while_idle() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "download_keepalive_watcher",
        &["llm_models::downloads_read"],
    )
    .await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/llm-models/downloads/subscribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("download progress subscribe request failed");
    assert_eq!(
        resp.status(),
        200,
        "an authenticated subscriber should get a 200 SSE stream"
    );

    // Read until a keep-alive comment frame appears. SSE comments are lines
    // starting with `:` — axum writes `:\n\n`. The `connected` handshake and any
    // `complete` event are ordinary frames and are skipped.
    let mut stream = resp.bytes_stream();
    let mut saw_keepalive = false;
    let mut transcript = String::new();
    let deadline = tokio::time::Instant::now() + KEEPALIVE_WAIT;

    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                transcript.push_str(&String::from_utf8_lossy(&chunk));
                if transcript.lines().any(|l| l.starts_with(':')) {
                    saw_keepalive = true;
                    break;
                }
            }
            // The stream ENDING is itself a failure here: an idle subscriber
            // must be kept, not dropped.
            Ok(Some(Err(e))) => panic!("download progress stream errored while idle: {e}"),
            Ok(None) => panic!(
                "download progress stream CLOSED while idle; transcript so far: {transcript:?}"
            ),
            Err(_) => break, // deadline
        }
    }

    assert!(
        saw_keepalive,
        "no SSE keep-alive comment arrived within {KEEPALIVE_WAIT:?} — an idle \
         download stream is silent, so a proxy or tunnel on the path can reap it \
         without either end noticing. Transcript: {transcript:?}"
    );
}

/// Keeps the enumerated id greppable in the test this branch added (A11).
const _TEST_ID: &str = "TEST-10";

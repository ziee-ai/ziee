//! Rate-limit (tower-governor) on/off regression tests.
//!
//! Bug: the built-in code_sandbox + memory MCP servers are reached over
//! loopback (127.0.0.1), so they share the same `PeerIpKeyExtractor` bucket as
//! all other traffic. A rapid agent tool loop drained that bucket and the
//! server returned HTTP 429 to itself ("Too Many Requests! Wait for Xs"). The
//! fix makes the limiter an operator-controlled on/off switch
//! (`server.rate_limit.enabled`).
//!
//! These tests verify the toggle end-to-end through the real spawned server:
//!   * enabled  + tiny caps  -> a request burst eventually gets 429
//!   * disabled + tiny caps  -> the same burst never gets 429
//!
//! The governor runs as an outer layer *before* handler auth, so an
//! unauthenticated GET cleanly distinguishes 429 (throttled) from 401
//! (reached the handler).

use crate::common::{TestServer, TestServerOptions};

/// An existing route. We don't authenticate — the governor rejects with 429
/// before auth runs, and a non-throttled request just gets 401. We only ever
/// assert on the presence/absence of 429.
const PROBE_PATH: &str = "/mcp/servers";

#[tokio::test]
async fn rate_limit_enabled_throttles_burst() {
    let server = TestServer::start_with_options(TestServerOptions {
        rate_limit: Some((true, 1, 2)), // enabled, 1 req/s, burst 2
        ..Default::default()
    })
    .await;

    let url = server.api_url(PROBE_PATH);
    let client = reqwest::Client::new();

    let mut got_429 = false;
    for _ in 0..20 {
        let resp = client.get(&url).send().await.expect("request failed");
        if resp.status() == 429 {
            got_429 = true;
            break;
        }
    }

    assert!(
        got_429,
        "with the limiter enabled (1 req/s, burst 2), a 20-request burst from \
         one peer IP must produce at least one HTTP 429"
    );
}

#[tokio::test]
async fn rate_limit_disabled_never_throttles_burst() {
    let server = TestServer::start_with_options(TestServerOptions {
        rate_limit: Some((false, 1, 2)), // DISABLED — caps are ignored
        ..Default::default()
    })
    .await;

    let url = server.api_url(PROBE_PATH);
    let client = reqwest::Client::new();

    for i in 0..40 {
        let resp = client.get(&url).send().await.expect("request failed");
        assert_ne!(
            resp.status(),
            429,
            "with the limiter disabled, no request may be throttled (request #{i})"
        );
    }
}

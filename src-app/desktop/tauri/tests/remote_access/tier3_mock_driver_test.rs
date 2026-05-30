//! Tier 3 — full tunnel-start happy path against the MockTunnelDriver.
//!
//! Exercises code paths that the Tier 2 precondition tests don't reach
//! (401/403/422 fire before the driver gets called). With the mock
//! substituted via `ZIEE_REMOTE_ACCESS_MOCK_TUNNEL=1` (set before
//! TestServer starts), `POST /tunnel/start` actually returns a fake
//! public URL, and `GET /status` reports `tunnel_state=connected`.
//!
//! The env var sentinel is the public injection point — see
//! `modules::remote_access::state::MOCK_TUNNEL_ENV`. The test sets it
//! ONCE before the first test in this file uses it, then leaves it
//! set (the process is short-lived and the next process will start
//! fresh).
//!
//! Note: ordering matters. The first test that calls
//! `tunnel_driver()` wins the OnceLock. We set the env var
//! pre-emptively in the module's `#[ctor]`-like static init so even
//! tests running before this file's tests see the mock.

use serde_json::{Value, json};

/// Set the mock-driver env var as early as possible — at the very
/// first reference to anything in this module. The static is
/// initialized lazily on first access; our tests force the access by
/// referencing `INIT_MOCK_DRIVER`.
static INIT_MOCK_DRIVER: std::sync::OnceLock<()> = std::sync::OnceLock::new();
fn ensure_mock_driver() {
    INIT_MOCK_DRIVER.get_or_init(|| {
        // SAFETY: setting an env var is safe before any background
        // threads access it; the TestServer hasn't booted yet at this
        // point in test discovery.
        unsafe {
            std::env::set_var("ZIEE_REMOTE_ACCESS_MOCK_TUNNEL", "1");
        }
    });
}

#[tokio::test]
async fn tunnel_start_mock_success_returns_public_url() {
    ensure_mock_driver();
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tier3_mock_start",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    // Save a fake token first — the tunnel-start handler refuses
    // (422) if no token is configured even with the mock driver,
    // because that gate runs in the handler before reaching the
    // driver. The mock driver ignores token value.
    let res = reqwest::Client::new()
        .put(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "ngrok_auth_token": "fake-mock-token" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Start the tunnel via the mock driver.
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        status, 200,
        "mock-driver start should succeed; body: {}",
        body
    );
    let public_url = body["public_url"]
        .as_str()
        .expect("public_url string in response");
    assert!(
        public_url.starts_with("https://mock-") && public_url.ends_with(".ngrok-mock.test"),
        "mock driver should return a synthetic URL pattern; got: {}",
        public_url
    );

    // Status should now report connected.
    let res = reqwest::Client::new()
        .get(server.api_url("/remote-access/status"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["tunnel_state"], "connected",
        "status after mock start should be connected; body: {}",
        body
    );
    assert!(
        body["public_url"].as_str().is_some(),
        "status should include public_url after start"
    );
}

#[tokio::test]
async fn tunnel_start_mock_with_domain_returns_domain_url() {
    ensure_mock_driver();
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tier3_mock_domain",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    // Save token + domain.
    let res = reqwest::Client::new()
        .put(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "ngrok_auth_token": "fake-mock-token",
            "ngrok_domain": "my-mock-app.ngrok.app",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // Stop any tunnel left over from a previous test in the same
    // process (the mock driver is a global singleton). Idempotent.
    let _ = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await;

    // Start — mock should honor the domain and return it.
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body: Value = res.json().await.unwrap();
    assert_eq!(status, 200, "{}", body);
    assert_eq!(
        body["public_url"], "https://my-mock-app.ngrok.app",
        "mock driver should return the configured domain verbatim"
    );

    // Cleanup so other tests aren't surprised by a still-running
    // mock tunnel (the OnceLock-singleton driver lives the whole test
    // process).
    let _ = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await;
}

#[tokio::test]
async fn tunnel_start_twice_returns_409_on_mock() {
    ensure_mock_driver();
    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tier3_mock_already",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    // Make sure no prior test left a tunnel up.
    let _ = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await;

    let _ = reqwest::Client::new()
        .put(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "ngrok_auth_token": "fake-mock-token" }))
        .send()
        .await
        .unwrap();

    let first = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200, "first start should succeed");

    // Second start without intervening stop → 409.
    let second = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        second.status(),
        409,
        "second start should report ALREADY_RUNNING via 409"
    );

    // Cleanup.
    let _ = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await;
}

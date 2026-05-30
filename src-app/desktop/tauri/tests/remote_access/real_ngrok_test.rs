//! Tier 8 — real-ngrok integration test.
//!
//! Reaches out to ngrok's edge with a real auth token, opens a tunnel
//! against the running TestServer, and confirms an HTTP request to
//! the public URL reaches our `/api/auth/config` handler. Marked
//! `#[ignore]` so it doesn't run on every `cargo test`; opt in via:
//!
//!     source .ngrok-test-credentials.env
//!     cargo test --test integration_tests \
//!         remote_access::real_ngrok -- --ignored --test-threads=1
//!
//! Required env:
//!   - NGROK_AUTH_TOKEN: ngrok account auth token
//!   - NGROK_TEST_DOMAIN: optional reserved domain (must be owned by
//!     the same account). Pass either a domain name
//!     ("my-app.ngrok.app") OR a domain ID ("rd_…"). If unset, the
//!     test uses an ephemeral *.ngrok-free.app URL.

use serde_json::json;
use std::time::Duration;

/// Helper: read env or return None (test skips silently when unset).
fn env_or_skip(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

#[tokio::test]
#[ignore = "needs NGROK_AUTH_TOKEN; run via just check-remote-access-real-ngrok"]
async fn real_ngrok_round_trip() {
    let Some(token) = env_or_skip("NGROK_AUTH_TOKEN") else {
        eprintln!("NGROK_AUTH_TOKEN not set; skipping real-ngrok test");
        return;
    };
    let domain = env_or_skip("NGROK_TEST_DOMAIN");

    let server = crate::common::TestServer::start_desktop().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ra_admin_real",
        &["remote_access::read", "remote_access::manage"],
    )
    .await;

    // Save the auth token + optional domain.
    let body = if let Some(ref d) = domain {
        json!({ "ngrok_auth_token": token, "ngrok_domain": d })
    } else {
        json!({ "ngrok_auth_token": token })
    };
    let res = reqwest::Client::new()
        .put(server.api_url("/remote-access/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&body)
        .send()
        .await
        .expect("save settings");
    assert_eq!(res.status(), 200, "save settings should succeed");

    // Start the tunnel.
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/start"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("start tunnel");
    let status = res.status();
    let body: serde_json::Value = res.json().await.expect("start tunnel JSON");
    if !status.is_success() {
        panic!(
            "tunnel start failed (status {}): {}",
            status,
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );
    }
    let public_url = body["public_url"]
        .as_str()
        .expect("public_url in response")
        .to_string();
    eprintln!("tunnel up at {}", public_url);

    // Give ngrok a few seconds to fully register the tunnel before
    // we try HTTPing through it.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // HTTP-GET the public URL's /api/auth/config endpoint. It's
    // unauthenticated, so we should be able to reach it without
    // any headers; the response confirms the tunnel actually
    // proxies to our local server.
    let res = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
        .get(format!("{}/api/auth/config", public_url))
        .send()
        .await
        .expect("GET through tunnel");
    assert_eq!(
        res.status(),
        200,
        "tunnel should proxy /api/auth/config to local server"
    );
    let auth_config: serde_json::Value = res.json().await.expect("auth_config JSON");
    // Through the tunnel, Host is the ngrok domain → server returns
    // hide_username=true.
    assert_eq!(
        auth_config["hide_username"], true,
        "tunneled request should set hide_username=true: {}",
        serde_json::to_string_pretty(&auth_config).unwrap_or_default()
    );

    // Stop the tunnel. Should succeed even on the live session.
    let res = reqwest::Client::new()
        .post(server.api_url("/remote-access/tunnel/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("stop tunnel");
    assert_eq!(res.status(), 204);
}

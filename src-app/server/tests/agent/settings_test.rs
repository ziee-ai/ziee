//! TEST-20 / TEST-21 — agent admin-settings REST surface (`/api/agent/settings`).
//! Roundtrip + bounds validation (400) + sync emit, and the auth gate (401/403).

use std::time::Duration;

use serde_json::json;

use crate::common::sync_probe::SyncProbe;
use crate::common::TestServer;

const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

async fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// TEST-20 — GET/PUT roundtrip, bounds → 400, and the AgentAdminSettings sync emit.
#[tokio::test]
async fn agent_settings_roundtrip_bounds_and_sync() {
    let server = TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "agent_settings_admin",
        &["agent::settings::read", "agent::settings::manage"],
    )
    .await;

    // GET returns the singleton (default) row.
    let resp = client()
        .await
        .get(server.api_url("/agent/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(
        body.get("default_sandbox_mode").is_some(),
        "settings body should carry default_sandbox_mode: {body}"
    );

    // A valid PUT updates + emits a sync event.
    let mut probe = SyncProbe::open(&server, &admin.token).await;
    let put = client()
        .await
        .put(server.api_url("/agent/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_max_steps": 42, "per_run_token_cap": 500_000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200, "valid PUT should succeed");
    probe
        .expect_event("agent_admin_settings", "update", EVENT_TIMEOUT)
        .await;

    // Roundtrip: the new value is persisted + read back.
    let got: serde_json::Value = client()
        .await
        .get(server.api_url("/agent/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got["default_max_steps"].as_i64(), Some(42));

    // Bounds: out-of-range + a bad enum → 400.
    for bad in [
        json!({ "default_max_steps": 99_999 }),
        json!({ "per_run_token_cap": 5 }),
        json!({ "default_sandbox_mode": "bogus-mode" }),
        json!({ "fan_out_max_threads": 999 }),
    ] {
        let r = client()
            .await
            .put(server.api_url("/agent/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&bad)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400, "invalid PUT {bad} must be rejected with 400");
    }
}

// TEST-21 — the auth gate: 401 unauthenticated, 403 without the permission.
#[tokio::test]
async fn agent_settings_requires_permission() {
    let server = TestServer::start().await;

    // Unauthenticated → 401.
    let unauth = client()
        .await
        .get(server.api_url("/agent/settings"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauth.status(), 401);

    // A user lacking agent::settings::read → 403 on GET.
    let noperm = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "agent_settings_noperm",
        &["profile::read"],
    )
    .await;
    let forbidden = client()
        .await
        .get(server.api_url("/agent/settings"))
        .header("Authorization", format!("Bearer {}", noperm.token))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);

    // A user with READ but not MANAGE → 403 on PUT.
    let reader = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "agent_settings_reader",
        &["agent::settings::read"],
    )
    .await;
    let put_forbidden = client()
        .await
        .put(server.api_url("/agent/settings"))
        .header("Authorization", format!("Bearer {}", reader.token))
        .json(&json!({ "default_max_steps": 10 }))
        .send()
        .await
        .unwrap();
    assert_eq!(put_forbidden.status(), 403);
}

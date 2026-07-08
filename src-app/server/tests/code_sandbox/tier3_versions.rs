//! Tier 3 — HTTP integration tests for the rootfs **version** admin
//! surface (Plan 5). Replaces the removed `tier3_prefetch` +
//! `tier3_environments` suites, which exercised the legacy
//! `/environments` + `/prefetch` endpoints that no longer exist.
//!
//! Endpoints under test (all under `/api/code-sandbox/rootfs/versions`):
//!   * `GET    /`                 — list (perm: environments::read)
//!   * `POST   /install`          — start download (perm: …::manage)
//!   * `GET    /install/subscribe`— SSE progress (perm: …::read)
//!   * `POST   /set-pin`          — change the pin (perm: …::manage)
//!   * `DELETE /{id}`             — delete artifact (perm: …::manage)
//!
//! These tests run against the DEFAULT harness (`code_sandbox.enabled:
//! false`), so they cover the layers that are independent of a mounted
//! rootfs / network / bwrap:
//!   - authentication (401) and the read-vs-manage permission split
//!     (403) — enforced by the `RequirePermissions` extractor BEFORE
//!     the handler body, so they hold regardless of sandbox state;
//!   - request validation (422) on install/set-pin — `validate_install_
//!     request` runs BEFORE the sandbox-initialized `live_pool()` gate;
//!   - the SSE subscribe happy path — `subscribe_install_progress_handler`
//!     never touches `live_pool()`, so it returns 200 + a `connected`
//!     event even with the sandbox disabled.
//!
//! The 200 happy paths for list/install/set-pin/delete require an
//! initialized sandbox (enabled config + DB pool) and are exercised by
//! the Tier-6 HTTP-E2E suite + the `version_manager` unit tests. We
//! deliberately do NOT assert the exact list/install status here because
//! `code_sandbox::config::STATE` is a process-wide `OnceCell` — a
//! sandbox-enabled test elsewhere in the same binary can leave it set,
//! making a 200-vs-503 assertion order-dependent.

use std::time::Duration;

use serde_json::{json, Value};

use crate::common::{test_helpers, TestServer};

// ---------------------------------------------------------------------
// URL + user helpers
// ---------------------------------------------------------------------

fn versions_url(server: &TestServer, suffix: &str) -> String {
    format!("{}/api/code-sandbox/rootfs/versions{}", server.base_url, suffix)
}

/// read + manage on environments.
async fn user_with_manage(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "rootfs_manage",
        &[
            "code_sandbox::environments::read",
            "code_sandbox::environments::manage",
        ],
    )
    .await
    .token
}

/// read only — accepted on read endpoints, rejected (403) on manage ones.
async fn user_with_read_only(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "rootfs_read",
        &["code_sandbox::environments::read"],
    )
    .await
    .token
}

/// Authenticated, but holds an UNRELATED permission — lacks both
/// environments scopes, so every rootfs-version endpoint 403s.
async fn user_without_env_perm(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "rootfs_nope",
        &["code_sandbox::resource_limits::read"],
    )
    .await
    .token
}

fn valid_install_body() -> Value {
    json!({
        "version": "1.2.3",
        "arch": "x86_64",
        "flavor": "minimal",
        "package": "squashfs",
    })
}

// =====================================================================
// GET /rootfs/versions  (perm: environments::read)
// =====================================================================

#[tokio::test]
async fn list_versions_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .get(versions_url(&server, ""))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn list_versions_requires_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_env_perm(&server).await;
    let resp = reqwest::Client::new()
        .get(versions_url(&server, ""))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

/// A user WITH environments::read passes the permission gate AND the LIST
/// path degrades gracefully: it returns **200** with the GitHub catalog +
/// a machine-readable `availability` reason instead of a blanket 503. The
/// default harness leaves `code_sandbox.enabled: false`, and each test runs
/// its own fresh server subprocess, so the recorded reason is deterministic:
/// `disabled_in_config`.
#[tokio::test]
async fn list_versions_passes_permission_gate_for_reader() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;
    let resp = reqwest::Client::new()
        .get(versions_url(&server, ""))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "the LIST path degrades to 200 even when the sandbox is disabled"
    );
    let body: Value = resp.json().await.expect("json body");
    assert_eq!(
        body["availability"], "disabled_in_config",
        "a disabled sandbox reports its reason so the UI can degrade gracefully"
    );
    assert!(
        body["available"].is_array(),
        "the GitHub catalog is always an array (possibly empty when offline)"
    );
}

/// The degraded LIST response carries the GitHub catalog but an EMPTY
/// installed/pinned set (they need the DB/state), and only the LIST path
/// degrades: the mutating `install` path still hard-gates on a live pool
/// and returns 503 `SANDBOX_NOT_INITIALIZED`.
#[tokio::test]
async fn list_versions_degraded_returns_available_when_disabled() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let list = reqwest::Client::new()
        .get(versions_url(&server, ""))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list request");
    assert_eq!(list.status().as_u16(), 200);
    let body: Value = list.json().await.expect("json body");
    assert_eq!(body["availability"], "disabled_in_config");
    assert_eq!(
        body["installed"].as_array().map(|a| a.len()),
        Some(0),
        "nothing is installed while the sandbox is disabled"
    );
    assert!(
        body["pinned_version"].is_null(),
        "no pin is surfaced while the sandbox is disabled"
    );
    assert!(body["available"].is_array());

    // The mutating path is unchanged — a valid install request clears
    // validation but then hits the live-pool gate → 503.
    let install = reqwest::Client::new()
        .post(versions_url(&server, "/install"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&valid_install_body())
        .send()
        .await
        .expect("install request");
    assert_eq!(
        install.status().as_u16(),
        503,
        "install still requires an initialized sandbox"
    );
    let ierr: Value = install.json().await.expect("json body");
    assert_eq!(ierr["error_code"], "SANDBOX_NOT_INITIALIZED");
}

// =====================================================================
// POST /rootfs/versions/install  (perm: environments::manage)
// =====================================================================

#[tokio::test]
async fn install_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(versions_url(&server, "/install"))
        .json(&valid_install_body())
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn install_requires_manage_permission() {
    let server = TestServer::start().await;
    // read-only token has environments::read but NOT ::manage.
    let token = user_with_read_only(&server).await;
    let resp = reqwest::Client::new()
        .post(versions_url(&server, "/install"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&valid_install_body())
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn install_rejects_invalid_requests_with_422() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    let client = reqwest::Client::new();

    // Each body is well-formed JSON with every required field present —
    // so the Json extractor succeeds and `validate_install_request` is
    // what rejects it (422), exercising the validator rather than the
    // extractor's missing-field path.
    let bad_bodies = [
        ("two-part version", json!({"version":"1.2","arch":"x86_64","flavor":"minimal","package":"squashfs"})),
        ("leading-zero version", json!({"version":"01.2.3","arch":"x86_64","flavor":"minimal","package":"squashfs"})),
        ("leading-zero prerelease", json!({"version":"1.2.3-rc.01","arch":"x86_64","flavor":"minimal","package":"squashfs"})),
        ("unknown arch", json!({"version":"1.2.3","arch":"arm","flavor":"minimal","package":"squashfs"})),
        ("unknown package", json!({"version":"1.2.3","arch":"x86_64","flavor":"minimal","package":"zip"})),
        ("flavor with slash", json!({"version":"1.2.3","arch":"x86_64","flavor":"bad/flavor","package":"squashfs"})),
        ("empty flavor", json!({"version":"1.2.3","arch":"x86_64","flavor":"","package":"squashfs"})),
    ];

    for (label, body) in bad_bodies {
        let resp = client
            .post(versions_url(&server, "/install"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
            .expect("request");
        assert_eq!(
            resp.status().as_u16(),
            422,
            "expected 422 for {label}; got {:?}",
            resp.text().await
        );
    }
}

// =====================================================================
// POST /rootfs/versions/set-pin  (perm: environments::manage)
// =====================================================================

#[tokio::test]
async fn set_pin_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(versions_url(&server, "/set-pin"))
        .json(&json!({ "version": "1.2.3" }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn set_pin_requires_manage_permission() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;
    let resp = reqwest::Client::new()
        .post(versions_url(&server, "/set-pin"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "version": "1.2.3" }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

#[tokio::test]
async fn set_pin_rejects_invalid_version_with_422() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;
    let client = reqwest::Client::new();
    for bad in ["1.2", "01.2.3", "v1.2.3", "1.2.3-rc.01", "not-a-version"] {
        let resp = client
            .post(versions_url(&server, "/set-pin"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "version": bad }))
            .send()
            .await
            .expect("request");
        assert_eq!(
            resp.status().as_u16(),
            422,
            "expected 422 for version={bad:?}; got {:?}",
            resp.text().await
        );
    }
}

// =====================================================================
// DELETE /rootfs/versions/{id}  (perm: environments::manage)
// =====================================================================

#[tokio::test]
async fn delete_requires_authentication() {
    let server = TestServer::start().await;
    let id = uuid::Uuid::new_v4();
    let resp = reqwest::Client::new()
        .delete(versions_url(&server, &format!("/{id}")))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn delete_requires_manage_permission() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;
    let id = uuid::Uuid::new_v4();
    let resp = reqwest::Client::new()
        .delete(versions_url(&server, &format!("/{id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

// =====================================================================
// GET /rootfs/versions/install/subscribe  (SSE, perm: environments::read)
// =====================================================================

#[tokio::test]
async fn subscribe_requires_authentication() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .get(versions_url(&server, "/install/subscribe"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn subscribe_requires_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_env_perm(&server).await;
    let resp = reqwest::Client::new()
        .get(versions_url(&server, "/install/subscribe"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 403);
}

/// The SSE handler never calls `live_pool()`, so a reader gets a live
/// stream regardless of whether the sandbox is initialized: 200, the
/// `text/event-stream` content type, the `X-Accel-Buffering: no`
/// proxy-buffering opt-out (audit Net2), and a first `connected` event.
#[tokio::test]
async fn subscribe_streams_connected_event_for_reader() {
    let server = TestServer::start().await;
    let token = user_with_read_only(&server).await;
    let mut resp = reqwest::Client::new()
        .get(versions_url(&server, "/install/subscribe"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status().as_u16(), 200, "expected SSE 200");
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(
        ct.starts_with("text/event-stream"),
        "expected text/event-stream content-type; got {ct:?}"
    );
    assert_eq!(
        resp.headers()
            .get("x-accel-buffering")
            .and_then(|v| v.to_str().ok()),
        Some("no"),
        "SSE response must carry X-Accel-Buffering: no"
    );

    // Read the first chunk (the handshake) under a timeout — the stream
    // stays open indefinitely, so we must not `.text()` the whole body.
    let chunk = tokio::time::timeout(Duration::from_secs(5), resp.chunk())
        .await
        .expect("timed out waiting for first SSE event")
        .expect("stream io error")
        .expect("stream closed before any event");
    let text = String::from_utf8_lossy(&chunk);
    assert!(
        text.contains("connected"),
        "first SSE event should be `connected`; got: {text:?}"
    );
}

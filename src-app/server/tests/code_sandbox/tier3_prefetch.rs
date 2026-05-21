//! Tier 3 — HTTP integration tests for the prefetch REST + SSE
//! surface (`/api/code-sandbox/environments`,
//! `/api/code-sandbox/prefetch`, `/api/code-sandbox/prefetch/{flavor}/events`).
//!
//! These boot a TestServer with sandbox **disabled** (default) so the
//! handlers exercise the full route stack — auth, JSON deserialization,
//! permission checks, response serialization — but the cache-dir lookup
//! falls back to a CWD-relative path that's almost certainly empty.
//! That makes `cached: false` the expected state for every flavor here,
//! which is exactly what we want for asserting the "auto-fetch will
//! trigger" branch.
//!
//! Tests that need the prefetch task to ACTUALLY download against a
//! local mirror belong in Tier 6 (out of scope here — they need the
//! dev-release.sh mirror running).
//!
//! Permission test pattern: `create_user_with_permissions(...)` grants
//! the requested perms via a test group; a user with NO matching perms
//! must hit 403 on the protected endpoints.

use serde_json::Value;
use uuid::Uuid;

use crate::common::{test_helpers, TestServer};

fn url(server: &TestServer, path: &str) -> String {
    format!("{}/api/code-sandbox{}", server.base_url, path)
}

async fn user_with_read(server: &TestServer) -> String {
    test_helpers::create_user_with_permissions(
        server,
        "prefetch_read",
        &["code_sandbox::environments::read"],
    )
    .await
    .token
}

async fn user_with_manage(server: &TestServer) -> String {
    // Manage perm alone is sufficient for POST; read is sufficient for
    // GET (including the SSE stream). Most realistic admin will have
    // both — the tests below grant whichever set the endpoint needs.
    test_helpers::create_user_with_permissions(
        server,
        "prefetch_manage",
        &["code_sandbox::environments::manage", "code_sandbox::environments::read"],
    )
    .await
    .token
}

async fn user_without_prefetch_perms(server: &TestServer) -> String {
    // Register a user but grant only code_sandbox::execute (the
    // existing perm, unrelated to env management). The new endpoints
    // should still reject this user.
    test_helpers::create_user_with_permissions(
        server,
        "prefetch_no_perm",
        &["code_sandbox::execute"],
    )
    .await
    .token
}

// =====================================================================
// GET /environments
// =====================================================================

#[tokio::test]
async fn environments_lists_minimal_and_full_with_cached_status() {
    let server = TestServer::start().await;
    let token = user_with_read(&server).await;

    let resp = reqwest::Client::new()
        .get(url(&server, "/environments"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200, "got {:?}", resp.text().await);
    let body: Value = resp.json().await.expect("parse");

    let available = body["available"].as_array().expect("available array");
    let flavors: Vec<&str> = available
        .iter()
        .filter_map(|e| e["flavor"].as_str())
        .collect();
    assert!(
        flavors.contains(&"minimal"),
        "expected 'minimal' in {flavors:?}"
    );
    assert!(
        flavors.contains(&"full"),
        "expected 'full' in {flavors:?}"
    );

    // Each entry shape: flavor + description + approximate_size_mb + cached.
    for e in available {
        assert!(e["flavor"].is_string(), "flavor missing: {e}");
        assert!(e["description"].is_string(), "description missing: {e}");
        assert!(
            e["approximate_size_mb"].is_number(),
            "approximate_size_mb missing: {e}"
        );
        assert!(e["cached"].is_boolean(), "cached missing: {e}");
    }
}

#[tokio::test]
async fn environments_requires_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_prefetch_perms(&server).await;

    let resp = reqwest::Client::new()
        .get(url(&server, "/environments"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "expected 403 for user without Read perm, got {:?}",
        resp.text().await
    );
}

#[tokio::test]
async fn environments_requires_authorization_header() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .get(url(&server, "/environments"))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 401, "expected 401 without bearer");
}

// =====================================================================
// POST /prefetch
// =====================================================================

#[tokio::test]
async fn start_prefetch_returns_task_id_and_events_url() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let resp = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("send");
    assert_eq!(
        resp.status().as_u16(),
        200,
        "got {:?}",
        resp.text().await
    );
    let body: Value = resp.json().await.expect("parse");
    assert!(body["task_id"].is_string(), "task_id missing: {body}");
    assert_eq!(body["flavor"], "minimal");
    assert_eq!(
        body["events_url"].as_str(),
        Some("/api/code-sandbox/prefetch/minimal/events")
    );
    assert!(body["expected_size_mb"].is_number());
    assert!(body["status"].is_string(), "status missing: {body}");
}

#[tokio::test]
async fn start_prefetch_concurrent_posts_both_succeed_with_valid_tasks() {
    // Two POSTs in flight for the same flavor must both succeed with
    // 200 + a structured response. The CONCURRENCY contract is
    // weaker than strict idempotency: if both calls observe the
    // DashMap entry as vacant (one wins the shard lock; the other
    // joins → same task_id), they share. If the first runner has
    // already reached terminal state (empty known_revisions.toml
    // makes the fetch fail in ~1 ms in this test setup), the second
    // POST correctly triggers a replacement task. Either outcome is
    // valid production behavior — what we pin is "both calls succeed
    // with a well-formed task envelope, and we never see two distinct
    // RUNNING tasks for the same flavor at once".
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let client = reqwest::Client::new();
    let body = serde_json::json!({ "flavor": "minimal" });

    let (r1, r2) = tokio::join!(
        client
            .post(url(&server, "/prefetch"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send(),
        client
            .post(url(&server, "/prefetch"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send(),
    );

    let b1: Value = r1.expect("send 1").json().await.expect("parse 1");
    let b2: Value = r2.expect("send 2").json().await.expect("parse 2");

    // Both must be well-formed responses (not error envelopes).
    for body in [&b1, &b2] {
        assert!(
            body["task_id"].is_string(),
            "concurrent POST returned malformed body: {body}"
        );
        assert_eq!(body["flavor"], "minimal");
    }

    // After the dust settles, the registry holds AT MOST ONE entry
    // per flavor (idempotent storage). Same-id OR replacement, but
    // the slot is single.
    let list_resp = client
        .get(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list send");
    let list_body: Value = list_resp.json().await.expect("list parse");
    let minimal_count = list_body["tasks"]
        .as_array()
        .map(|a| a.iter().filter(|t| t["flavor"] == "minimal").count())
        .unwrap_or(0);
    assert_eq!(
        minimal_count, 1,
        "expected exactly one minimal task entry after two concurrent POSTs; got list: {list_body}"
    );
}

#[tokio::test]
async fn start_prefetch_rejects_unknown_flavor() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    let resp = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "no-such-flavor" }))
        .send()
        .await
        .expect("send");
    assert_eq!(
        resp.status().as_u16(),
        400,
        "expected 400 for unknown flavor, got {:?}",
        resp.text().await
    );
    let body: Value = resp.json().await.expect("parse");
    // The error envelope (per AppError::IntoResponse) carries an
    // `error_code` field. Pin it so a rename surfaces here.
    assert_eq!(
        body["error_code"].as_str(),
        Some("SANDBOX_UNKNOWN_FLAVOR"),
        "expected SANDBOX_UNKNOWN_FLAVOR error_code: {body}"
    );
}

#[tokio::test]
async fn start_prefetch_requires_manage_permission() {
    let server = TestServer::start().await;
    let token_read_only = user_with_read(&server).await;

    let resp = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token_read_only}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("send");
    assert_eq!(
        resp.status().as_u16(),
        403,
        "expected 403 for user with only Read perm, got {:?}",
        resp.text().await
    );
}

#[tokio::test]
async fn start_prefetch_requires_authorization() {
    let server = TestServer::start().await;
    let resp = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 401);
}

// =====================================================================
// GET /prefetch (list tasks)
// =====================================================================

#[tokio::test]
async fn list_prefetch_tasks_returns_array_after_start() {
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    // Trigger a task so the list is non-empty.
    let _ = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("send");

    let resp = reqwest::Client::new()
        .get(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.expect("parse");
    let tasks = body["tasks"].as_array().expect("tasks array");
    assert!(
        tasks.iter().any(|t| t["flavor"] == "minimal"),
        "expected a minimal task in {tasks:?}"
    );
    // Status is one of the four PrefetchStatus variants.
    for t in tasks {
        let status = t["status"].as_str().expect("status string");
        assert!(
            ["running", "completed", "failed", "already_cached"]
                .contains(&status),
            "unknown status {status:?}: {t}"
        );
    }
}

#[tokio::test]
async fn list_prefetch_tasks_requires_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_prefetch_perms(&server).await;

    let resp = reqwest::Client::new()
        .get(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 403);
}

// =====================================================================
// GET /prefetch/{flavor}/events (SSE stream)
// =====================================================================

#[tokio::test]
async fn events_returns_404_when_no_task_exists() {
    // A fresh flavor with no prior POST: the registry has nothing
    // under that key, so the SSE handler returns 404.
    let server = TestServer::start().await;
    let token = user_with_read(&server).await;
    // Use a different flavor than other tests start to keep the
    // registry empty for this flavor key.
    let resp = reqwest::Client::new()
        .get(url(&server, "/prefetch/full/events"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(
        resp.status().as_u16(),
        404,
        "expected 404 for absent task, got {:?}",
        resp.text().await
    );
}

#[tokio::test]
async fn events_requires_read_permission() {
    let server = TestServer::start().await;
    let token = user_without_prefetch_perms(&server).await;

    let resp = reqwest::Client::new()
        .get(url(&server, "/prefetch/minimal/events"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    // Could be 403 (perm denied) or 404 (task absent). The key
    // assertion: NOT 200. We accept either since they both deny
    // access; 403 is preferred when the handler runs auth first.
    let s = resp.status().as_u16();
    assert!(
        s == 403,
        "expected 403 for user without Read perm, got {s}: {:?}",
        resp.text().await
    );
}

#[tokio::test]
async fn events_emits_connected_then_terminal_for_failing_fetch() {
    // Test setup: sandbox is DISABLED in TestServer config. Starting
    // a prefetch for `minimal` will resolve fine (the flavor IS in
    // KNOWN_FLAVORS) but the network fetch will fail fast (empty
    // known_revisions.toml → resolve error). The SSE stream should
    // emit `connected` first, then a `failed` event.
    let server = TestServer::start().await;
    let token = user_with_manage(&server).await;

    // POST start
    let post = reqwest::Client::new()
        .post(url(&server, "/prefetch"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("send");
    assert_eq!(post.status().as_u16(), 200);

    // Subscribe to SSE. Read the response body as a stream of bytes
    // and look for "event: connected" + a terminal event ("complete"
    // or "failed") within a tight deadline. The fetch fails fast
    // (~10 ms) since known_revisions.toml is empty in the binary.
    let resp = reqwest::Client::new()
        .get(url(&server, "/prefetch/minimal/events"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);

    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        resp.bytes(),
    )
    .await
    .expect("SSE response timeout")
    .expect("read body");
    let text = String::from_utf8_lossy(&bytes);

    assert!(
        text.contains("event: connected"),
        "expected 'event: connected' in SSE body, got:\n{text}"
    );
    assert!(
        text.contains("event: failed") || text.contains("event: complete"),
        "expected a terminal SSE event (failed/complete), got:\n{text}"
    );
}

// =====================================================================
// Sanity: tier3_prefetch entries appear in the dispatch graph
// (cheap check that the routes are actually mounted)
// =====================================================================

#[tokio::test]
async fn unmounted_routes_return_404_not_403() {
    // If `code_sandbox_router()` regressed (e.g. accidentally dropped
    // a route), an unauthenticated GET to that path would return 404
    // (route not found) rather than 401 (auth missing). Pin the
    // expected status to 401 so a future regression here is obvious.
    let server = TestServer::start().await;
    let client = reqwest::Client::new();

    for path in &["/environments", "/prefetch", "/prefetch/minimal/events"] {
        let resp = client.get(url(&server, path)).send().await.expect("send");
        assert_eq!(
            resp.status().as_u16(),
            401,
            "{path}: expected 401 (auth missing) — got {} — route may be missing",
            resp.status()
        );
    }
}

// silence unused-import warning when the file is included as a module
// but no tests run (e.g. cargo test with a filter that excludes prefetch)
#[allow(dead_code)]
fn _unused() {
    let _ = Uuid::nil();
}

// =====================================================================
// Success-path tests using mirror_fixture (download from a local
// http server serving a real .squashfs from .ziee-cache).
//
// These skip cleanly when bwrap or a built squashfs is absent. On
// a typical dev machine they exercise the full
//   POST /prefetch → spawn runner → download → sha256-verify
//   → atomic-install → SSE Complete event
// path end-to-end without needing a real GitHub release.
// =====================================================================

use crate::code_sandbox::mirror_fixture;

#[tokio::test]
async fn end_to_end_download_succeeds_and_emits_complete_with_bytes() {
    let Some(fixture) = mirror_fixture::setup("minimal").await else {
        return;
    };
    let token = test_helpers::create_user_with_permissions(
        &fixture.server,
        "e2e_prefetch_manage",
        &["code_sandbox::environments::manage", "code_sandbox::environments::read"],
    )
    .await
    .token;

    // POST /prefetch — runner starts; downloads from our mirror.
    let post = reqwest::Client::new()
        .post(format!("{}/api/code-sandbox/prefetch", fixture.server.base_url))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "flavor": "minimal" }))
        .send()
        .await
        .expect("post send");
    assert_eq!(post.status().as_u16(), 200, "got {:?}", post.text().await);
    let post_body: Value = post.json().await.expect("post parse");
    assert_eq!(post_body["status"], "running");

    // Subscribe to SSE — read the response body as a stream of bytes
    // and look for `event: complete` within a tight deadline.
    let resp = reqwest::Client::new()
        .get(format!(
            "{}/api/code-sandbox/prefetch/minimal/events",
            fixture.server.base_url
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("sse send");
    assert_eq!(resp.status().as_u16(), 200);

    let bytes = tokio::time::timeout(std::time::Duration::from_secs(30), resp.bytes())
        .await
        .expect("SSE timeout")
        .expect("read body");
    let text = String::from_utf8_lossy(&bytes);

    assert!(text.contains("event: connected"), "missing connected:\n{text}");
    assert!(
        text.contains("event: progress"),
        "expected at least one progress event:\n{text}"
    );
    assert!(
        text.contains("event: complete"),
        "expected terminal complete event (success path):\n{text}"
    );
    assert!(
        !text.contains("event: failed"),
        "fetch failed unexpectedly:\n{text}"
    );

    // The Complete event's data should report bytes_downloaded > 0
    // (we deliberately started with an empty test cache_dir).
    let complete_data_line = text
        .lines()
        .skip_while(|l| !l.starts_with("event: complete"))
        .nth(1)
        .expect("data line after complete event");
    assert!(
        complete_data_line.starts_with("data:"),
        "expected data line, got: {complete_data_line}"
    );
    let payload: Value = serde_json::from_str(
        complete_data_line.trim_start_matches("data:").trim(),
    )
    .expect("parse complete payload");
    let bytes_downloaded = payload["bytes_downloaded"]
        .as_u64()
        .expect("bytes_downloaded missing");
    assert!(
        bytes_downloaded > 1024,
        "expected a real download (>1 KiB), got {bytes_downloaded}: {payload}"
    );
}

#[tokio::test]
async fn end_to_end_second_post_after_install_reports_already_cached() {
    let Some(fixture) = mirror_fixture::setup("minimal").await else {
        return;
    };
    let token = test_helpers::create_user_with_permissions(
        &fixture.server,
        "e2e_prefetch_cached",
        &["code_sandbox::environments::manage", "code_sandbox::environments::read"],
    )
    .await
    .token;

    let client = reqwest::Client::new();
    let prefetch_url = format!("{}/api/code-sandbox/prefetch", fixture.server.base_url);
    let body = serde_json::json!({ "flavor": "minimal" });

    // 1st POST — kick off + drain its SSE so we know it finished.
    let _ = client
        .post(&prefetch_url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("post 1");
    let sse = client
        .get(format!(
            "{}/api/code-sandbox/prefetch/minimal/events",
            fixture.server.base_url
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("sse 1 send");
    let _ = tokio::time::timeout(std::time::Duration::from_secs(30), sse.bytes())
        .await
        .expect("sse 1 timeout");

    // 2nd POST — the cache_dir now has the squashfs; the runner
    // short-circuits via the idempotency path in runtime_fetch
    // (sha256 matches → returns immediately with bytes_downloaded=0).
    let post2 = client
        .post(&prefetch_url)
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("post 2");
    assert_eq!(post2.status().as_u16(), 200);

    // Drain SSE 2 to read the final event.
    let sse2 = client
        .get(format!(
            "{}/api/code-sandbox/prefetch/minimal/events",
            fixture.server.base_url
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("sse 2 send");
    let bytes2 = tokio::time::timeout(std::time::Duration::from_secs(10), sse2.bytes())
        .await
        .expect("sse 2 timeout")
        .expect("read body 2");
    let text2 = String::from_utf8_lossy(&bytes2);

    assert!(
        text2.contains("event: complete"),
        "second prefetch should complete (cached path):\n{text2}"
    );

    // The Complete event payload should report bytes_downloaded = 0
    // — proves the second POST took the cached-idempotency branch.
    let complete_data = text2
        .lines()
        .skip_while(|l| !l.starts_with("event: complete"))
        .nth(1)
        .expect("data line after complete");
    let payload: Value = serde_json::from_str(
        complete_data.trim_start_matches("data:").trim(),
    )
    .expect("parse complete payload");
    assert_eq!(
        payload["bytes_downloaded"].as_u64(),
        Some(0),
        "expected bytes_downloaded=0 on cached path, got: {payload}"
    );
}

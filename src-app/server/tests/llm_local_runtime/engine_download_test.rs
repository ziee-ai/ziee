//! Tier 2 — engine-binary download from the mock release repo.
//!
//! Exercises the FULL download pipeline (resolve → fetch → extract →
//! cache → register) against the loopback `MockReleaseServer`, plus
//! version CRUD + permissions.

use crate::common::TestServer;
use crate::common::test_helpers::{create_user_with_permissions, create_user_with_only_permissions};
use super::mock_release;
use super::test_helpers::{self as lrt, LOCAL_RUNTIME_ADMIN_PERMS};
use futures::StreamExt;
use reqwest::StatusCode;
use serde_json::json;
use std::time::{Duration, Instant};

/// The engine downloads from the mock, registers a version row, and
/// shows up in the (engine-filtered) version list. The previous
/// `allow_unsigned_downloads` supply-chain gate has been removed —
/// downloads now proceed unconditionally (cosign verify in the runtime
/// crate logs a warning when the sibling `.sig` is missing but no
/// longer blocks).
#[tokio::test]
async fn download_engine_from_mock_succeeds() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url("/local-runtime/versions?engine=llamacpp"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let versions = body["versions"].as_array().expect("versions array");
    assert!(
        versions
            .iter()
            .any(|v| v["id"].as_str() == Some(version_id.to_string().as_str())),
        "downloaded version should appear in the list: {body}"
    );
}

/// A re-download is idempotent (cache hit) and still returns 200.
/// With detached tasks, a re-POST joins the existing terminal task
/// (or replaces it with a fresh one that completes immediately on
/// the binary cache hit) — either way the resulting version row is
/// the same one already registered.
#[tokio::test]
async fn download_idempotent_on_second_call() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let v1 = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    // Second explicit download of the same coordinates — should map
    // back to the same version row v1.
    let payload = json!({
        "engine": "llamacpp",
        "version": mock.version,
        "platform": mock.platform,
        "arch": mock.arch,
        "backend": "cpu",
    });
    let resp = reqwest::Client::new()
        .post(mock.server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "re-download should kick off");
    let body: serde_json::Value = resp.json().await.unwrap();
    let key = body["key"].as_str().expect("key").to_string();
    let v2 = lrt::wait_for_download(&mock.server, &admin.token, &key).await;
    assert_eq!(v2, v1, "re-download must resolve to the same version row");
}

/// Full version CRUD: download → get → set-default → delete.
#[tokio::test]
async fn version_crud_lifecycle() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    // GET one
    let get = client
        .get(mock.server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);

    // set-default (download helper already did it; idempotent re-set)
    let set_default = client
        .post(mock.server.api_url(&format!("/local-runtime/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(set_default.status(), StatusCode::OK);

    // delete (with binary removal)
    let del = client
        .delete(mock.server.api_url(&format!(
            "/local-runtime/versions/{version_id}?remove_binary=true"
        )))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), StatusCode::NO_CONTENT);

    // gone
    let get2 = client
        .get(mock.server.api_url(&format!("/local-runtime/versions/{version_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get2.status(), StatusCode::NOT_FOUND);
}

/// `check-updates` diffs upstream releases against what's installed and
/// flags the build-pending case (tag exists, no binary asset for this host).
#[tokio::test]
async fn check_updates_reports_diff_and_pending_builds() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    let fetch = || async {
        let resp = client
            .get(mock.server.api_url("/local-runtime/versions/llamacpp/check-updates"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        resp.json::<serde_json::Value>().await.unwrap()
    };
    let find = |body: &serde_json::Value, want: &str| -> serde_json::Value {
        body["versions"]
            .as_array()
            .expect("versions array")
            .iter()
            .find(|e| e["version"].as_str() == Some(want))
            .unwrap_or_else(|| panic!("version {want} missing from {body}"))
            .clone()
    };

    // Before installing anything.
    let body = fetch().await;
    assert_eq!(body["engine"].as_str(), Some("llamacpp"));
    assert_eq!(body["platform"].as_str(), Some(mock.platform.as_str()));
    assert_eq!(body["arch"].as_str(), Some(mock.arch.as_str()));

    // TEST_VERSION ships the host cpu asset → ready, not yet installed.
    let test_v = find(&body, mock_release::TEST_VERSION);
    assert_eq!(test_v["binary_ready"].as_bool(), Some(true));
    assert_eq!(test_v["installed"].as_bool(), Some(false));
    assert!(
        test_v["available_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str() == Some("cpu")),
        "expected cpu in available_backends: {test_v}"
    );
    // Size is the actual archive size on disk (mock fixture stats
    // the real file when building releases.json) — so size_bytes
    // must be present and > 0 for the host-matching asset.
    let size = test_v["size_bytes"]
        .as_u64()
        .unwrap_or_else(|| panic!("size_bytes missing on host-ready release: {test_v}"));
    assert!(size > 0, "size_bytes must be > 0, got {size}");

    // PENDING_VERSION has no asset → surfaced but not installable,
    // and no size_bytes (the field is skip_serializing_if=Option::
    // is_none on the wire — so it's either absent or json null).
    let pending = find(&body, mock_release::PENDING_VERSION);
    assert_eq!(pending["binary_ready"].as_bool(), Some(false));
    assert_eq!(pending["installed"].as_bool(), Some(false));
    assert!(pending["available_backends"].as_array().unwrap().is_empty());
    assert!(
        pending.get("size_bytes").map(|v| v.is_null()).unwrap_or(true),
        "build-pending release must not carry size_bytes: {pending}"
    );

    // Install TEST_VERSION, then re-check → now flagged installed.
    lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let body2 = fetch().await;
    let test_v2 = find(&body2, mock_release::TEST_VERSION);
    assert_eq!(test_v2["installed"].as_bool(), Some(true), "should be installed after download: {test_v2}");
    assert!(
        test_v2["installed_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b.as_str() == Some("cpu")),
        "expected cpu in installed_backends: {test_v2}"
    );
}

/// Version endpoints reject callers lacking the dedicated
/// `versions_read` / `create` / `delete` permissions (02-permissions F-10
/// split: `llm_local_runtime::read` alone is NOT enough).
#[tokio::test]
async fn version_endpoints_require_permissions() {
    let server = TestServer::start().await;
    // Has instance-read but none of the version permissions.
    let user =
        create_user_with_only_permissions(&server, "reader", &["llm_local_runtime::read"]).await;
    let client = reqwest::Client::new();

    let list = client
        .get(server.api_url("/local-runtime/versions"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::FORBIDDEN, "list needs versions_read");

    let download = client
        .post(server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "engine": "llamacpp", "version": "v0.0.0-test",
            "platform": "linux", "arch": "x86_64", "backend": "cpu"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::FORBIDDEN, "download needs create");
}

// ─── Detached-task surface: list / snapshot / SSE ──────────────────────────
//
// The download endpoint is now fire-and-forget (`tokio::spawn` detached);
// these tests cover the three companion endpoints the UI uses to follow a
// running task: the snapshot/list polling surface and the live SSE stream.

/// After a successful download, the task stays in the in-process
/// registry with status=completed and a populated `result_version_id`
/// — that's what `GET /versions/downloads` returns to the UI on
/// mount, which is how page-reload survival re-paints the row.
#[tokio::test]
async fn list_active_downloads_includes_completed_task() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;

    let resp = reqwest::Client::new()
        .get(mock.server.api_url("/local-runtime/versions/downloads"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: serde_json::Value = resp.json().await.unwrap();
    let downloads = body["downloads"].as_array().expect("downloads array");
    let entry = downloads
        .iter()
        .find(|d| d["engine"].as_str() == Some("llamacpp") && d["version"].as_str() == Some(mock.version.as_str()))
        .unwrap_or_else(|| panic!("downloaded task should appear in list: {body}"));
    assert_eq!(entry["status"].as_str(), Some("completed"));
    assert_eq!(
        entry["result_version_id"].as_str(),
        Some(version_id.to_string().as_str()),
        "list entry should carry the new version's id"
    );
    // Composite key matches the format the SSE endpoint expects.
    assert_eq!(
        entry["key"].as_str(),
        Some(format!("llamacpp@{}@cpu", mock.version).as_str())
    );
}

/// The SSE endpoint must emit Connected then (at minimum) the
/// terminal Complete event. We POST the download via the *detached*
/// endpoint, immediately open the SSE stream by key, and parse
/// chunks until we see the terminal frame. The mock-engine binary
/// is small enough that Progress frames may or may not land before
/// Complete — the contract we assert is "Connected first, Complete
/// last, with a usable version_id."
#[tokio::test]
async fn sse_events_emit_connected_then_complete() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    // Kick off the detached task.
    let post = reqwest::Client::new()
        .post(mock.server.api_url("/local-runtime/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "engine": "llamacpp",
            "version": mock.version,
            "platform": mock.platform,
            "arch": mock.arch,
            "backend": "cpu",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(post.status(), StatusCode::OK);
    let started: serde_json::Value = post.json().await.unwrap();
    let key = started["key"].as_str().expect("key").to_string();
    assert_eq!(key, format!("llamacpp@{}@cpu", mock.version));
    // Sanity-check the events URL the API hands back to the UI.
    assert!(
        started["events_url"]
            .as_str()
            .unwrap()
            .contains(&key),
        "events_url should embed the task key: {started}"
    );

    // Open the SSE stream.
    let sse = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/versions/downloads/{key}/events")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(sse.status(), StatusCode::OK);
    assert!(
        sse.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("text/event-stream"),
        "endpoint must serve SSE content-type"
    );

    // Parse the chunked body into discrete SSE event frames.
    // Each frame is "event: <name>\ndata: <json>\n\n".
    let mut events: Vec<(String, serde_json::Value)> = Vec::new();
    let mut buf = String::new();
    let mut stream = sse.bytes_stream();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut saw_complete = false;
    while !saw_complete {
        tokio::select! {
            chunk = stream.next() => {
                let Some(chunk) = chunk else { break };
                let chunk = chunk.expect("stream chunk");
                buf.push_str(std::str::from_utf8(&chunk).expect("utf8"));
                while let Some(end) = buf.find("\n\n") {
                    let frame: String = buf.drain(..end + 2).collect();
                    let mut name = None;
                    let mut payload: Option<serde_json::Value> = None;
                    for line in frame.lines() {
                        if let Some(rest) = line.strip_prefix("event: ") {
                            name = Some(rest.trim().to_string());
                        } else if let Some(rest) = line.strip_prefix("data: ") {
                            payload = Some(
                                serde_json::from_str(rest.trim())
                                    .unwrap_or_else(|_| serde_json::Value::String(rest.trim().to_string())),
                            );
                        }
                    }
                    if let (Some(n), Some(p)) = (name, payload) {
                        if n == "complete" || n == "failed" { saw_complete = true; }
                        events.push((n, p));
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                if Instant::now() > deadline {
                    panic!("SSE stream did not reach terminal frame in time. Events so far: {events:?}");
                }
            }
        }
    }

    // First frame must be Connected with the task identity.
    assert!(!events.is_empty(), "stream produced no events");
    let (first_name, first_payload) = &events[0];
    assert_eq!(first_name, "connected", "first event must be Connected, got {events:?}");
    assert_eq!(first_payload["key"].as_str(), Some(key.as_str()));
    assert_eq!(first_payload["engine"].as_str(), Some("llamacpp"));

    // Final frame must be Complete with a real version_id.
    let (last_name, last_payload) = events.last().unwrap();
    assert_eq!(last_name, "complete", "stream did not end with Complete: {events:?}");
    let version_id = last_payload["version_id"]
        .as_str()
        .unwrap_or_else(|| panic!("Complete frame missing version_id: {events:?}"));

    // Cross-check: the snapshot endpoint should agree.
    let snap = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/versions/downloads/{key}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(snap.status(), StatusCode::OK);
    let snap_body: serde_json::Value = snap.json().await.unwrap();
    assert_eq!(snap_body["status"].as_str(), Some("completed"));
    assert_eq!(snap_body["result_version_id"].as_str(), Some(version_id));
}

/// A *late* SSE subscriber (after the task already reached terminal)
/// must still receive Connected + the terminal Complete event, then
/// the stream closes. This is the contract that lets a page reload
/// during a completed-but-not-dismissed download repaint the row
/// without losing the outcome — the registry retains terminal
/// entries for exactly this purpose.
#[tokio::test]
async fn late_subscriber_replays_terminal_outcome() {
    let mock = mock_release::setup().await;
    let admin = create_user_with_permissions(&mock.server, "admin", LOCAL_RUNTIME_ADMIN_PERMS).await;

    // Run the download synchronously to terminal via the helper.
    let version_id = lrt::download_engine_from_mock(&mock, &admin.token, "llamacpp").await;
    let key = format!("llamacpp@{}@cpu", mock.version);

    let sse = reqwest::Client::new()
        .get(mock.server.api_url(&format!("/local-runtime/versions/downloads/{key}/events")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .unwrap();
    assert_eq!(sse.status(), StatusCode::OK);

    // Read until the stream ends (terminal-only replay closes the
    // stream after emitting Complete).
    let mut events: Vec<(String, serde_json::Value)> = Vec::new();
    let mut buf = String::new();
    let mut stream = sse.bytes_stream();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(c) => {
                        let bytes = c.expect("stream chunk");
                        buf.push_str(std::str::from_utf8(&bytes).expect("utf8"));
                        while let Some(end) = buf.find("\n\n") {
                            let frame: String = buf.drain(..end + 2).collect();
                            let mut name = None;
                            let mut payload: Option<serde_json::Value> = None;
                            for line in frame.lines() {
                                if let Some(rest) = line.strip_prefix("event: ") {
                                    name = Some(rest.trim().to_string());
                                } else if let Some(rest) = line.strip_prefix("data: ") {
                                    payload = Some(
                                        serde_json::from_str(rest.trim())
                                            .unwrap_or_else(|_| serde_json::Value::String(rest.trim().to_string())),
                                    );
                                }
                            }
                            if let (Some(n), Some(p)) = (name, payload) {
                                events.push((n, p));
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                if Instant::now() > deadline {
                    panic!("late-subscriber stream did not close in time. Events: {events:?}");
                }
            }
        }
        // Stop once we've seen the terminal frame; the server should
        // also close the stream right after.
        if events.iter().any(|(n, _)| n == "complete" || n == "failed") {
            break;
        }
    }

    let names: Vec<&str> = events.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names.first().copied(), Some("connected"), "late subscriber must get Connected first; got {events:?}");
    assert!(names.contains(&"complete"), "late subscriber must replay terminal Complete; got {events:?}");
    // The replayed Complete must carry the same version_id as the snapshot.
    let complete = events.iter().find(|(n, _)| n == "complete").unwrap();
    assert_eq!(
        complete.1["version_id"].as_str(),
        Some(version_id.to_string().as_str()),
        "replayed Complete should carry the registered version id"
    );
}

//! TEST-16 — the `GET /api/voice/versions/check-updates` admin surface.
//!
//! Drives the REAL update-check path (`binary_manager::check_for_updates` →
//! `WhisperDownloader::list_releases`) against the loopback `MockReleaseServer`
//! (the same fixture `version_download_test` uses, wired via the
//! `WHISPER_RUNTIME_{RELEASE,API}_MIRROR` env seams). No network, no credentials.
//!
//! Asserts:
//!   - the mock's release is surfaced for the host platform/arch with
//!     `binary_ready=true` (a cpu asset is published) and `installed=false`
//!     (nothing downloaded yet);
//!   - a non-admin is denied (403, gated by `voice::admin::read`);
//!   - after actually downloading that version (full resolve→verify→extract→
//!     register pipeline), a re-check flips `installed=true` / `installed_backends`
//!     to `["cpu"]`.

use std::time::Duration;

use serde_json::{Value, json};

use super::mock_release;
use super::{drive_download_to_terminal, VOICE_ADMIN_PERMS};
use crate::common::test_helpers::create_user_with_permissions;

/// Fetch the check-updates response as `token`.
async fn check_updates(server: &crate::common::TestServer, token: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(server.api_url("/voice/versions/check-updates"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("check-updates request")
}

#[tokio::test]
async fn test_check_updates_lists_mock_release_then_flips_installed() {
    let mock = mock_release::setup().await;
    let admin =
        create_user_with_permissions(&mock.server, "voice_upd_admin", VOICE_ADMIN_PERMS).await;
    // Negative control: a default-Users member (voice::transcribe but no admin).
    let plain = create_user_with_permissions(&mock.server, "voice_upd_plain", &[]).await;

    // A non-admin is denied — the surface is gated by voice::admin::read.
    let res = check_updates(&mock.server, &plain.token).await;
    assert_eq!(res.status(), 403, "check-updates needs voice::admin::read");

    // Admin: the mock's release is surfaced, scoped to the host, binary-ready but
    // not yet installed.
    let res = check_updates(&mock.server, &admin.token).await;
    assert_eq!(res.status(), 200, "admin check-updates should 200");
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["platform"].as_str().unwrap(), mock.platform);
    assert_eq!(body["arch"].as_str().unwrap(), mock.arch);

    let row = body["versions"]
        .as_array()
        .expect("versions array")
        .iter()
        .find(|v| v["version"] == mock.version.as_str())
        .expect("the mock release should be listed");
    assert_eq!(
        row["binary_ready"], true,
        "a cpu asset is published for the host → binary_ready"
    );
    assert_eq!(
        row["installed"], false,
        "nothing downloaded yet → installed=false"
    );
    assert!(
        row["installed_backends"].as_array().unwrap().is_empty(),
        "no installed backends before download"
    );
    assert!(
        row["available_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b == "cpu"),
        "cpu is the upstream-published backend for the host"
    );

    // Now actually download it through the full pipeline, driving the SSE to
    // `complete` (mirrors version_download_test).
    let client = reqwest::Client::new();
    let res = client
        .post(mock.server.api_url("/voice/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": mock.version }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "download start should 200");
    let started: Value = res.json().await.unwrap();
    let key = started["key"].as_str().expect("download key").to_string();
    drive_download_to_terminal(&mock.server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("download should reach `complete`");

    // A re-check now reports the version as installed for the cpu backend.
    let res = check_updates(&mock.server, &admin.token).await;
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let row = body["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["version"] == mock.version.as_str())
        .expect("the mock release should still be listed");
    assert_eq!(
        row["installed"], true,
        "after download the version is installed for the host"
    );
    assert!(
        row["installed_backends"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b == "cpu"),
        "the cpu backend is now listed as installed"
    );
}

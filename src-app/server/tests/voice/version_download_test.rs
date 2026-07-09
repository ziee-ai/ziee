//! TEST-15 + TEST-17 — the whisper runtime-version admin surface.
//!
//! TEST-15 drives the FULL binary-download pipeline against a loopback
//! `MockReleaseServer` (resolve → download → sha256-verify → extract → cache →
//! register), consuming the `downloads/{key}/events` SSE to `complete` and
//! asserting a `voice_runtime_versions` row appears + the binary is on disk.
//! Then `set-default` (asserting the `VoiceRuntimeVersion` sync). TEST-17
//! asserts the delete in-use guard (409 on the default) + a clean 204 + on-disk
//! removal for a non-in-use version.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::mock_release;
use super::{drive_download_to_terminal, insert_version_row, VOICE_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

/// TEST-15 — download via the mock release, SSE to complete, row registered,
/// then set-default emits the version sync.
#[tokio::test]
async fn test_version_download_via_mock_and_set_default() {
    let mock = mock_release::setup().await;
    let admin =
        create_user_with_permissions(&mock.server, "voice_dl_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    // Subscribe BEFORE the download so the completion Create emit is captured.
    let mut probe = SyncProbe::open(&mock.server, &admin.token).await;

    // Start the detached download (host platform/arch, cpu backend by default).
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

    // Drive the SSE progress → complete.
    drive_download_to_terminal(&mock.server, &admin.token, &key, Duration::from_secs(30))
        .await
        .expect("download should reach `complete`");

    // The completion emits VoiceRuntimeVersion/create to admins.
    probe
        .expect_event("voice_runtime_version", "create", Duration::from_secs(5))
        .await;

    // A row now exists for the downloaded version, with the binary on disk.
    let res = client
        .get(mock.server.api_url("/voice/versions"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let list: Value = res.json().await.unwrap();
    let row = list["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["version"] == mock.version.as_str())
        .expect("downloaded version registered");
    let version_id = Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
    let binary_path = row["binary_path"].as_str().unwrap();
    assert!(
        std::path::Path::new(binary_path).exists(),
        "extracted whisper-server binary should be on disk at {binary_path}"
    );
    assert_eq!(row["backend"], "cpu");
    assert_eq!(row["is_system_default"], false, "not default until set");

    // set-default → 200 + VoiceRuntimeVersion/update sync.
    let res = client
        .post(mock.server.api_url(&format!("/voice/versions/{version_id}/set-default")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "set-default should 200");
    let updated: Value = res.json().await.unwrap();
    assert_eq!(updated["is_system_default"], true);

    let frame = probe
        .expect_event("voice_runtime_version", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, version_id.to_string(), "set-default frame carries the row id");
}

/// TEST-17 — delete guard: 409 for the in-use (system default) version, 204 for
/// a non-in-use one, and `?remove_binary=true` clears the on-disk dir.
#[tokio::test]
async fn test_delete_version_guard_and_remove_binary() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_del_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    // Version A: the system default → delete refused with 409.
    let default_id = insert_version_row(&server, "v-default", "cpu", "/nonexistent/whisper-server", true).await;
    let res = client
        .delete(server.api_url(&format!("/voice/versions/{default_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 409, "the system-default version cannot be deleted");

    // Version B: not default, with a real binary dir so remove_binary can clear it.
    let bin_dir = server
        .data_dir()
        .join("whisper-runtime")
        .join("binaries")
        .join("v-spare")
        .join("linux-x86_64-cpu");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let bin_path = bin_dir.join("whisper-server");
    std::fs::write(&bin_path, b"stub binary").unwrap();
    let spare_id = insert_version_row(
        &server,
        "v-spare",
        "cpu",
        bin_path.to_string_lossy().as_ref(),
        false,
    )
    .await;

    let res = client
        .delete(server.api_url(&format!("/voice/versions/{spare_id}?remove_binary=true")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "a non-in-use version deletes cleanly");
    assert!(!bin_dir.exists(), "remove_binary=true should delete the binary dir");

    // The row is gone; a re-delete is idempotent (204).
    let res = client
        .delete(server.api_url(&format!("/voice/versions/{spare_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204, "deleting an already-gone version is idempotent");
}

//! REAL end-to-end test against the ACTUAL `ziee-ai/whisper.cpp` GitHub release.
//!
//! Unlike `version_download_test` (which drives the pipeline against a loopback
//! `MockReleaseServer` via the debug mirror seam), this test starts a plain
//! `TestServer` with NO mirror override, so the production download path resolves
//! against the REAL GitHub API + release CDN — exactly what runs on a user's
//! machine. It exercises the full HTTP path:
//!
//!   POST /voice/versions/download {version:"latest"}  →  resolve latest tag  →
//!   download `whisper-server-<plat>-<arch>-cpu` archive  →  MANDATORY `.sha256`
//!   sidecar verify  →  extract  →  register `voice_runtime_versions` row  →
//!   SSE `complete`  →  the extracted `whisper-server` binary actually runs.
//!
//! This proves the fork's release CI publishes artifacts whose NAMES + `.sha256`
//! sidecars match what the runtime enforces, and that the packaged binary is
//! self-contained (libs resolve via RPATH `$ORIGIN` / `@loader_path`).
//!
//! `#[ignore]` by default: needs network + a PUBLISHED release, so it can't run
//! in the offline CI matrix — same gating rationale as the llm runtime's
//! `gold_smoke`. Run it explicitly:
//!
//!   source tests/.env.test
//!   cargo test --test integration_tests \
//!     -- --ignored voice::real_repo_test::real_whisper_release --test-threads=1

use std::time::Duration;

use serde_json::{json, Value};

use super::{drive_download_to_terminal, VOICE_ADMIN_PERMS};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

#[tokio::test]
#[ignore = "hits the real ziee-ai/whisper.cpp GitHub release (network + published release required)"]
async fn real_whisper_release_downloads_verifies_and_runs() {
    // Plain TestServer — NO mock_release, NO mirror env → the downloader points
    // at the real https://api.github.com + https://github.com hosts.
    let server = TestServer::start().await;
    let admin =
        create_user_with_permissions(&server, "voice_real_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    // Kick off the real download for this host (platform/arch auto-detected,
    // backend defaults to cpu — the one the fork publishes for every host).
    // `version:"latest"` exercises the real `releases/latest` resolve.
    let res = client
        .post(server.api_url("/voice/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "latest" }))
        .send()
        .await
        .expect("start real download");
    assert_eq!(
        res.status(),
        200,
        "download start should 200 (is a release published on ziee-ai/whisper.cpp?)"
    );
    let started: Value = res.json().await.unwrap();
    let key = started["key"].as_str().expect("download key").to_string();

    // Drive the SSE to `complete`. Generous timeout: a real ~6 MB CPU archive
    // download + sha256 verify + extract over the network.
    drive_download_to_terminal(&server, &admin.token, &key, Duration::from_secs(180))
        .await
        .expect("real download should reach `complete` (name/sha256 contract must match)");

    // A `voice_runtime_versions` row now exists, binary on disk.
    let res = client
        .get(server.api_url("/voice/versions"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let list: Value = res.json().await.unwrap();
    let row = list["versions"]
        .as_array()
        .and_then(|v| v.first())
        .expect("a downloaded version should be registered");
    let version = row["version"].as_str().unwrap().to_string();
    let binary_path = row["binary_path"].as_str().unwrap().to_string();
    assert_eq!(row["backend"], "cpu");
    assert!(
        std::path::Path::new(&binary_path).exists(),
        "extracted whisper-server binary should be on disk at {binary_path}"
    );
    eprintln!("real repo: registered whisper-server {version} at {binary_path}");

    // The extracted binary is self-contained + runs (libs resolve via RPATH).
    let out = std::process::Command::new(&binary_path)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("spawn {binary_path} --help: {e}"));
    assert!(
        out.status.success(),
        "whisper-server --help should exit 0 (libs resolve via $ORIGIN); status={:?}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    // whisper-server prints usage + the `load_backend: loaded CPU backend`
    // line to STDERR, so check both streams.
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        help.contains("whisper-server") || help.contains("threads") || help.contains("--help"),
        "help text should look like whisper-server usage; got:\n{help}"
    );

    eprintln!(
        "real repo: whisper-server {version} downloaded from ziee-ai/whisper.cpp, \
         sha256-verified, extracted, and ran successfully ✅"
    );
}

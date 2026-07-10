//! TEST-20 — the managed whisper-server instance lifecycle.
//!
//! Uses the REAL production auto-start path (a freshly-built `stub-whisper-server`
//! registered as the system-default runtime + a pre-staged ggml model), then
//! drives the admin instance endpoints for real:
//!
//!   transcribe (auto-start) → GET /instance (running/healthy, has base_url+port)
//!     → POST /instance/restart (running again) → POST /instance/stop
//!     → GET /instance (stopped).
//!
//! A second test asserts the idle-reaper unloads an idle instance, using the
//! debug-only `WHISPER_RUNTIME_REAPER_TICK_MS` seam + a short `idle_unload_secs`
//! and a *bounded poll* to the terminal state (no fixed sleep-then-assert): the
//! same terminal-condition-wait pattern the download-SSE helper uses.

use std::time::Duration;

use serde_json::{Value, json};

use super::{insert_version_row, make_wav, stage_model, stub_whisper_binary, VOICE_ADMIN_PERMS};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};

/// Multipart-post a WAV to the transcribe endpoint (drives the auto-start path).
async fn post_transcribe(server: &TestServer, token: &str, wav: Vec<u8>) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("file", part);
    reqwest::Client::new()
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("transcribe request")
}

/// GET the managed instance snapshot as `token`.
async fn get_instance(server: &TestServer, token: &str) -> Value {
    let res = reqwest::Client::new()
        .get(server.api_url("/voice/instance"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get instance");
    assert_eq!(res.status(), 200, "GET /voice/instance should 200");
    res.json().await.unwrap()
}

/// Register the stub as the system-default whisper runtime + pre-stage the model
/// so the air-gap auto-start path skips any binary/model download.
async fn stage_default_runtime(server: &TestServer) {
    let stub = stub_whisper_binary();
    insert_version_row(server, "v0.0.0-stub", "cpu", stub.to_string_lossy().as_ref(), true).await;
    stage_model(server, "base");
}

/// TEST-20 (deterministic) — start (via transcribe) → running → restart → stop.
#[tokio::test]
async fn test_instance_start_restart_stop() {
    let server = TestServer::start().await;
    stage_default_runtime(&server).await;

    // An admin holds voice::admin::* (instance surface) AND voice::transcribe (via
    // the default Users group), so one token drives the whole flow.
    let admin = create_user_with_permissions(&server, "voice_life_admin", VOICE_ADMIN_PERMS).await;

    // Before any use, the singleton row is stopped.
    let info = get_instance(&server, &admin.token).await;
    assert_eq!(info["status"], "stopped", "instance starts out stopped");

    // (a) Trigger the REAL auto-start path via a transcribe.
    let resp = post_transcribe(&server, &admin.token, make_wav(1.0)).await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "transcribe should 200 (body: {body})");

    // (b) The instance is now running + healthy with a loopback base_url + port.
    let info = get_instance(&server, &admin.token).await;
    assert_eq!(info["status"], "running", "instance running after auto-start");
    assert_eq!(info["state"], "healthy", "health state is healthy");
    assert!(info["local_port"].is_number(), "a loopback port is bound");
    let base_url = info["base_url"].as_str().expect("base_url present");
    assert!(
        base_url.starts_with("http://127.0.0.1:"),
        "base_url is a loopback URL, got {base_url}"
    );
    assert_eq!(info["active_model"], "ggml-base.bin", "configured model active");

    // (c) Restart → still running (drain + respawn with the configured model).
    let res = reqwest::Client::new()
        .post(server.api_url("/voice/instance/restart"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "restart should 200");
    let restarted: Value = res.json().await.unwrap();
    assert_eq!(restarted["status"], "running", "running again after restart");
    assert_eq!(restarted["state"], "healthy", "healthy again after restart");

    // (d) Stop → the snapshot reports stopped.
    let res = reqwest::Client::new()
        .post(server.api_url("/voice/instance/stop"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "stop should 200");
    let stopped: Value = res.json().await.unwrap();
    assert_eq!(stopped["status"], "stopped", "stop transitions to stopped");

    let info = get_instance(&server, &admin.token).await;
    assert_eq!(info["status"], "stopped", "GET /instance confirms stopped");

    // Non-admin (voice::transcribe only) cannot read/mutate the instance surface.
    let plain = create_user_with_permissions(&server, "voice_life_plain", &[]).await;
    let res = reqwest::Client::new()
        .get(server.api_url("/voice/instance"))
        .header("Authorization", format!("Bearer {}", plain.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "GET /instance needs voice::admin::read");
    let res = reqwest::Client::new()
        .post(server.api_url("/voice/instance/stop"))
        .header("Authorization", format!("Bearer {}", plain.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "stop needs voice::admin::manage");
}

/// TEST-20 (idle-reap) — the reaper unloads an idle running instance. Uses the
/// debug reaper-tick seam + a 1s idle threshold, then polls the instance to its
/// terminal `stopped` state with a generous deadline (bounded wait on a
/// condition, NOT a fixed sleep) so it is deterministic, not flaky.
#[tokio::test]
async fn test_idle_reaper_unloads_instance() {
    // Fast reaper tick so idle-eviction is observable in seconds (debug-only seam,
    // compiled out of release builds).
    let opts = TestServerOptions {
        extra_env: vec![("WHISPER_RUNTIME_REAPER_TICK_MS".to_string(), "250".to_string())],
        ..Default::default()
    };
    let server = TestServer::start_with_options(opts).await;
    stage_default_runtime(&server).await;
    let admin = create_user_with_permissions(&server, "voice_reap_admin", VOICE_ADMIN_PERMS).await;

    // Shrink the idle-unload threshold + drain window so the reaper evicts quickly.
    let res = reqwest::Client::new()
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "idle_unload_secs": 1, "drain_timeout_secs": 1 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "settings PUT should 200");

    // Auto-start via a transcribe → running.
    let resp = post_transcribe(&server, &admin.token, make_wav(1.0)).await;
    assert_eq!(resp.status(), 200, "transcribe should 200");
    let info = get_instance(&server, &admin.token).await;
    assert_eq!(info["status"], "running", "running right after auto-start");

    // Bounded poll: the reaper (tick 250ms, idle 1s) drains + SIGTERMs the idle
    // instance and marks the row stopped. Assert we reach that terminal state
    // within a generous deadline — no assumption about exact timing.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let info = get_instance(&server, &admin.token).await;
        if info["status"] == "stopped" {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "idle instance was not reaped within the deadline (last status: {})",
                info["status"]
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

//! TEST-14 — the deploy-level config kill switch (`voice: { enabled: false }`).
//!
//! When voice is disabled in config, `VoiceModule::register_routes` merges
//! NOTHING, so the entire voice REST surface is unmounted (transcribe +
//! capability + admin). The routes return 404, no whisper-server can ever be
//! spawned, and the server still boots normally.

use super::make_wav;
use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;

#[tokio::test]
async fn test_voice_disabled_unmounts_routes_and_server_still_boots() {
    // The harness spawns + health-polls the server; returning proves it booted.
    let server = TestServer::start_with_options(TestServerOptions {
        voice_enabled: Some(false),
        ..Default::default()
    })
    .await;

    // Sanity: the server is up and non-voice routes work.
    let health = reqwest::Client::new()
        .get(format!("{}/api/health", server.base_url))
        .send()
        .await
        .unwrap();
    assert!(health.status().is_success(), "server should be healthy");

    // A default Users member holds voice::transcribe, but the route is gone.
    let user = create_user_with_permissions(&server, "voice_disabled_user", &[]).await;
    let admin = create_user_with_permissions(
        &server,
        "voice_disabled_admin",
        super::VOICE_ADMIN_PERMS,
    )
    .await;
    let client = reqwest::Client::new();

    // transcribe route unmounted → 404 (an unmatched route, not the 401/403 an
    // auth-gated-but-mounted route would return).
    let part = reqwest::multipart::Part::bytes(make_wav(0.5))
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    let resp = client
        .post(server.api_url("/voice/transcribe"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "transcribe route must be unmounted (404)");

    // settings route unmounted → 404 even for an admin (route absent, not 403).
    let resp = client
        .get(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "settings route must be unmounted (404)");

    // capability route also unmounted.
    let resp = client
        .get(server.api_url("/voice/capability"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "capability route must be unmounted (404)");
}

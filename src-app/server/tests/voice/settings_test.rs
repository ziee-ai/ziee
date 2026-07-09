//! TEST-18 + TEST-21 — voice settings CRUD, gating, range validation, and the
//! `VoiceSettings` sync emit.

use std::time::Duration;

use serde_json::{Value, json};
use uuid::Uuid;

use super::VOICE_ADMIN_PERMS;
use crate::common::TestServer;
use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::create_user_with_permissions;

/// TEST-18 — admin GET/PUT round-trip; values persist.
#[tokio::test]
async fn test_admin_get_and_update_settings_roundtrip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_settings_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    // GET returns the seeded singleton with the migration defaults.
    let res = client
        .get(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], true);
    assert_eq!(row["model"], "base");
    assert_eq!(row["language"], "auto");
    assert_eq!(row["max_upload_bytes"], 33_554_432i64);

    // PUT a mix of fields (incl. the 64 MiB max_upload_bytes ceiling exactly).
    let res = client
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "model": "small",
            "language": "es",
            "idle_unload_secs": 900,
            "auto_start_timeout_secs": 45,
            "drain_timeout_secs": 20,
            "max_clip_seconds": 300,
            "max_upload_bytes": 67_108_864i64,
            "enabled": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["model"], "small");
    assert_eq!(row["language"], "es");
    assert_eq!(row["max_upload_bytes"], 67_108_864i64);
    assert_eq!(row["enabled"], false);

    // Persisted: a fresh GET reflects the update.
    let res = client
        .get(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["model"], "small");
    assert_eq!(row["idle_unload_secs"], 900);
    assert_eq!(row["max_clip_seconds"], 300);
}

/// TEST-18 — non-admin is denied both read and write.
#[tokio::test]
async fn test_non_admin_cannot_read_or_update_settings() {
    let server = TestServer::start().await;
    // Default Users member: holds voice::transcribe but no admin perms.
    let user = create_user_with_permissions(&server, "voice_settings_plain", &[]).await;
    let client = reqwest::Client::new();

    let res = client
        .get(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "read needs voice::admin::read");

    let res = client
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "write needs voice::admin::manage");
}

/// TEST-18 — range + allow-list validation returns 400 (incl. the 64 MiB
/// max_upload_bytes ceiling and the language allow-list).
#[tokio::test]
async fn test_settings_range_validation() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_range_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    let cases: &[(&str, Value)] = &[
        // model allow-list
        ("model", json!("huge")),
        // language allow-list: not 'auto' and not a 2-letter code
        ("language", json!("xyz")),
        ("language", json!("1")),
        // idle_unload_secs 0..=86400
        ("idle_unload_secs", json!(90_000)),
        ("idle_unload_secs", json!(-1)),
        // auto_start_timeout_secs 1..=600
        ("auto_start_timeout_secs", json!(0)),
        ("auto_start_timeout_secs", json!(601)),
        // drain_timeout_secs 1..=600
        ("drain_timeout_secs", json!(0)),
        ("drain_timeout_secs", json!(601)),
        // max_clip_seconds 1..=3600
        ("max_clip_seconds", json!(0)),
        ("max_clip_seconds", json!(3601)),
        // max_upload_bytes 1024..=67108864 (64 MiB)
        ("max_upload_bytes", json!(1023)),
        ("max_upload_bytes", json!(67_108_865i64)),
    ];

    for (field, value) in cases {
        let mut map = serde_json::Map::new();
        map.insert((*field).to_string(), value.clone());
        let res = client
            .put(server.api_url("/voice/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&Value::Object(map))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 400, "{field}={value} must be rejected with 400");
    }

    // Positive controls: 'auto' and a valid 2-letter code are accepted.
    for lang in ["auto", "en", "ZH"] {
        let res = client
            .put(server.api_url("/voice/settings"))
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&json!({ "language": lang }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200, "language {lang} should be accepted");
    }
}

/// TEST-21 — a settings PUT emits `VoiceSettings`/`update` to the
/// `voice::admin::read` audience, self-echo-suppressed for the mutating
/// connection and invisible to a non-admin.
#[tokio::test]
async fn test_settings_update_emits_sync_to_admins_only() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_sync_admin", VOICE_ADMIN_PERMS).await;
    // Negative control: a member with voice::transcribe but no admin::read.
    let plain = create_user_with_permissions(&server, "voice_sync_plain", &[]).await;

    // Two admin connections: one is the mutation's origin, the other observes.
    let mut origin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut observer_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        // Echo the origin connection id so the fan-out skips it (self-echo).
        .header("X-Sync-Connection-Id", origin_probe.connection_id().to_string())
        .json(&json!({ "max_clip_seconds": 90 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // The other admin connection receives the singleton (nil-id) frame.
    let frame = observer_probe
        .expect_event("voice_settings", "update", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string(), "singleton settings → nil id");

    // The originating connection is self-echo-suppressed; the non-admin is
    // outside the audience. Both observe nothing.
    origin_probe.expect_silence(Duration::from_secs(1)).await;
    plain_probe.expect_silence(Duration::from_secs(1)).await;
}

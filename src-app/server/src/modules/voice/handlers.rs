//! HTTP handlers for the voice dictation admin + capability REST surface.
//!
//! The transcribe handler and the runtime-version / model / instance admin
//! handlers are added in the engine + lifecycle layers; this file holds the
//! settings singleton (admin) and the capability snapshot (any transcribe user).

use aide::transform::TransformOperation;
use axum::{Json, http::StatusCode};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::models::{UpdateVoiceSettingsRequest, VoiceCapability, VoiceSettings};
use super::permissions::{VoiceAdminManage, VoiceAdminRead, VoiceTranscribe};

// ───────────────────────────── settings (admin) ─────────────────────────────

pub async fn get_settings(
    _auth: RequirePermissions<(VoiceAdminRead,)>,
) -> ApiResult<Json<VoiceSettings>> {
    let row = Repos.voice.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminRead,)>(op)
        .id("Voice.getSettings")
        .tag("Voice")
        .summary("Read voice dictation settings")
        .response::<200, Json<VoiceSettings>>()
}

/// Range + allow-list validation for a settings PATCH. Defense-in-depth
/// alongside the DB CHECKs → clearer 400s than an opaque constraint violation.
/// Pure (no I/O) so it is directly unit-testable; `update_settings` calls it
/// before touching the repository, so the handler behavior is unchanged.
/// A storable whisper-model name: 1..=50 chars of `[A-Za-z0-9._-]` (fits
/// `voice_runtime_settings.model VARCHAR(50)` + the `ggml-<name>.bin` convention,
/// with no path-traversal characters).
pub fn is_valid_model_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 50
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && name != "."
        && name != ".."
        && !name.contains("..")
}

/// A valid model source: an `owner/repo` HF slug OR an `https://` base URL.
pub fn is_valid_model_source_repo(repo: &str) -> bool {
    let r = repo.trim();
    if r.is_empty() || r.len() > 200 {
        return false;
    }
    if r.starts_with("https://") {
        return url::Url::parse(r).is_ok();
    }
    // owner/repo — exactly one slash, both sides non-empty, no traversal/space.
    let parts: Vec<&str> = r.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        })
}

pub fn validate_settings_patch(body: &UpdateVoiceSettingsRequest) -> Result<(), AppError> {
    // Model can now be ANY installed model name (catalog ∪ uploaded), so the
    // validator only enforces the storable FORMAT (fits VARCHAR(50), safe slug);
    // the handler additionally verifies the name resolves to a catalog-supported
    // OR an installed model (async) before persisting it.
    if let Some(ref m) = body.model
        && !is_valid_model_name(m)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "model name must be 1..=50 chars of [A-Za-z0-9._-]",
        ));
    }
    if let Some(ref repo) = body.model_source_repo
        && !is_valid_model_source_repo(repo)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "model_source_repo must be an 'owner/repo' slug or an https:// URL",
        ));
    }
    if let Some(ref lang) = body.language {
        // Accept `auto` (whisper auto-detect) or a 2-letter ISO 639-1 code, so a
        // bad value fails with a clean 400 here instead of an opaque 503 on the
        // next transcribe. Empty is tolerated (treated as `auto` downstream).
        let l = lang.trim();
        let ok = l.is_empty()
            || l.eq_ignore_ascii_case("auto")
            || (l.len() == 2 && l.bytes().all(|b| b.is_ascii_alphabetic()));
        if !ok {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "language must be 'auto' or a 2-letter ISO 639-1 code (e.g. en, es, zh)",
            ));
        }
    }
    if let Some(n) = body.idle_unload_secs
        && !(0..=86_400).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "idle_unload_secs out of range (0..=86400)",
        ));
    }
    if let Some(n) = body.auto_start_timeout_secs
        && !(1..=600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "auto_start_timeout_secs out of range (1..=600)",
        ));
    }
    if let Some(n) = body.drain_timeout_secs
        && !(1..=600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "drain_timeout_secs out of range (1..=600)",
        ));
    }
    if let Some(n) = body.max_clip_seconds
        && !(1..=3_600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "max_clip_seconds out of range (1..=3600)",
        ));
    }
    if let Some(n) = body.max_upload_bytes
        && !(1_024..=67_108_864).contains(&n)
    {
        // Ceiling matches VOICE_TRANSCRIBE_BODY_LIMIT (64 MiB) so a larger
        // setting can't yield a 413 before the handler's logical cap runs.
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "max_upload_bytes out of range (1024..=67108864)",
        ));
    }
    if let Some(n) = body.stream_interval_ms
        && !(300..=10_000).contains(&n)
    {
        // Matches the DB CHECK on stream_interval_ms. This is the client's interim
        // decode cadence hint (the client single-flights interim decodes, so it
        // paces — not gates — load); a floor keeps a misconfigured cadence sane and
        // >10s stops feeling live.
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "stream_interval_ms out of range (300..=10000)",
        ));
    }
    if let Some(n) = body.stream_max_decode_secs
        && !(5..=600).contains(&n)
    {
        // Matches the DB CHECK. Too small drops useful context; too large defeats
        // the per-tick cost bound.
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "stream_max_decode_secs out of range (5..=600)",
        ));
    }
    Ok(())
}

pub async fn update_settings(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateVoiceSettingsRequest>,
) -> ApiResult<Json<VoiceSettings>> {
    validate_settings_patch(&body)?;

    // The model, if changed via settings, must resolve to a catalog-supported
    // name OR an already-installed model (the primary activation path is
    // POST /voice/models/{id}/activate; this covers a direct settings edit).
    if let Some(ref m) = body.model
        && !super::model::is_supported_model(m)
        && Repos.voice_model.get_by_name(m).await?.is_none()
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("model {m:?} is neither a catalog model nor installed"),
        )
        .to_api_error());
    }

    // A source-repo change must invalidate the catalog cache so the next fetch
    // hits the new source.
    let source_changed = body.model_source_repo.is_some();

    let row = Repos
        .voice
        .update_settings(
            body.enabled,
            body.model,
            body.language,
            body.idle_unload_secs,
            body.auto_start_timeout_secs,
            body.drain_timeout_secs,
            body.max_clip_seconds,
            body.max_upload_bytes,
            body.streaming_enabled,
            body.stream_interval_ms,
            body.stream_max_decode_secs,
            body.model_source_repo,
        )
        .await?;

    if source_changed {
        super::model_catalog::invalidate_cache();
    }

    sync_publish(
        SyncEntity::VoiceSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<VoiceAdminRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.updateSettings")
        .tag("Voice")
        .summary("Update voice dictation settings (enable, model, language, caps)")
        .response::<200, Json<VoiceSettings>>()
}

// ─────────────────────────── capability (any user) ──────────────────────────

pub async fn get_capability(
    _auth: RequirePermissions<(VoiceTranscribe,)>,
) -> ApiResult<Json<VoiceCapability>> {
    let settings = Repos.voice.get_settings().await?;
    let runtime_ready = super::binary_manager::runtime_ready().await;
    let model_ready = super::model::model_present(&settings.model);
    let enabled = settings.enabled;
    let can_transcribe = enabled && runtime_ready && model_ready;
    let cap = VoiceCapability {
        enabled,
        runtime_ready,
        model_ready,
        model: settings.model,
        max_clip_seconds: settings.max_clip_seconds,
        can_transcribe,
        // Live captions require the whole mic to be usable (runtime + model ready)
        // AND the deployment streaming toggle — the interim loop must never start
        // against an unprovisioned runtime.
        streaming_enabled: can_transcribe && settings.streaming_enabled,
        stream_interval_ms: settings.stream_interval_ms,
    };
    Ok((StatusCode::OK, Json(cap)))
}

pub fn get_capability_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceTranscribe,)>(op)
        .id("Voice.capability")
        .tag("Voice")
        .summary("Voice dictation readiness for the composer mic button")
        .response::<200, Json<VoiceCapability>>()
}

// ─────────────────────── sync-cache (admin convenience) ──────────────────────

use super::runtime_version::models::SyncCacheResponse;

pub async fn sync_cache(
    _auth: RequirePermissions<(VoiceAdminManage,)>,
) -> ApiResult<Json<SyncCacheResponse>> {
    let synced_count = super::binary_manager::sync_cache().await?;
    Ok((
        StatusCode::OK,
        Json(SyncCacheResponse {
            synced_count,
            message: format!("Synced {synced_count} cached whisper binary(ies)"),
        }),
    ))
}

pub fn sync_cache_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(VoiceAdminManage,)>(op)
        .id("Voice.syncVersionCache")
        .tag("Voice")
        .summary("Back-fill the runtime-version registry from cached whisper binaries on disk")
        .response::<200, Json<SyncCacheResponse>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-5: storable model-name format + source-repo format validators.
    #[test]
    fn model_name_format_accepts_valid_rejects_bad() {
        assert!(is_valid_model_name("base"));
        assert!(is_valid_model_name("large-v3-turbo-q5_0"));
        assert!(is_valid_model_name("my.custom_model-1"));
        assert!(!is_valid_model_name("")); // empty
        assert!(!is_valid_model_name("../etc/passwd")); // traversal chars
        assert!(!is_valid_model_name("a/b")); // slash
        assert!(!is_valid_model_name("..")); // dotdot
        assert!(!is_valid_model_name(&"x".repeat(51))); // > 50
    }

    #[test]
    fn source_repo_format_accepts_slug_and_https_rejects_bad() {
        assert!(is_valid_model_source_repo("ggerganov/whisper.cpp"));
        assert!(is_valid_model_source_repo("https://hf.internal/mirror"));
        assert!(!is_valid_model_source_repo("")); // empty
        assert!(!is_valid_model_source_repo("no-slash")); // not owner/repo
        assert!(!is_valid_model_source_repo("a/b/c")); // too many segments
        assert!(!is_valid_model_source_repo("../evil")); // traversal
        assert!(!is_valid_model_source_repo("http://insecure/x")); // non-https url
    }

    fn base() -> UpdateVoiceSettingsRequest {
        UpdateVoiceSettingsRequest::default()
    }

    fn assert_validation_400(body: &UpdateVoiceSettingsRequest, why: &str) {
        let err = validate_settings_patch(body).expect_err(why);
        assert_eq!(err.status_code(), 400, "{why}");
        assert_eq!(err.error_code(), "VALIDATION_ERROR", "{why}");
    }

    #[test]
    fn accepts_valid_values() {
        // A fully-populated in-range patch validates.
        let ok = UpdateVoiceSettingsRequest {
            enabled: Some(true),
            model: Some("base.en".to_string()),
            language: Some("en".to_string()),
            idle_unload_secs: Some(1800),
            auto_start_timeout_secs: Some(30),
            drain_timeout_secs: Some(30),
            max_clip_seconds: Some(120),
            max_upload_bytes: Some(1_048_576),
            streaming_enabled: Some(true),
            stream_interval_ms: Some(1500),
            stream_max_decode_secs: Some(30),
            model_source_repo: Some("ggerganov/whisper.cpp".to_string()),
        };
        assert!(validate_settings_patch(&ok).is_ok());
        // Empty patch (all None → leave everything) is valid.
        assert!(validate_settings_patch(&base()).is_ok());
    }

    #[test]
    fn language_auto_and_iso_639_1_ok_bad_rejected() {
        for lang in ["auto", "AUTO", "en", "ES", "zh", "", "  "] {
            let b = UpdateVoiceSettingsRequest {
                language: Some(lang.to_string()),
                ..base()
            };
            assert!(
                validate_settings_patch(&b).is_ok(),
                "language {lang:?} should be accepted"
            );
        }
        for lang in ["english", "e", "e1", "123"] {
            let b = UpdateVoiceSettingsRequest {
                language: Some(lang.to_string()),
                ..base()
            };
            assert_validation_400(&b, "bad language must be rejected");
        }
    }

    #[test]
    fn model_name_format_validated_existence_deferred_to_handler() {
        // The pure validator now checks storable FORMAT only (any library model
        // name is allowed — a catalog `large-v3` is format-valid); the
        // catalog-or-installed EXISTENCE check moved to the async `update_settings`
        // handler (covered by the integration tests). So a well-formed name passes
        // here regardless of whether it's one of the 4 built-in defaults.
        for m in ["tiny", "base", "base.en", "small", "large-v3", "large-v3-turbo-q5_0"] {
            let b = UpdateVoiceSettingsRequest {
                model: Some(m.to_string()),
                ..base()
            };
            assert!(
                validate_settings_patch(&b).is_ok(),
                "well-formed model {m} passes format validation"
            );
        }
        // A MALFORMED name (path-traversal / slash / too long) is still rejected.
        for bad in ["../etc", "a/b", ".."] {
            let b = UpdateVoiceSettingsRequest {
                model: Some(bad.to_string()),
                ..base()
            };
            assert_validation_400(&b, "malformed model name must be rejected");
        }
    }

    #[test]
    fn out_of_range_numeric_caps_rejected() {
        // idle_unload_secs: 0..=86400
        assert_validation_400(
            &UpdateVoiceSettingsRequest { idle_unload_secs: Some(-1), ..base() },
            "idle below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { idle_unload_secs: Some(86_401), ..base() },
            "idle above range",
        );
        // auto_start_timeout_secs: 1..=600
        assert_validation_400(
            &UpdateVoiceSettingsRequest { auto_start_timeout_secs: Some(0), ..base() },
            "auto-start below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { auto_start_timeout_secs: Some(601), ..base() },
            "auto-start above range",
        );
        // drain_timeout_secs: 1..=600
        assert_validation_400(
            &UpdateVoiceSettingsRequest { drain_timeout_secs: Some(0), ..base() },
            "drain below range",
        );
        // max_clip_seconds: 1..=3600
        assert_validation_400(
            &UpdateVoiceSettingsRequest { max_clip_seconds: Some(0), ..base() },
            "max_clip below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { max_clip_seconds: Some(3_601), ..base() },
            "max_clip above range",
        );
        // max_upload_bytes: 1024..=67108864
        assert_validation_400(
            &UpdateVoiceSettingsRequest { max_upload_bytes: Some(1_023), ..base() },
            "max_upload below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { max_upload_bytes: Some(67_108_865), ..base() },
            "max_upload above range",
        );
        // stream_interval_ms: 300..=10000 (TEST-1)
        assert_validation_400(
            &UpdateVoiceSettingsRequest { stream_interval_ms: Some(299), ..base() },
            "stream_interval below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { stream_interval_ms: Some(10_001), ..base() },
            "stream_interval above range",
        );
        // stream_max_decode_secs: 5..=600
        assert_validation_400(
            &UpdateVoiceSettingsRequest { stream_max_decode_secs: Some(4), ..base() },
            "max_decode below range",
        );
        assert_validation_400(
            &UpdateVoiceSettingsRequest { stream_max_decode_secs: Some(601), ..base() },
            "max_decode above range",
        );
        // Boundary values are inclusive-OK.
        assert!(validate_settings_patch(&UpdateVoiceSettingsRequest {
            idle_unload_secs: Some(0),
            max_clip_seconds: Some(3_600),
            max_upload_bytes: Some(67_108_864),
            stream_interval_ms: Some(300),
            ..base()
        })
        .is_ok());
        assert!(validate_settings_patch(&UpdateVoiceSettingsRequest {
            stream_interval_ms: Some(10_000),
            streaming_enabled: Some(false),
            ..base()
        })
        .is_ok());
    }
}

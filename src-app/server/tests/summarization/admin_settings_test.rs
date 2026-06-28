// ============================================================================
// Summarization admin-settings integration tests.
//
// The pure logic of `decide_summarize_action`, `build_transcript`, and
// `apply_summary_block` lives in `#[cfg(test)] mod tests` inside
// `src/modules/summarization/engine/summarizer.rs` (19 tests, all
// pure, ~0ms). These integration tests cover the REST surface for
// the admin-tunable knobs that drive the summarizer:
//
//   * enable round-trip
//   * default_summarization_model_id round-trip incl. NULL (zero-config)
//   * summarize_after_tokens / summarizer_keep_recent_tokens round-trip
//     + range checks + the keep < trigger invariant
//   * full_summary_prompt / incremental_summary_prompt placeholder validation
//   * NULL-as-fallback semantics (clearing back to the compiled defaults)
//   * permission gate (non-admin gets 403)
//
// The actual chat → summarizer → LLM → apply pipeline is a Tier-5
// manual exercise — see `real_llm_test.rs`.
// ============================================================================

use serde_json::{Value, json};

async fn admin_user(
    name: &str,
) -> (crate::common::TestServer, crate::common::test_helpers::TestUser) {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        name,
        &[
            "summarization::settings::read",
            "summarization::settings::manage",
        ],
    )
    .await;
    (server, user)
}

#[tokio::test]
async fn test_enabled_round_trip() {
    let (server, admin) = admin_user("summ_enabled").await;

    // Default is TRUE per migration 91 — toggle it to FALSE.
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], false);

    // Round-trip via GET.
    let get = reqwest::Client::new()
        .get(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    let body: Value = get.json().await.unwrap();
    assert_eq!(body["enabled"], false);
}

#[tokio::test]
async fn test_default_summarization_model_round_trip_and_clear() {
    let (server, admin) = admin_user("summ_model").await;

    // Start at NULL (the migration-seeded default — zero-config).
    let row: Value = reqwest::Client::new()
        .get(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(row["default_summarization_model_id"].is_null());

    // Set + clear via explicit null tri-state.
    let cleared = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_summarization_model_id": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(cleared.status(), 200);
    let row: Value = cleared.json().await.unwrap();
    assert!(row["default_summarization_model_id"].is_null());
}

#[tokio::test]
async fn test_threshold_round_trip() {
    let (server, admin) = admin_user("summ_thresholds").await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "summarize_after_tokens": 7500,
            "summarizer_keep_recent_tokens": 1500,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["summarize_after_tokens"], 7500);
    assert_eq!(row["summarizer_keep_recent_tokens"], 1500);
}

#[tokio::test]
async fn test_threshold_range_validation_returns_400() {
    let (server, admin) = admin_user("summ_range").await;

    // Below minimum (500).
    let too_low = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarize_after_tokens": 100 }))
        .send()
        .await
        .unwrap();
    assert_eq!(too_low.status(), 400);

    // Above maximum (1_000_000).
    let too_high = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarize_after_tokens": 2_000_000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(too_high.status(), 400);

    // keep_recent must be >= 100.
    let keep_low = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarizer_keep_recent_tokens": 50 }))
        .send()
        .await
        .unwrap();
    assert_eq!(keep_low.status(), 400);
}

#[tokio::test]
async fn test_keep_recent_must_be_below_trigger() {
    // Effective-keep < effective-trigger invariant checked in the handler
    // before the DB CHECK gets a chance to bubble up as a 500.
    let (server, admin) = admin_user("summ_invariant").await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "summarize_after_tokens": 5000,
            "summarizer_keep_recent_tokens": 5000, // not < 5000
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        400,
        "keep_recent >= trigger must return 400 from the handler"
    );

    // And the case where only keep is updated, but the prior trigger
    // is below the new keep value — still 400 because the effective
    // values violate the invariant.
    let _ = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarize_after_tokens": 5000 }))
        .send()
        .await
        .unwrap();
    let bad_keep = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "summarizer_keep_recent_tokens": 6000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_keep.status(), 400);
}

#[tokio::test]
async fn test_full_prompt_validation_missing_transcript_placeholder() {
    let (server, admin) = admin_user("summ_full_prompt").await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "full_summary_prompt": "Summarize the conversation:",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        400,
        "full_summary_prompt without {{transcript}} placeholder must be rejected"
    );
}

#[tokio::test]
async fn test_incremental_prompt_validation_missing_placeholders() {
    let (server, admin) = admin_user("summ_inc_prompt").await;

    // Missing {previous_summary}.
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "incremental_summary_prompt":
                "Update summary with these new turns: {new_transcript}",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Missing {new_transcript}.
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "incremental_summary_prompt":
                "Update {previous_summary} with new turns.",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_prompt_length_cap_returns_400() {
    // The validation chain caps each prompt override at MAX_PROMPT_LEN
    // (32 KiB) — these templates are injected into every summarization
    // LLM call, so an unbounded value is a cost/DoS vector. The existing
    // prompt tests only exercise the *placeholder* branch; the length
    // cap (handlers.rs `s.len() > MAX_PROMPT_LEN`) was uncovered. Build
    // an over-cap string that STILL contains the required placeholders,
    // so a rejection can only have come from the length check, not the
    // placeholder check that runs after it.
    let (server, admin) = admin_user("summ_prompt_len").await;

    // 40 KiB of filler (> the 32 KiB cap) plus the required placeholder.
    let oversized_full = format!("{} {{transcript}}", "x".repeat(40_000));
    let full_res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "full_summary_prompt": oversized_full }))
        .send()
        .await
        .unwrap();
    let status = full_res.status();
    let body = full_res.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "over-cap full_summary_prompt must return 400, got {status}: {body}"
    );
    assert!(
        body.contains("32 KiB") || body.contains("limit"),
        "error body should mention the size limit, got: {body}"
    );

    // Same for the incremental prompt — over-cap but carrying BOTH
    // required placeholders, so only the length cap can reject it.
    let oversized_inc = format!(
        "{} {{previous_summary}} {{new_transcript}}",
        "y".repeat(40_000)
    );
    let inc_res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "incremental_summary_prompt": oversized_inc }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        inc_res.status(),
        400,
        "over-cap incremental_summary_prompt must return 400"
    );

    // Positive control: a valid (under-cap, placeholder-bearing) prompt
    // still succeeds, proving the cap rejects size specifically and not
    // the whole prompt-override path.
    let ok_res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "full_summary_prompt": "Summarize: {transcript}" }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok_res.status(), 200, "an under-cap prompt must still save");
}

#[tokio::test]
async fn test_prompt_override_round_trip_and_clear() {
    let (server, admin) = admin_user("summ_prompt_clear").await;

    // Set valid overrides.
    let good_full = "Summarize this conversation: {transcript}";
    let good_inc = "Update {previous_summary} using {new_transcript}";
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "full_summary_prompt": good_full,
            "incremental_summary_prompt": good_inc,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["full_summary_prompt"], good_full);
    assert_eq!(row["incremental_summary_prompt"], good_inc);

    // Clear back to NULL (compiled-in default) via explicit null.
    let cleared = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "full_summary_prompt": null,
            "incremental_summary_prompt": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(cleared.status(), 200);
    let row: Value = cleared.json().await.unwrap();
    assert!(row["full_summary_prompt"].is_null());
    assert!(row["incremental_summary_prompt"].is_null());
}

#[tokio::test]
async fn test_admin_endpoints_require_summarization_settings_permission() {
    // A user WITHOUT `summarization::settings::{read,manage}` must hit
    // 403 on both endpoints — `RequirePermissions` is the gate.
    let server = crate::common::TestServer::start().await;
    let unauthorized = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_no_perm",
        &[],
    )
    .await;

    let get_res = reqwest::Client::new()
        .get(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", unauthorized.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res.status(), 403);

    let put_res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", unauthorized.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(put_res.status(), 403);
}

#[tokio::test]
async fn test_default_model_id_rejects_non_chat_capable_model_returns_400() {
    // Pointing `default_summarization_model_id` at an embedding-only
    // (or otherwise non-chat-capable) model must return 400 — the
    // engine could not produce a summary against it, and the failure
    // would only surface as a `tracing::warn` at chat-turn time on a
    // live deployment. Seeded directly via SQL to avoid pulling in the
    // full provider-create API surface.
    let (server, admin) = admin_user("summ_default_model_capability").await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    let provider_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO llm_providers (name, provider_type, enabled)
         VALUES ('summ-cap-test', 'openai', true)
         RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("seed llm_provider");

    let model_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO llm_models
            (provider_id, name, display_name, capabilities, enabled)
         VALUES ($1, 'embed-only', 'Embed Only',
                 '{\"text_embedding\": true}'::jsonb, true)
         RETURNING id",
    )
    .bind(provider_id)
    .fetch_one(&pool)
    .await
    .expect("seed llm_model");

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_summarization_model_id": model_id.to_string() }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "non-chat-capable model_id must return 400, got {status}: {body}"
    );
    assert!(
        body.contains("chat-capable"),
        "error body should mention the chat capability gate, got: {body}"
    );
}

#[tokio::test]
async fn test_default_model_id_validates_existence_returns_400() {
    // Setting `default_summarization_model_id` to a UUID that doesn't
    // exist in `llm_models` must return 400 (the handler pre-checks),
    // not a raw 500 from a deferred FK violation in the DB.
    let (server, admin) = admin_user("summ_default_model_fk").await;

    let ghost = uuid::Uuid::new_v4();
    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_summarization_model_id": ghost.to_string() }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        400,
        "non-existent default_summarization_model_id must return 400"
    );
}

#[tokio::test]
async fn test_read_only_user_can_get_but_not_put() {
    // A user with `summarization::settings::read` but NOT `::manage`
    // must succeed on GET (200) and fail on PUT (403). This exercises
    // the per-endpoint perm split — granting read alone isn't enough
    // to mutate the singleton.
    let server = crate::common::TestServer::start().await;
    let read_only = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_read_only",
        &["summarization::settings::read"],
    )
    .await;

    let get_res = reqwest::Client::new()
        .get(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", read_only.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get_res.status(), 200, "read perm alone must allow GET");

    let put_res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", read_only.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        put_res.status(),
        403,
        "read perm alone must NOT allow PUT — manage perm required"
    );
}

// ── Sync emission (gap 2b4d98f76c40 — SummarizationAdminSettings) ────────────

/// A settings PUT must publish `summarization_admin_settings`/`update` to
/// holders of `summarization::settings::read` (handlers.rs:194-200, audience
/// `Audience::perm::<SummarizationSettingsRead>()`), and must NOT reach a user
/// without that read perm. Closes the SyncEntity::SummarizationAdminSettings
/// emit-coverage gap.
#[tokio::test]
async fn test_summarization_settings_update_emits_sync_to_admins_only() {
    use crate::common::sync_probe::SyncProbe;
    use std::time::Duration;
    use uuid::Uuid;

    let (server, admin) = admin_user("summ_sync_admin").await;
    // Plain user: subscribes, but lacks summarization::settings::read.
    let plain =
        crate::common::test_helpers::create_user_with_permissions(&server, "summ_sync_plain", &[])
            .await;

    let mut admin_probe = SyncProbe::open(&server, &admin.token).await;
    let mut plain_probe = SyncProbe::open(&server, &plain.token).await;

    let res = reqwest::Client::new()
        .put(server.api_url("/summarization/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let frame = admin_probe
        .expect_event("summarization_admin_settings", "update", Duration::from_secs(5))
        .await;
    // Singleton row → nil wire id (notify-and-refetch).
    assert_eq!(frame.id, Uuid::nil().to_string());

    plain_probe.expect_silence(Duration::from_secs(1)).await;
}

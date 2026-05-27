// ============================================================================
// Summarizer integration tests.
//
// The pure-logic of `decide_summarize_action`, `build_transcript`, and
// `apply_summary_block` lives in `#[cfg(test)] mod tests` inside
// `src/modules/chat/extensions/memory/summarizer.rs` (16 tests, all
// pure, ~0ms). These integration tests cover the REST surface for
// the admin-tunable knobs that drive the summarizer:
//
//   * default_extraction_model_id round-trip (the model the summarizer uses)
//   * summarize_after_n_messages / summarizer_keep_recent round-trip + CHECK
//   * full_summary_prompt / incremental_summary_prompt placeholder validation
//   * NULL-as-fallback semantics (clearing back to the compiled-in defaults)
//
// The actual chat → summarizer → LLM → apply pipeline is a Tier-5
// manual exercise (needs a real LLM).
// ============================================================================

use serde_json::{Value, json};

async fn admin_user(
    suffix: &str,
) -> (crate::common::TestServer, crate::common::test_helpers::TestUser) {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        suffix,
        &["memory::admin::read", "memory::admin::manage"],
    )
    .await;
    (server, admin)
}

#[tokio::test]
async fn test_default_extraction_model_round_trip() {
    let (server, admin) = admin_user("summ_admin").await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "default_extraction_model_id": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["default_extraction_model_id"].is_null());
}

#[tokio::test]
async fn test_summarizer_threshold_round_trip() {
    let (server, admin) = admin_user("summ_thresholds").await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "summarize_after_n_messages": 75,
            "summarizer_keep_recent": 15,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["summarize_after_n_messages"], 75);
    assert_eq!(row["summarizer_keep_recent"], 15);
}

#[tokio::test]
async fn test_summarizer_threshold_check_constraint_keep_recent_below_trigger() {
    // The migration enforces summarizer_keep_recent < summarize_after_n_messages
    // via a row-level CHECK. Sending an invalid pair must fail (the
    // sqlx error path bubbles up as a 5xx, but the row in the DB is
    // unchanged — that's the property we care about).
    let (server, admin) = admin_user("summ_check").await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "summarize_after_n_messages": 50,
            "summarizer_keep_recent": 50,  // not < 50
        }))
        .send()
        .await
        .unwrap();
    // CHECK violations come back as 500 (sqlx surfaces them as
    // generic db errors); the contract here is just "the write
    // didn't land". Re-fetch to confirm.
    assert!(
        res.status() == 500 || res.status() == 400,
        "expected check-constraint failure, got {}",
        res.status()
    );

    let after: Value = reqwest::Client::new()
        .get(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Either the defaults (50/10) or whatever was there before — but
    // NOT (50/50), which would mean the bad write landed.
    assert_ne!(after["summarizer_keep_recent"], 50);
}

#[tokio::test]
async fn test_full_summary_prompt_validation_missing_placeholder() {
    let (server, admin) = admin_user("summ_full_validate").await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "full_summary_prompt": "Summarize this. (no placeholder!)",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
    let body: Value = res.json().await.unwrap();
    let msg = body["message"].as_str().or(body["error"].as_str()).unwrap_or("");
    assert!(
        msg.contains("{transcript}"),
        "expected error mentioning {{transcript}}, got: {msg}"
    );
}

#[tokio::test]
async fn test_incremental_prompt_validation_missing_placeholder() {
    let (server, admin) = admin_user("summ_inc_validate").await;
    // Missing {new_transcript}.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "incremental_summary_prompt":
                "Update this: {previous_summary} (but I forgot to ask for new_transcript)",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Missing {previous_summary} too.
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "incremental_summary_prompt":
                "Just rewrite based on {new_transcript}, ignore history",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);
}

#[tokio::test]
async fn test_prompt_override_round_trip_and_clear() {
    let (server, admin) = admin_user("summ_prompt_rt").await;
    let client = reqwest::Client::new();
    let url = server.api_url("/memory/admin-settings");
    let tok = format!("Bearer {}", admin.token);

    // 1. Defaults are NULL.
    let row: Value = client
        .get(&url)
        .header("Authorization", &tok)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(row["full_summary_prompt"].is_null());
    assert!(row["incremental_summary_prompt"].is_null());

    // 2. Set a valid override on both.
    let full_p = "Custom full prompt with {transcript} placeholder.";
    let inc_p = "Custom incremental prompt: {previous_summary} | {new_transcript}";
    let res = client
        .put(&url)
        .header("Authorization", &tok)
        .json(&json!({
            "full_summary_prompt": full_p,
            "incremental_summary_prompt": inc_p,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["full_summary_prompt"], full_p);
    assert_eq!(row["incremental_summary_prompt"], inc_p);

    // 3. Clear via explicit null → both back to NULL (default).
    let res = client
        .put(&url)
        .header("Authorization", &tok)
        .json(&json!({
            "full_summary_prompt": null,
            "incremental_summary_prompt": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["full_summary_prompt"].is_null());
    assert!(row["incremental_summary_prompt"].is_null());

    // 4. Clear via empty string → also back to NULL (handler
    //    normalizes empty → null, avoiding a "" prompt being persisted
    //    and silently producing empty LLM requests).
    let res = client
        .put(&url)
        .header("Authorization", &tok)
        .json(&json!({
            "full_summary_prompt": "",
            "incremental_summary_prompt": "",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert!(row["full_summary_prompt"].is_null());
    assert!(row["incremental_summary_prompt"].is_null());
}

#[tokio::test]
async fn test_prompt_override_requires_admin_permission() {
    let server = crate::common::TestServer::start().await;
    // Regular user with only memory::read + memory::write — no admin grants.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_no_admin",
        &["memory::read", "memory::write"],
    )
    .await;
    let res = reqwest::Client::new()
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "full_summary_prompt": "Anything with {transcript}.",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

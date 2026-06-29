use serde_json::Value;

fn completed_guides(body: &Value) -> Vec<String> {
    body.get("completed_guide_ids")
        .and_then(|v| v.as_array())
        .expect("response should have completed_guide_ids array")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

fn completed_steps(body: &Value) -> Vec<String> {
    body.get("completed_step_ids")
        .and_then(|v| v.as_array())
        .expect("response should have completed_step_ids array")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

#[tokio::test]
async fn test_complete_guide_marks_guide_completed() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_guide", &[]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        completed_guides(&body).contains(&"getting-started".to_string()),
        "completed_guide_ids should contain the guide id"
    );
}

#[tokio::test]
async fn test_complete_guide_is_idempotent() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_idem", &[]).await;
    let url = server.api_url("/onboarding/getting-started/complete");

    let mut last: Value = Value::Null;
    for _ in 0..2 {
        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 200);
        last = response.json().await.expect("Failed to parse JSON");
    }

    let count = completed_guides(&last)
        .iter()
        .filter(|g| *g == "getting-started")
        .count();
    assert_eq!(count, 1, "guide id must not be duplicated on repeat completion");
}

#[tokio::test]
async fn test_complete_guide_empty_id_is_rejected() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_empty", &[]).await;

    // %20 decodes to a single space → trims to empty → validation error.
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/%20/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "whitespace guide_id should be rejected");
}

#[tokio::test]
async fn test_complete_guide_step_marks_step_completed() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_step", &[]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/steps/welcome/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        completed_steps(&body).contains(&"getting-started/welcome".to_string()),
        "completed_step_ids should contain the `guide/step` key"
    );
}

#[tokio::test]
async fn test_complete_guide_step_idempotent_and_validates() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_step2", &[]).await;
    let url = server.api_url("/onboarding/getting-started/steps/welcome/complete");

    let mut last: Value = Value::Null;
    for _ in 0..2 {
        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 200);
        last = response.json().await.expect("Failed to parse JSON");
    }
    let count = completed_steps(&last)
        .iter()
        .filter(|s| *s == "getting-started/welcome")
        .count();
    assert_eq!(count, 1, "step key must not be duplicated");

    // Empty step id → 400.
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/steps/%20/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 400, "whitespace step_id should be rejected");
}

#[tokio::test]
async fn test_onboarding_endpoints_require_permission_and_auth() {
    let server = crate::common::TestServer::start().await;
    let no_perm =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "onb_noperm").await;

    let guide_url = server.api_url("/onboarding/getting-started/complete");
    let step_url = server.api_url("/onboarding/getting-started/steps/welcome/complete");

    for url in [&guide_url, &step_url] {
        // No ProfileEdit → 403.
        let response = reqwest::Client::new()
            .post(url)
            .header("Authorization", format!("Bearer {}", no_perm.token))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 403, "user without ProfileEdit should be forbidden: {url}");

        // No token → 401.
        let response = reqwest::Client::new()
            .post(url)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 401, "unauthenticated request should be 401: {url}");
    }
}

#[tokio::test]
async fn test_get_progress_empty_for_fresh_user() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_fresh", &[]).await;

    let response = reqwest::Client::new()
        .get(server.api_url("/onboarding/progress"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(completed_guides(&body).is_empty(), "fresh user has no guides");
    assert!(completed_steps(&body).is_empty(), "fresh user has no steps");
}

#[tokio::test]
async fn test_get_progress_reflects_completion() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_reflect", &[]).await;

    reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    let response = reqwest::Client::new()
        .get(server.api_url("/onboarding/progress"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(
        completed_guides(&body).contains(&"getting-started".to_string()),
        "GET progress should reflect the completed guide"
    );
}

#[tokio::test]
async fn test_get_progress_is_authentication_only() {
    // Gate split: a user with NO permissions can still read their own
    // progress (auth-only) but cannot POST a completion (ProfileEdit).
    let server = crate::common::TestServer::start().await;
    let no_perm =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "onb_getnoperm").await;

    let response = reqwest::Client::new()
        .get(server.api_url("/onboarding/progress"))
        .header("Authorization", format!("Bearer {}", no_perm.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        200,
        "GET progress is auth-only; no-permission user should still read it"
    );

    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/complete"))
        .header("Authorization", format!("Bearer {}", no_perm.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        403,
        "POST complete still requires ProfileEdit"
    );

    // Unauthenticated GET → 401.
    let response = reqwest::Client::new()
        .get(server.api_url("/onboarding/progress"))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 401, "unauthenticated GET progress should be 401");
}

// audit id all-3339417ae42b — guide_id validation edge cases beyond the
// whitespace/empty case: invalid characters and over-length ids must 400.
#[tokio::test]
async fn test_complete_guide_invalid_chars_rejected() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_badchars", &[]).await;

    // Uppercase letters are not in the slug allowlist (lowercase/digit/-/_).
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/Getting-Started/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 400, "uppercase guide_id should be rejected");

    // A dot is outside the allowlist too.
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/bad.id/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 400, "dotted guide_id should be rejected");
}

#[tokio::test]
async fn test_complete_guide_overlong_id_rejected() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_long", &[]).await;

    // 65 chars > MAX_ONBOARDING_ID_LEN (64).
    let long_id = "a".repeat(65);
    let response = reqwest::Client::new()
        .post(server.api_url(&format!("/onboarding/{long_id}/complete")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 400, "over-length guide_id should be rejected");
}

#[tokio::test]
async fn test_complete_step_invalid_step_id_rejected() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_badstep", &[]).await;

    // Valid guide_id, invalid step_id (uppercase) → 400.
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/steps/BadStep/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 400, "invalid step_id should be rejected");
}

#[tokio::test]
async fn test_completion_cardinality_cap_rejects_over_256() {
    // The handler caps both arrays at MAX_ONBOARDING_COMPLETIONS (256):
    // complete_guide rejects with 400 ONBOARDING_LIMIT once
    // completed_guide_ids.len() >= 256 (handlers.rs:84-90), and
    // complete_guide_step does the same for completed_step_ids
    // (handlers.rs:139-145). Rather than issue 256 sequential POSTs, we
    // seed the user_onboarding row at EXACTLY the cap directly, then prove a
    // single further distinct completion is refused at the cardinality guard.
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_cap", &[]).await;
    let user_id = uuid::Uuid::parse_str(&user.user_id).expect("user_id is a valid uuid");

    // 256 distinct guide ids and 256 distinct step keys, all matching the
    // [a-z0-9_-] / "{guide}/{step}" formats the handler accepts.
    let guides: Vec<String> = (0..256).map(|i| format!("g{i}")).collect();
    let steps: Vec<String> = (0..256).map(|i| format!("getting-started/s{i}")).collect();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");
    sqlx::query(
        "INSERT INTO user_onboarding (user_id, completed_guide_ids, completed_step_ids)
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&guides)
    .bind(&steps)
    .execute(&pool)
    .await
    .expect("Failed to seed 256 completions at the cap");

    // 257th distinct GUIDE → 400 with the cap's error code (not a 200, and
    // not a VALIDATION_ERROR — proving it's the cardinality guard that fired).
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/over-the-cap/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        400,
        "the 257th distinct guide must be rejected at the cap"
    );
    let body: Value = response.json().await.expect("Failed to parse error body");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("ONBOARDING_LIMIT"),
        "guide rejection must be the cardinality cap, not validation: {body}"
    );

    // 257th distinct STEP → same cap on completed_step_ids.
    let response = reqwest::Client::new()
        .post(server.api_url("/onboarding/getting-started/steps/over-the-cap/complete"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        400,
        "the 257th distinct step must be rejected at the cap"
    );
    let body: Value = response.json().await.expect("Failed to parse error body");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("ONBOARDING_LIMIT"),
        "step rejection must be the cardinality cap, not validation: {body}"
    );

    // The cap holds the stored sets at exactly 256 — the over-cap entries
    // were never appended.
    let response = reqwest::Client::new()
        .get(server.api_url("/onboarding/progress"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        completed_guides(&body).len(),
        256,
        "guide set must stay capped at 256"
    );
    assert_eq!(
        completed_steps(&body).len(),
        256,
        "step set must stay capped at 256"
    );
    assert!(
        !completed_guides(&body).contains(&"over-the-cap".to_string()),
        "the rejected guide must not have been stored"
    );
}

/// The per-user completion cardinality cap (MAX_ONBOARDING_COMPLETIONS) is
/// enforced ATOMICALLY in the repository via `cardinality(...) < $3` inside the
/// upsert — not just by the handler's pre-check. Drives the real repo fn with a
/// small cap so the test is cheap: the (cap+1)-th distinct guide is NOT appended.
#[tokio::test]
#[serial_test::serial(repos)]
async fn completion_cardinality_cap_is_enforced_in_the_repository() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "onb_cap", &[]).await;
    let uid = uuid::Uuid::parse_str(&user.user_id).unwrap();

    // Cap = 2: the first two distinct guides land, the third is refused by the
    // atomic `cardinality < $3` guard (no error — the append simply no-ops).
    ziee::Repos.onboarding.complete_guide(uid, "guide-a", 2).await.expect("a");
    ziee::Repos.onboarding.complete_guide(uid, "guide-b", 2).await.expect("b");
    let progress = ziee::Repos.onboarding.complete_guide(uid, "guide-c", 2).await.expect("c");

    assert_eq!(
        progress.completed_guide_ids.len(),
        2,
        "the cap must block the 3rd completion; got {:?}",
        progress.completed_guide_ids
    );
    assert!(progress.completed_guide_ids.contains(&"guide-a".to_string()));
    assert!(progress.completed_guide_ids.contains(&"guide-b".to_string()));
    assert!(
        !progress.completed_guide_ids.contains(&"guide-c".to_string()),
        "the over-cap guide must NOT be appended"
    );

    // Idempotent re-completion of an already-present guide stays at the cap.
    let again = ziee::Repos.onboarding.complete_guide(uid, "guide-a", 2).await.expect("re-a");
    assert_eq!(again.completed_guide_ids.len(), 2, "re-completing an existing guide is a no-op");
}


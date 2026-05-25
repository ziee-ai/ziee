// ============================================================================
// Onboarding module endpoint tests
//
//   POST /api/onboarding/{guide_id}/complete                  (ProfileRead)
//   POST /api/onboarding/{guide_id}/steps/{step_id}/complete  (ProfileRead)
//
// Both return the updated User. A normally-registered user has ProfileRead via
// the default "Users" group, so the happy-path tests use a vanilla user; the
// 403 test strips all groups.
// ============================================================================

use serde_json::Value;

fn completed_guides(body: &Value) -> Vec<String> {
    body.get("completed_onboarding_ids")
        .and_then(|v| v.as_array())
        .expect("response should have completed_onboarding_ids array")
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect()
}

fn completed_steps(body: &Value) -> Vec<String> {
    body.get("completed_onboarding_step_ids")
        .and_then(|v| v.as_array())
        .expect("response should have completed_onboarding_step_ids array")
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
        "completed_onboarding_ids should contain the guide id"
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
        "completed_onboarding_step_ids should contain the `guide/step` key"
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
        // No ProfileRead → 403.
        let response = reqwest::Client::new()
            .post(url)
            .header("Authorization", format!("Bearer {}", no_perm.token))
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 403, "user without ProfileRead should be forbidden: {url}");

        // No token → 401.
        let response = reqwest::Client::new()
            .post(url)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(response.status(), 401, "unauthenticated request should be 401: {url}");
    }
}

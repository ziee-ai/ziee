// ============================================================================
// GET /api/conversations/{id}/summary — read the active-branch summary.
//
// Three tests:
//   - null-when-none: a conversation with no summary row returns
//     `null` (not 404) so the frontend can render "no summary yet"
//     uniformly.
//   - seeded-round-trip: write a `conversation_summaries` row directly
//     via SQL, then assert the endpoint returns the same fields.
//   - 404-on-other-user: ownership-gated; intruder gets 404 to defeat
//     probing.
// ============================================================================

use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn create_conversation(
    server: &crate::common::TestServer,
    token: &str,
) -> String {
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "title": "summary endpoint test" }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "create conversation failed: {}",
        res.status()
    );
    let row: Value = res.json().await.unwrap();
    row["id"].as_str().expect("conversation id").to_string()
}

async fn open_pool(server: &crate::common::TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test DB")
}

async fn active_branch_id(pool: &PgPool, conversation_id: Uuid) -> Uuid {
    sqlx::query_scalar!(
        r#"SELECT active_branch_id as "active_branch_id!" FROM conversations WHERE id = $1"#,
        conversation_id
    )
    .fetch_one(pool)
    .await
    .expect("active_branch_id on conversation")
}

#[tokio::test]
async fn test_summary_endpoint_returns_null_when_no_row() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_endpoint_null",
        &["conversations::read"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summary")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body.is_null(), "no summary row should return null, got {body}");
}

#[tokio::test]
async fn test_summary_endpoint_returns_seeded_row() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_endpoint_seed",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id_str = create_conversation(&server, &user.token).await;
    let conv_id = Uuid::parse_str(&conv_id_str).unwrap();
    let pool = open_pool(&server).await;
    let branch_id = active_branch_id(&pool, conv_id).await;

    // Seed a summary row directly via SQL — the engine's
    // `upsert_summary` is the only writer in production but we bypass
    // it here to keep the endpoint test focused.
    sqlx::query!(
        r#"
        INSERT INTO conversation_summaries
            (branch_id, summary_text, summarized_up_to_id, message_count, model_used)
        VALUES ($1, $2, NULL, 42, 'test-model')
        "#,
        branch_id,
        "user mentioned a trip to tokyo and a dog named sneezles"
    )
    .execute(&pool)
    .await
    .expect("seed conversation_summaries");

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id_str}/summary")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        body["summary_text"].as_str(),
        Some("user mentioned a trip to tokyo and a dog named sneezles")
    );
    assert_eq!(body["message_count"], 42);
    assert_eq!(body["model_used"], "test-model");
    assert_eq!(body["branch_id"], branch_id.to_string());
}

#[tokio::test]
async fn test_summary_endpoint_returns_404_for_other_users_conversation() {
    let server = crate::common::TestServer::start().await;
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_endpoint_owner",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let intruder = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_endpoint_intruder",
        &["conversations::read"],
    )
    .await;
    let conv_id = create_conversation(&server, &owner.token).await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conv_id}/summary")))
        .header("Authorization", format!("Bearer {}", intruder.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        404,
        "intruder must get 404 (conflated to defeat probing)"
    );
}

#[tokio::test]
async fn test_summary_endpoint_returns_404_for_nonexistent_conversation() {
    // A random UUID not present in `conversations` must also return 404
    // — same response shape as a wrong-owner request, so the endpoint
    // can't be used to enumerate ids.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_endpoint_404",
        &["conversations::read"],
    )
    .await;

    let ghost = Uuid::new_v4();
    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{ghost}/summary")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "GET on ghost id must be 404");
}

// audit id all-0b8f4496681d — GET /conversations/{id}/summary is gated by
// ConversationsRead (handlers.rs:210-212); only ownership (404) was tested. A
// user lacking conversations::read must be refused with 403 — the perm gate
// fires before existence/ownership, so any id suffices.
#[tokio::test]
async fn test_summary_endpoint_requires_conversations_read() {
    let server = crate::common::TestServer::start().await;
    // Default group removed → no conversations::read.
    let user = crate::common::test_helpers::create_user_with_only_permissions(
        &server,
        "summ_endpoint_noperm",
#[tokio::test]
async fn test_summary_endpoint_requires_conversations_read_permission() {
    // The endpoint is gated by `ConversationsRead` (handlers.rs:232). The
    // existing tests cover ownership (404) but never the permission gate:
    // a user WITHOUT `conversations::read` must be refused 403 by the
    // RequirePermissions extractor BEFORE the ownership/existence check
    // runs — so even a valid, real conversation id owned by someone else
    // yields 403 (perm gate), not 404 (ownership).
    let server = crate::common::TestServer::start().await;

    // Owner has read+edit and creates a real conversation (valid id).
    let owner = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_perm_owner",
        &["conversations::read", "conversations::edit"],
    )
    .await;
    let conv_id = create_conversation(&server, &owner.token).await;

    // This user is intentionally missing `conversations::read` (only a
    // profile perm so the account is valid/active).
    let no_read = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_perm_no_read",
        &["profile::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}/summary", Uuid::new_v4())))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "missing conversations::read must be 403");
        .get(server.api_url(&format!("/conversations/{conv_id}/summary")))
        .header("Authorization", format!("Bearer {}", no_read.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        403,
        "a user lacking conversations::read must get 403 (perm gate fires before ownership)"
    );
}

// ============================================================================
// POST /_test/summarization/refresh — the debug-only synchronous refresh hook
// (handlers.rs:`test_refresh`, registered in routes.rs only under
// `#[cfg(debug_assertions)]`, which `cargo test` builds with).
//
// The full LLM-driven summarization is exercised key-gated in
// `real_llm_test.rs::trigger_refresh_via_test_hook`; these two tests cover the
// endpoint's DETERMINISTIC surface that runs in every CI with no provider key:
//   - it is permission-gated (`summarization::settings::manage`), and
//   - on a branch with nothing to summarize it drives the real handler →
//     `refresh_summary` Noop early-return → 200 `{ "ok": true }` (no LLM call:
//     the model is loaded only AFTER the Noop check, so the model id is never
//     dereferenced on this path).
// ============================================================================

#[tokio::test]
async fn test_refresh_endpoint_requires_manage_permission() {
    let server = crate::common::TestServer::start().await;

    // A user with conversation perms but NOT summarization::settings::manage.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_refresh_nogate",
        &["conversations::read", "conversations::create"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/_test/summarization/refresh"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "branch_id": Uuid::new_v4(),
            "model_id": Uuid::new_v4(),
        }))
        .send()
        .await
        .unwrap();

    // 403 (not 404): proves the debug route IS registered in the test build
    // AND that it is gated by `RequirePermissions<(SummarizationSettingsManage,)>`.
    assert_eq!(
        res.status(),
        403,
        "test_refresh must require summarization::settings::manage (403), got {}: {}",
        res.status(),
        res.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn test_refresh_endpoint_noop_branch_returns_ok() {
    let server = crate::common::TestServer::start().await;

    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_refresh_ok",
        &[
            "conversations::read",
            "conversations::create",
            "summarization::settings::manage",
        ],
    )
    .await;

    // A fresh conversation → an active branch with no summarizable messages, so
    // `decide_summarize_action` returns `Noop` and `refresh_summary` returns
    // `Ok(())` before ever loading/calling the model.
    let conv_id = create_conversation(&server, &user.token).await;
    let pool = open_pool(&server).await;
    let branch_id = active_branch_id(&pool, Uuid::parse_str(&conv_id).unwrap()).await;

    let res = reqwest::Client::new()
        .post(server.api_url("/_test/summarization/refresh"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            // model_id is never dereferenced on the Noop path (model load
            // happens after the Noop early-return) — a random uuid is fine.
            "branch_id": branch_id,
            "model_id": Uuid::new_v4(),
        }))
        .send()
        .await
        .unwrap();

    let status = res.status();
    let body: Value = res.json().await.unwrap();
    assert_eq!(
        status, 200,
        "test_refresh on an empty branch must succeed via the Noop path, got {status}: {body}"
    );
    assert_eq!(
        body,
        json!({ "ok": true }),
        "the handler returns {{\"ok\":true}} on success"
    );
}

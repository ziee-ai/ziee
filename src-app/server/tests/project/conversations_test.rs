//! Conversation ↔ project relation: attach/detach endpoint coverage
//! + list scoping + FK cascade behavior. Single file per
//! resource-pairing convention (compare `files_test.rs` for the
//! project↔file pair).
//!
//! Legacy tests that exercised `POST /conversations` with a body
//! `project_id`, `PUT /conversations/{id}` for moves,
//! `?project_id=<uuid>` query filtering, and backend default_model_id
//! snapshot-on-create were deleted as part of the chat↔project
//! decoupling — those code paths no longer exist:
//!   - chat creates UNFILED conversations only; project membership
//!     is set via `POST /projects/{id}/conversations/{conv_id}`.
//!   - chat's `GET /conversations` always returns unfiled-only; no
//!     `?project_id` query param.
//!   - default_model_id resolution moved to the frontend; E2E specs
//!     cover the seed-from-project-default flow.

use reqwest::StatusCode;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::helpers;
use crate::common::TestServer;
use crate::common::test_helpers::TestUser;

// ============================================================
// Test infrastructure helpers
// ============================================================

/// Open a small connection pool against the same test DB the server
/// is using. Used to inspect `conversation_mcp_settings` + the
/// conversation row directly.
async fn pool(server: &TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap()
}

/// Set a project's MCP defaults to a distinct shape (auto_approve)
/// so attach-time snapshot assertions can distinguish "from project
/// defaults" vs "from baseline manual_approve."
async fn set_distinct_mcp_defaults(server: &TestServer, user: &TestUser, project_id: &str) {
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/projects/{}/mcp-settings", project_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [],
            "disabled_servers": [],
            "loop_settings": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "set mcp settings: {}",
        resp.text().await.unwrap_or_default()
    );
}

async fn db_project_id_for_conversation(
    pool: &sqlx::PgPool,
    conversation_id: Uuid,
) -> Option<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT project_id FROM project_conversations WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap();
    row.map(|(p,)| p)
}

async fn db_model_id_for_conversation(
    pool: &sqlx::PgPool,
    conversation_id: Uuid,
) -> Option<Uuid> {
    let row: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT model_id FROM conversations WHERE id = $1")
            .bind(conversation_id)
            .fetch_optional(pool)
            .await
            .unwrap();
    row.and_then(|(m,)| m)
}

async fn db_mcp_snapshot_approval_mode(
    pool: &sqlx::PgPool,
    conversation_id: Uuid,
) -> Option<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT approval_mode FROM mcp_settings WHERE conversation_id = $1",
    )
    .bind(conversation_id)
    .fetch_optional(pool)
    .await
    .unwrap();
    row.map(|(m,)| m)
}

// ============================================================
// List + cascade behavior
// ============================================================

#[tokio::test]
async fn list_project_conversations_returns_only_scoped() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p1 = helpers::create_project(&server, &user, "One").await;
    let p2 = helpers::create_project(&server, &user, "Two").await;
    let p1_id = p1["id"].as_str().unwrap();
    let p2_id = p2["id"].as_str().unwrap();

    let conv_p1 = helpers::create_project_conversation(&server, &user, p1_id).await;
    let _conv_p2 = helpers::create_project_conversation(&server, &user, p2_id).await;
    let _unfiled = helpers::create_unfiled_conversation(&server, &user).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}/conversations", p1_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let convs = body.as_array().expect("array");
    assert_eq!(convs.len(), 1, "exactly one conversation in P1");
    // The list endpoint joins project_conversations and returns
    // members of P1 only — assert membership via the conv id rather
    // than reading a project_id field (which no longer exists on
    // ConversationResponse).
    assert_eq!(convs[0]["id"], conv_p1);
}

#[tokio::test]
async fn chat_list_returns_all_user_conversations() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "P").await;
    let pid = p["id"].as_str().unwrap();
    let in_project = helpers::create_project_conversation(&server, &user, pid).await;
    let unfiled = helpers::create_unfiled_conversation(&server, &user).await;

    // Chat's `GET /conversations` is project-blind: it returns the
    // caller's conversations regardless of project membership.
    // Filtering by project is a project-module concern handled via
    // `GET /projects/{id}/conversations`.
    let resp = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let convs = body.as_array().expect("array");
    let ids: Vec<&str> = convs.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&unfiled.as_str()), "unfiled present: {:?}", ids);
    assert!(ids.contains(&in_project.as_str()), "project-bound present: {:?}", ids);
}

#[tokio::test]
async fn delete_project_unfiles_conversations_via_cascade() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let p = helpers::create_project(&server, &user, "To Delete").await;
    let pid = p["id"].as_str().unwrap();
    let conv_id_str = helpers::create_project_conversation(&server, &user, pid).await;
    let conv_uuid = Uuid::parse_str(&conv_id_str).unwrap();

    // Sanity: membership exists before delete.
    assert!(
        db_project_id_for_conversation(&pool, conv_uuid).await.is_some()
    );

    // Delete the project.
    assert_eq!(
        helpers::delete_project(&server, &user, pid).await,
        StatusCode::NO_CONTENT
    );

    // The conversation still exists.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}", conv_id_str)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Project-conversations row was cascaded away with the project
    // (migration 73 declares the FK as ON DELETE CASCADE on both
    // sides), so the conversation is now unfiled.
    assert!(
        db_project_id_for_conversation(&pool, conv_uuid).await.is_none(),
        "project_conversations membership should be dropped on project delete",
    );
}

#[tokio::test]
async fn default_assistant_deleted_sets_null() {
    // Migration 46: `default_assistant_id` is FK to assistants(id) with
    // ON DELETE SET NULL. Deleting the assistant should NOT delete the
    // project but should blank the column.
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let assistant_resp = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Pet Assistant" }))
        .send()
        .await
        .unwrap();
    assert_eq!(assistant_resp.status(), StatusCode::CREATED);
    let assistant: Value = assistant_resp.json().await.unwrap();
    let aid = assistant["id"].as_str().unwrap();

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Asst Holder", "default_assistant_id": aid }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();
    assert_eq!(project["default_assistant_id"], aid);

    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistants/{}", aid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success(), "assistant delete failed: {}", del.status());

    let (_status, body) = helpers::get_project(&server, &user, pid).await;
    let project_after = body.expect("project still exists");
    assert!(
        project_after["default_assistant_id"].is_null(),
        "expected SET NULL on FK cascade, got: {:?}",
        project_after["default_assistant_id"]
    );
}

// ============================================================
// Attach endpoint — success paths
// ============================================================

#[tokio::test]
async fn attach_unfiled_conversation_sets_project_id_and_snapshots_mcp() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();
    set_distinct_mcp_defaults(&server, &user, pid).await;

    let conv_id_str = helpers::create_unfiled_conversation(&server, &user).await;
    let conv_id = Uuid::parse_str(&conv_id_str).unwrap();
    assert!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await.is_none(),
        "expected no MCP snapshot before attach"
    );

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "attach: {}",
        resp.text().await.unwrap_or_default()
    );

    // Response body asserts the post-attach ConversationResponse
    // shape — message_count is the real value (fresh conv, so 0).
    // (No project_id on the wire: membership lives in the
    // project_conversations join table; assert below via DB.)
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["id"], conv_id_str);
    assert_eq!(body["message_count"], 0);

    // Membership row exists in project_conversations.
    let project_uuid = Uuid::parse_str(pid).unwrap();
    assert_eq!(
        db_project_id_for_conversation(&pool, conv_id).await,
        Some(project_uuid),
    );
    // MCP snapshot was written with the project's distinct defaults.
    assert_eq!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await,
        Some("auto_approve".to_string()),
    );
}

#[tokio::test]
async fn attach_is_idempotent_and_refreshes_snapshot_from_current_project_defaults() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();
    let conv_id_str = helpers::create_unfiled_conversation(&server, &user).await;
    let conv_id = Uuid::parse_str(&conv_id_str).unwrap();

    // First attach: snapshot picks up `manual_approve` from defaults.
    let first = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await,
        Some("manual_approve".to_string()),
    );

    // Mutate the project's MCP defaults.
    set_distinct_mcp_defaults(&server, &user, pid).await;

    // Re-POST: idempotency returns 200 + REFRESHES the snapshot from
    // the project's CURRENT defaults (proves DO UPDATE path).
    let second = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await,
        Some("auto_approve".to_string()),
    );
}

#[tokio::test]
async fn cross_project_reattach_overwrites_snapshot_with_destination_defaults() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    // Project A keeps manual_approve (default); B set to auto_approve.
    let project_a = helpers::create_project(&server, &user, "A").await;
    let project_b = helpers::create_project(&server, &user, "B").await;
    let pid_a = project_a["id"].as_str().unwrap();
    let pid_b = project_b["id"].as_str().unwrap();
    set_distinct_mcp_defaults(&server, &user, pid_b).await;

    let conv_id_str = helpers::create_unfiled_conversation(&server, &user).await;
    let conv_id = Uuid::parse_str(&conv_id_str).unwrap();

    // Attach to A.
    let into_a = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid_a, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(into_a.status(), StatusCode::OK);
    assert_eq!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await,
        Some("manual_approve".to_string()),
    );

    // Re-attach to B — snapshot rewrites to B's defaults.
    let into_b = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid_b, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        into_b.status(),
        StatusCode::OK,
        "cross-project re-attach should succeed: {}",
        into_b.text().await.unwrap_or_default()
    );

    let project_b_uuid = Uuid::parse_str(pid_b).unwrap();
    assert_eq!(
        db_project_id_for_conversation(&pool, conv_id).await,
        Some(project_b_uuid),
    );
    assert_eq!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await,
        Some("auto_approve".to_string()),
    );
}

#[tokio::test]
async fn attach_preserves_explicit_conversation_model_id() {
    // T2-2 from audit: replacement for the deleted
    // `explicit_model_id_overrides_project_default_on_create` test.
    // The attach endpoint must NOT clobber `conversations.model_id`
    // with the project's default_model_id (the conversation's
    // explicit choice wins, period).
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let configs = crate::chat::helpers::get_test_model_configs();
    let mut models = Vec::new();
    for cfg in &configs {
        let m = crate::chat::helpers::create_test_model_with_config(
            &server,
            cfg,
            Some(&user.user_id),
        )
        .await;
        if !m.is_null() {
            models.push(m);
            if models.len() == 2 {
                break;
            }
        }
    }
    if models.len() < 2 {
        eprintln!(
            "Skipping attach_preserves_explicit_conversation_model_id — need 2 distinct provider keys"
        );
        return;
    }
    let project_default = models[0]["id"].as_str().unwrap();
    let explicit_model = models[1]["id"].as_str().unwrap();
    let explicit_model_uuid = Uuid::parse_str(explicit_model).unwrap();

    // Project pins `project_default` as its default_model_id.
    let project = helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Default Holder", "default_model_id": project_default }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    // Create unfiled conversation with the OTHER model explicitly.
    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": explicit_model }))
        .send()
        .await
        .unwrap();
    assert_eq!(conv_resp.status(), StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    let conv_id_str = conv["id"].as_str().unwrap();
    let conv_id = Uuid::parse_str(conv_id_str).unwrap();

    // Sanity: conversation has explicit model BEFORE attach.
    assert_eq!(
        db_model_id_for_conversation(&pool, conv_id).await,
        Some(explicit_model_uuid),
    );

    // Attach to project.
    let attach = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), StatusCode::OK);

    // The CONTRACT: model_id stays as the explicit user choice;
    // attach does NOT snapshot project.default_model_id.
    assert_eq!(
        db_model_id_for_conversation(&pool, conv_id).await,
        Some(explicit_model_uuid),
        "attach must NOT overwrite conversation.model_id with project default"
    );
}

// ============================================================
// Attach endpoint — failure paths
// ============================================================

#[tokio::test]
async fn attach_returns_404_when_project_does_not_exist() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let conv_id = helpers::create_unfiled_conversation(&server, &user).await;
    let bogus_project = Uuid::new_v4();

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            bogus_project, conv_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn attach_returns_404_when_project_owned_by_other_user() {
    let server = TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let conv_id = helpers::create_unfiled_conversation(&server, &alice).await;
    let bobs_project = helpers::create_project(&server, &bob, "Bob's").await;
    let bobs_pid = bobs_project["id"].as_str().unwrap();

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            bobs_pid, conv_id
        )))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn attach_returns_404_when_conversation_owned_by_other_user() {
    let server = TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let alices_project = helpers::create_project(&server, &alice, "Alice's").await;
    let pid = alices_project["id"].as_str().unwrap();
    let bobs_conv = helpers::create_unfiled_conversation(&server, &bob).await;

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, bobs_conv
        )))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================================================
// Detach endpoint
// ============================================================

#[tokio::test]
async fn detach_clears_project_id_and_deletes_mcp_snapshot() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();
    let conv_id_str = helpers::create_unfiled_conversation(&server, &user).await;
    let conv_id = Uuid::parse_str(&conv_id_str).unwrap();

    // Setup: attach first so we have something to undo.
    let attach = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(attach.status(), StatusCode::OK);
    assert!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await.is_some(),
        "precondition: snapshot exists after attach"
    );

    // Detach.
    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id_str
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "detach: {}",
        resp.text().await.unwrap_or_default()
    );

    assert_eq!(
        db_project_id_for_conversation(&pool, conv_id).await,
        None,
        "conversation.project_id should be cleared"
    );
    assert!(
        db_mcp_snapshot_approval_mode(&pool, conv_id).await.is_none(),
        "MCP snapshot row should be deleted on detach"
    );
}

#[tokio::test]
async fn detach_then_reattach_round_trip() {
    // T2-9 from audit: cover detach → re-attach to the same project
    // doesn't leave any stale state.
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();
    let project_uuid = Uuid::parse_str(pid).unwrap();
    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;
    let conv_uuid = Uuid::parse_str(&conv_id).unwrap();

    // Detach.
    let detach = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(detach.status(), StatusCode::NO_CONTENT);
    assert!(db_mcp_snapshot_approval_mode(&pool, conv_uuid).await.is_none());

    // Re-attach to the same project. Snapshot should be created
    // fresh (was deleted), project_id restored.
    let reattach = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, conv_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(reattach.status(), StatusCode::OK);
    assert_eq!(
        db_project_id_for_conversation(&pool, conv_uuid).await,
        Some(project_uuid),
    );
    assert!(
        db_mcp_snapshot_approval_mode(&pool, conv_uuid).await.is_some(),
        "snapshot should be re-created on re-attach after detach",
    );
}

#[tokio::test]
async fn detach_returns_404_when_conversation_belongs_to_a_different_project() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let project_a = helpers::create_project(&server, &user, "A").await;
    let project_b = helpers::create_project(&server, &user, "B").await;
    let pid_a = project_a["id"].as_str().unwrap();
    let pid_b = project_b["id"].as_str().unwrap();
    let conv_id = helpers::create_unfiled_conversation(&server, &user).await;

    // Attach conv to A.
    let _ = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid_a, conv_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    // Detach via B's URL — should 404 (mis-addressed).
    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid_b, conv_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn detach_returns_404_when_conversation_is_already_unfiled() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();
    let unfiled = helpers::create_unfiled_conversation(&server, &user).await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            pid, unfiled
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn detach_returns_404_when_project_owned_by_other_user() {
    let server = TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let bobs_project = helpers::create_project(&server, &bob, "Bob's").await;
    let bobs_pid = bobs_project["id"].as_str().unwrap();
    let alices_conv = helpers::create_unfiled_conversation(&server, &alice).await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            bobs_pid, alices_conv
        )))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ============================================================
// project_for_conversation — GET /projects/by-conversation/{id}
// ============================================================
//
// Used by the frontend project chat extension to resolve a
// conversation's parent project (trailing chip on ConversationCard,
// "Open: NAME" entry in 3-dot menu). Always 200 — returns Project
// for attached conversations, `null` for unfiled / nonexistent / not
// owned. Treating "unfiled" as a 404 used to spam the client console
// with error logs on every chat surface load.

#[tokio::test]
async fn project_for_conversation_returns_project_when_attached() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let project = helpers::create_project(&server, &user, "Parent").await;
    let pid = project["id"].as_str().unwrap();
    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/by-conversation/{}", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(!body.is_null(), "expected Project body, got null");
    assert_eq!(body["id"], pid);
    assert_eq!(body["name"], "Parent");
}

#[tokio::test]
async fn project_for_conversation_returns_null_when_unfiled() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let unfiled = helpers::create_unfiled_conversation(&server, &user).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/by-conversation/{}", unfiled)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_null(), "expected null for unfiled conversation");
}

#[tokio::test]
async fn project_for_conversation_returns_null_when_conversation_owned_by_other_user() {
    // Cross-user ownership: alice queries by Bob's project-bound conv.
    // The handler resolves via project ownership scope, so alice
    // doesn't get a leak even though the row physically exists.
    let server = TestServer::start().await;
    let alice = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let bob = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let bobs_project = helpers::create_project(&server, &bob, "Bob's").await;
    let bobs_pid = bobs_project["id"].as_str().unwrap();
    let bobs_conv = helpers::create_project_conversation(&server, &bob, bobs_pid).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/by-conversation/{}", bobs_conv)))
        .header("Authorization", format!("Bearer {}", alice.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_null(), "ownership leak — alice should see null for Bob's conv");
}

#[tokio::test]
async fn project_for_conversation_returns_null_for_nonexistent_conversation() {
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let bogus = Uuid::new_v4();
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/by-conversation/{}", bogus)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(body.is_null(), "expected null for nonexistent conversation id");
}

// ============================================================
// FK cascade: deleting the conversation drops its
// project_conversations row (the reverse of
// `delete_project_unfiles_conversations_via_cascade`).
// ============================================================

#[tokio::test]
async fn delete_conversation_cascades_project_conversations_row() {
    // Migration 73 declares `conversation_id REFERENCES conversations(id)
    // ON DELETE CASCADE`. Deleting the conversation must drop its
    // membership row, leaving the project intact but with one fewer
    // attached conversation.
    let server = TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;
    let pool = pool(&server).await;

    let project = helpers::create_project(&server, &user, "Holder").await;
    let pid = project["id"].as_str().unwrap();
    let conv_id_str = helpers::create_project_conversation(&server, &user, pid).await;
    let conv_uuid = Uuid::parse_str(&conv_id_str).unwrap();

    // Sanity: membership row exists before delete.
    assert!(
        db_project_id_for_conversation(&pool, conv_uuid).await.is_some(),
        "membership precondition"
    );

    // Delete the conversation through chat's normal endpoint.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/conversations/{}", conv_id_str)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(
        del.status().is_success(),
        "conversation delete failed: {}",
        del.status()
    );

    // Project still exists.
    let (status, _) = helpers::get_project(&server, &user, pid).await;
    assert_eq!(status, StatusCode::OK, "project should survive");

    // Membership row cascaded away with the conversation.
    assert!(
        db_project_id_for_conversation(&pool, conv_uuid).await.is_none(),
        "project_conversations row should cascade-delete with conversation",
    );
}

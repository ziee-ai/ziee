//! Conversation ↔ project relation: assignment, listing, move,
//! deletion behavior (SET NULL).

use reqwest::StatusCode;
use serde_json::{Value, json};

use super::helpers;

#[tokio::test]
async fn create_conversation_in_project_sets_project_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let project = helpers::create_project(&server, &user, "P").await;
    let pid = project["id"].as_str().unwrap();

    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "project_id": pid }))
        .send()
        .await
        .unwrap();
    assert_eq!(conv_resp.status(), StatusCode::CREATED);
    let body: Value = conv_resp.json().await.unwrap();
    assert_eq!(body["project_id"], pid);
}

#[tokio::test]
async fn list_project_conversations_returns_only_scoped() {
    let server = crate::common::TestServer::start().await;
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

    let _conv_p1 = helpers::create_project_conversation(&server, &user, p1_id).await;
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
    assert_eq!(convs[0]["project_id"], p1_id);
}

#[tokio::test]
async fn list_unfiled_conversations_only() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "P").await;
    let pid = p["id"].as_str().unwrap();
    let _in_project = helpers::create_project_conversation(&server, &user, pid).await;
    let unfiled = helpers::create_unfiled_conversation(&server, &user).await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/conversations?project_id=null"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let convs = body.as_array().expect("array");
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0]["id"], unfiled);
}

#[tokio::test]
async fn move_conversation_into_and_out_of_project() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "Holder").await;
    let pid = p["id"].as_str().unwrap();
    let conv_id = helpers::create_unfiled_conversation(&server, &user).await;

    // Move into project.
    let into = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "project_id": pid }))
        .send()
        .await
        .unwrap();
    assert_eq!(into.status(), StatusCode::OK);
    let after_into: Value = into.json().await.unwrap();
    assert_eq!(after_into["project_id"], pid);

    // Move OUT (explicit null).
    let out = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "project_id": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(out.status(), StatusCode::OK);
    let after_out: Value = out.json().await.unwrap();
    assert!(after_out["project_id"].is_null(), "expected NULL");
}

#[tokio::test]
async fn cannot_move_to_other_users_project() {
    let server = crate::common::TestServer::start().await;
    let user_a = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "alice",
        helpers::full_project_permissions(),
    )
    .await;
    let user_b = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "bob",
        helpers::full_project_permissions(),
    )
    .await;

    let p_b = helpers::create_project(&server, &user_b, "Bob's").await;
    let pid_b = p_b["id"].as_str().unwrap();
    let conv_a = helpers::create_unfiled_conversation(&server, &user_a).await;

    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{}", conv_a)))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .json(&json!({ "project_id": pid_b }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn delete_project_preserves_conversations_with_null_project_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "To Delete").await;
    let pid = p["id"].as_str().unwrap();
    let conv_id = helpers::create_project_conversation(&server, &user, pid).await;

    // Delete the project.
    assert_eq!(
        helpers::delete_project(&server, &user, pid).await,
        StatusCode::NO_CONTENT
    );

    // The conversation still exists, project_id NULL'd.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{}", conv_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert!(body["project_id"].is_null());
}

#[tokio::test]
async fn default_model_snapshots_into_conversation_on_create() {
    // When a conversation is created inside a project AND no explicit
    // model_id is sent, the server must snapshot `project.default_model_id`
    // into `conversations.model_id`. Plan 5 §4 precedence table:
    //   request.model_id → conversation.model_id (snapshot) → project.default_model_id
    //
    // Uses the chat helpers to provision a real LLM model; if no
    // provider API key is set the helper returns null and we skip
    // (mirrors the chat helpers' contract).
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let configs = crate::chat::helpers::get_test_model_configs();
    let mut model: Value = Value::Null;
    for cfg in &configs {
        let m = crate::chat::helpers::create_test_model_with_config(
            &server,
            cfg,
            Some(&user.user_id),
        )
        .await;
        if !m.is_null() {
            model = m;
            break;
        }
    }
    if model.is_null() {
        eprintln!(
            "Skipping default_model_snapshots_into_conversation_on_create — no provider API key set"
        );
        return;
    }
    let model_id = model["id"].as_str().unwrap();

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Model Holder", "default_model_id": model_id }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    // Create conversation in the project, no explicit model_id.
    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "project_id": pid }))
        .send()
        .await
        .unwrap();
    assert_eq!(conv_resp.status(), StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    assert_eq!(
        conv["model_id"], model_id,
        "conversation.model_id should be snapshotted from project.default_model_id"
    );
    assert_eq!(conv["project_id"], pid);
}

#[tokio::test]
async fn explicit_model_id_overrides_project_default_on_create() {
    // Inverse: when the create request DOES carry model_id, the
    // project default is ignored (request.model_id wins per the
    // precedence table).
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

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
            "Skipping explicit_model_id_overrides_project_default_on_create — need 2 distinct provider keys"
        );
        return;
    }
    let default_model_id = models[0]["id"].as_str().unwrap();
    let explicit_model_id = models[1]["id"].as_str().unwrap();

    let project = helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Override Test", "default_model_id": default_model_id }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();

    let conv_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "project_id": pid, "model_id": explicit_model_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(conv_resp.status(), StatusCode::CREATED);
    let conv: Value = conv_resp.json().await.unwrap();
    assert_eq!(
        conv["model_id"], explicit_model_id,
        "explicit model_id must win over project.default_model_id"
    );
}

#[tokio::test]
async fn default_assistant_deleted_sets_null() {
    // Migration 46: `default_assistant_id` is FK to assistants(id) with
    // ON DELETE SET NULL. Deleting the assistant should NOT delete the
    // project but should blank the column.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    // Create an assistant.
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

    // Create a project that pins this assistant as the default.
    let project = helpers::create_project_with(
        &server,
        &user,
        json!({ "name": "Asst Holder", "default_assistant_id": aid }),
    )
    .await;
    let pid = project["id"].as_str().unwrap();
    assert_eq!(project["default_assistant_id"], aid);

    // Delete the assistant.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistants/{}", aid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success(), "assistant delete failed: {}", del.status());

    // Re-fetch the project — default_assistant_id should now be NULL.
    let (_status, body) = helpers::get_project(&server, &user, pid).await;
    let project_after = body.expect("project still exists");
    assert!(
        project_after["default_assistant_id"].is_null(),
        "expected SET NULL on FK cascade, got: {:?}",
        project_after["default_assistant_id"]
    );
}

#[tokio::test]
async fn project_id_filter_with_uuid_scopes_to_that_project() {
    // `?project_id=<uuid>` is honored alongside `?project_id=null`.
    // Returns conversations scoped to the named project only.
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let p = helpers::create_project(&server, &user, "P").await;
    let pid = p["id"].as_str().unwrap();
    let in_p = helpers::create_project_conversation(&server, &user, pid).await;
    let _other = helpers::create_unfiled_conversation(&server, &user).await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations?project_id={}", pid)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let convs = body.as_array().expect("array");
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0]["id"], in_p);
}

#[tokio::test]
async fn project_id_filter_with_invalid_uuid_returns_400() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        helpers::full_project_permissions(),
    )
    .await;

    let resp = reqwest::Client::new()
        .get(server.api_url("/conversations?project_id=not-a-uuid"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

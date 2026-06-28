// Integration tests for Assistant module

mod sync_emit_test;

use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_assistants_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let response = reqwest::Client::new()
        .get(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_user_assistant_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "user").await;

    let payload = json!({
        "name": "My Assistant"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_template_requires_template_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create"],
    )
    .await;

    let payload = json!({
        "name": "Template Assistant"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_list_templates_requires_template_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .get(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// User Assistant CRUD Tests
// =====================================================

#[tokio::test]
async fn test_create_user_assistant_success() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create"],
    )
    .await;

    let payload = json!({
        "name": "My Assistant",
        "description": "My personal assistant",
        "instructions": "Be helpful and concise"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "My Assistant");
    assert_eq!(body["description"], "My personal assistant");
    assert_eq!(body["is_template"], false);
    assert_eq!(body["is_default"], false);
    assert_eq!(body["enabled"], true);
    assert!(body["created_by"].is_string());
}

#[tokio::test]
async fn test_list_user_assistants() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create", "assistants::read"],
    )
    .await;

    // Create two assistants
    create_user_assistant(&server, &user.token, "Assistant 1").await;
    create_user_assistant(&server, &user.token, "Assistant 2").await;

    // List user assistants
    let response = reqwest::Client::new()
        .get(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["assistants"].is_array());
    assert!(body["assistants"].as_array().unwrap().len() >= 2);
    assert!(body["total"].as_i64().unwrap() >= 2);
}

#[tokio::test]
async fn test_get_user_assistant_by_id() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create", "assistants::read"],
    )
    .await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "Test Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Get by ID
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], assistant["id"]);
    assert_eq!(body["name"], "Test Assistant");
}

#[tokio::test]
async fn test_update_user_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create", "assistants::edit", "assistants::read"],
    )
    .await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "Original Name").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Update
    let payload = json!({
        "name": "Updated Name",
        "description": "New description"
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Updated Name");
    assert_eq!(body["description"], "New description");
}

#[tokio::test]
async fn test_delete_user_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &[
            "assistants::create",
            "assistants::delete",
            "assistants::read",
        ],
    )
    .await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "To Delete").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Delete
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deleted
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Template Assistant CRUD Tests
// =====================================================

#[tokio::test]
async fn test_create_template_assistant_success() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistant_templates::create"],
    )
    .await;

    let payload = json!({
        "name": "Template Assistant",
        "description": "A template for all users"
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Template Assistant");
    assert_eq!(body["is_template"], true);
    assert!(body["created_by"].is_null());
}

#[tokio::test]
async fn test_list_template_assistants() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["assistant_templates::create", "assistant_templates::read"],
    )
    .await;

    // Create templates
    create_template_assistant(&server, &admin.token, "Template 1").await;
    create_template_assistant(&server, &admin.token, "Template 2").await;

    // List templates
    let response = reqwest::Client::new()
        .get(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["assistants"].is_array());
    assert!(body["assistants"].as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn test_get_template_assistant_by_id() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["assistant_templates::create", "assistant_templates::read"],
    )
    .await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "Template").await;
    let template_id = template["id"].as_str().unwrap();

    // Get by ID
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/assistant-templates/{}", template_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], template["id"]);
    assert_eq!(body["is_template"], true);
}

#[tokio::test]
async fn test_update_template_assistant() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["assistant_templates::create", "assistant_templates::edit"],
    )
    .await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "Original Template").await;
    let template_id = template["id"].as_str().unwrap();

    // Update
    let payload = json!({
        "name": "Updated Template"
    });

    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/assistant-templates/{}", template_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["name"], "Updated Template");
}

#[tokio::test]
async fn test_delete_template_assistant() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["assistant_templates::create", "assistant_templates::delete"],
    )
    .await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "To Delete").await;
    let template_id = template["id"].as_str().unwrap();

    // Delete
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistant-templates/{}", template_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

// =====================================================
// Ownership Tests
// =====================================================

#[tokio::test]
async fn test_user_cannot_read_other_users_assistant() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["assistants::create", "assistants::read"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["assistants::read"],
    )
    .await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to read it
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_user_cannot_edit_other_users_assistant() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["assistants::create"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["assistants::edit"],
    )
    .await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to edit it
    let payload = json!({"name": "Hacked"});
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_user_cannot_delete_other_users_assistant() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user1",
        &["assistants::create"],
    )
    .await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user2",
        &["assistants::delete"],
    )
    .await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to delete it
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

// =====================================================
// Default Assistant Tests
// =====================================================

#[tokio::test]
async fn test_create_default_user_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create", "assistants::read"],
    )
    .await;

    let payload = json!({
        "name": "My Default",
        "is_default": true
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["is_default"], true);

    // Get default
    let response = reqwest::Client::new()
        .get(server.api_url("/assistants/default"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let default_assistant: serde_json::Value = response.json().await.unwrap();
    assert_eq!(default_assistant["id"], body["id"]);
}

#[tokio::test]
async fn test_only_one_default_per_user() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create", "assistants::read"],
    )
    .await;

    // Create first default
    let payload1 = json!({
        "name": "Default 1",
        "is_default": true
    });

    let response1 = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload1)
        .send()
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::CREATED);
    let assistant1: serde_json::Value = response1.json().await.unwrap();

    // Create second default
    let payload2 = json!({
        "name": "Default 2",
        "is_default": true
    });

    let response2 = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload2)
        .send()
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::CREATED);
    let assistant2: serde_json::Value = response2.json().await.unwrap();

    // Verify assistant1 is no longer default
    let response = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/assistants/{}",
            assistant1["id"].as_str().unwrap()
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["is_default"], false);

    // Verify assistant2 is default
    assert_eq!(assistant2["is_default"], true);
}

// =====================================================
// Validation Tests
// =====================================================

#[tokio::test]
async fn test_create_assistant_empty_name() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::create"],
    )
    .await;

    let payload = json!({
        "name": ""
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_assistant_not_found() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["assistants::read"],
    )
    .await;

    let assistant_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Event System Tests
// =====================================================

#[tokio::test]
async fn test_default_template_cloned_on_user_registration() {
    let server = crate::common::TestServer::start().await;

    // Create an admin user to create a default template
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "admin_template",
        &[
            "assistant_templates::create",
            "assistant_templates::set_default",
        ],
    )
    .await;

    // Create a default enabled template
    let template_payload = json!({
        "name": "Default Test Template",
        "description": "A template that should be cloned to new users",
        "instructions": "You are a helpful assistant",
        "is_default": true
    });

    let create_template_response = reqwest::Client::new()
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&template_payload)
        .send()
        .await
        .unwrap();

    assert_eq!(create_template_response.status(), StatusCode::CREATED);
    let template: serde_json::Value = create_template_response.json().await.unwrap();
    assert_eq!(template["name"], "Default Test Template");
    assert_eq!(template["is_default"], true);
    assert_eq!(template["enabled"], true);

    // Create a new user with read assistants permission (this should trigger the UserCreated event)
    let new_user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "newuser_event",
        &["assistants::read"],
    )
    .await;

    // Wait a moment for the async event to process
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    // List the new user's assistants to verify the template was cloned
    let list_response = reqwest::Client::new()
        .get(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", new_user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_result: serde_json::Value = list_response.json().await.unwrap();

    // Verify the cloned assistant exists
    let assistants = list_result["assistants"].as_array().unwrap();
    assert!(
        !assistants.is_empty(),
        "New user should have at least one assistant (cloned from template). Found {} assistants",
        assistants.len()
    );

    // Find the cloned assistant
    let cloned_assistant = assistants
        .iter()
        .find(|a| a["name"] == "Default Test Template")
        .expect("Cloned template assistant should exist");

    assert_eq!(cloned_assistant["name"], "Default Test Template");
    assert_eq!(
        cloned_assistant["description"],
        "A template that should be cloned to new users"
    );
    assert_eq!(cloned_assistant["is_template"], false); // It's a user assistant, not a template
    // 10-assistant F-04 (Medium): template-clone-on-signup now forces
    // is_default=false instead of copying the template's flag. New
    // users start with no default assistant; they pick one explicitly
    // post-signup. Previously a `is_default=true` template would mint
    // a forced-default per user with no opt-out.
    assert_eq!(cloned_assistant["is_default"], false);
    assert_eq!(cloned_assistant["enabled"], true);
}

// =====================================================
// Helper Functions
// =====================================================

async fn create_user_assistant(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> serde_json::Value {
    let payload = json!({
        "name": name
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

// The template endpoints must refuse to operate on a non-template
// (user-owned) assistant: update_template / delete_template both check
// `!existing.is_template` and return 404 so a user assistant can't be
// mutated or deleted through the template surface (handlers.rs:498-501,
// 544-546). A 404 (not 403) is intentional — the row simply isn't a
// template from the template endpoint's point of view.
#[tokio::test]
async fn test_update_template_rejects_non_template_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tmpl_guard_update",
        &["assistants::create", "assistant_templates::edit"],
    )
    .await;

    // A normal user assistant (is_template = false).
    let assistant = create_user_assistant(&server, &user.token, "Not A Template").await;
    let id = assistant["id"].as_str().unwrap();
    assert_eq!(assistant["is_template"], false);

    // Reaching the template-edit handler (perm passes), but the
    // is_template guard rejects it with 404.
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/assistant-templates/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Hijacked" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_template_rejects_non_template_assistant() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tmpl_guard_delete",
        &[
            "assistants::create",
            "assistants::read",
            "assistant_templates::delete",
        ],
    )
    .await;

    let assistant = create_user_assistant(&server, &user.token, "Keep Me").await;
    let id = assistant["id"].as_str().unwrap();
    assert_eq!(assistant["is_template"], false);

    // Template-delete handler is reached (perm passes), but the
    // is_template guard rejects with 404 — the user assistant survives.
    let response = reqwest::Client::new()
        .delete(server.api_url(&format!("/assistant-templates/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Confirm it was NOT deleted — still readable on the user surface.
    let get = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
}

async fn create_template_assistant(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
) -> serde_json::Value {
    let payload = json!({
        "name": name
    });

    let response = reqwest::Client::new()
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

/// `is_template` is deliberately omitted from `UpdateAssistantRequest`, so it is
/// IMMUTABLE: a client that injects `is_template: true` into the update body
/// must NOT be able to promote a user assistant into a template. The field is
/// dropped at deserialization and the persisted value stays `false`.
#[tokio::test]
async fn test_is_template_is_immutable_on_update() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tmpl_immut",
        &["assistants::create", "assistants::edit", "assistants::read"],
    )
    .await;

    let assistant = create_user_assistant(&server, &user.token, "Plain Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();
    assert_eq!(assistant["is_template"], false, "precondition: a user assistant");

    // Attempt to flip is_template via the update body (and is_default for good measure).
    let payload = json!({
        "name": "Still Not A Template",
        "is_template": true,
        "is_default": true,
    });
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["is_template"], false,
        "is_template must remain false — the injected field is ignored"
    );

    // Confirm via a fresh GET that the persisted row wasn't promoted either.
    let got: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(got["is_template"], false, "GET must also show is_template still false");
}

// =====================================================
// message_assistant attribution persistence (migration 75)
// =====================================================

/// The assistant extension's `after_user_message_created` hook records which
/// assistant was active into the `message_assistant` join table, and
/// `GET /api/messages/{id}/assistant` reads it back. This test seeds a
/// user-owned conversation/branch/message + a message_assistant row directly,
/// then asserts the read endpoint returns the attributed assistant — and that
/// a message with NO attribution returns `assistant_id: null`.
#[tokio::test]
async fn test_message_assistant_attribution_persists_and_reads_back() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "msg_assistant",
        &[
            "assistants::create",
            "assistants::read",
            "conversations::read",
        ],
    )
    .await;

    // A real assistant row (message_assistant.assistant_id FKs to assistants).
    let assistant = create_user_assistant(&server, &user.token, "Attributed Assistant").await;
    let assistant_id = Uuid::parse_str(assistant["id"].as_str().unwrap()).unwrap();

    // Seed a user-owned conversation → branch → two messages.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = Uuid::parse_str(&user.user_id).unwrap();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    let attributed_msg = Uuid::new_v4();
    let bare_msg = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'ma', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE conversations SET active_branch_id = $1 WHERE id = $2")
        .bind(branch_id)
        .bind(conv_id)
        .execute(&pool)
        .await
        .unwrap();
    for msg_id in [attributed_msg, bare_msg] {
        sqlx::query(
            r#"INSERT INTO messages (id, role, originated_from_id, created_at)
               VALUES ($1, 'user', $1, NOW())"#,
        )
        .bind(msg_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO branch_messages (branch_id, message_id, created_at)
               VALUES ($1, $2, NOW())"#,
        )
        .bind(branch_id)
        .bind(msg_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    // Attribute exactly one message.
    sqlx::query(
        r#"INSERT INTO message_assistant (message_id, assistant_id) VALUES ($1, $2)"#,
    )
    .bind(attributed_msg)
    .bind(assistant_id)
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let get_attr = |msg: Uuid| {
        let url = server.api_url(&format!("/messages/{msg}/assistant"));
        let token = user.token.clone();
        async move {
            reqwest::Client::new()
                .get(url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .unwrap()
        }
    };

    // Attributed message → returns the assistant id.
    let res = get_attr(attributed_msg).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["assistant_id"].as_str().unwrap(),
        assistant_id.to_string(),
        "attributed message must read back its assistant: {body}"
    );

    // Message with no attribution → owned, but assistant_id is null.
    let res = get_attr(bare_msg).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["assistant_id"].is_null(), "un-attributed message → null: {body}");

    // A message the user doesn't own (random id) → 404 (ownership conflation).
    let res = get_attr(Uuid::new_v4()).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Template cloning on user creation (event_handlers.rs)
// =====================================================

/// CloneTemplateAssistantsHandler (fired async on UserEvent::Created) clones
/// ONLY templates where `is_default && enabled` to each new user. A default+
/// enabled template is cloned (as a non-template, non-default user assistant);
/// a non-default template is NOT. We create both as admin BEFORE registering a
/// fresh user, then poll that user's assistant list.
#[tokio::test]
async fn test_user_creation_clones_only_default_enabled_templates() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tpl_clone_admin",
        &["assistant_templates::create"],
    )
    .await;

    let tag = &Uuid::new_v4().to_string()[..8];
    let default_name = format!("CloneDefault-{tag}");
    let nondefault_name = format!("CloneSkip-{tag}");
    let client = reqwest::Client::new();
    let mk_template = |name: String, is_default: bool| {
        let client = client.clone();
        let url = server.api_url("/assistant-templates");
        let token = admin.token.clone();
        async move {
            let r = client
                .post(url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "name": name, "is_default": is_default, "enabled": true }))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::CREATED, "create template {name:?}");
        }
    };
    mk_template(default_name.clone(), true).await;
    mk_template(nondefault_name.clone(), false).await;

    // Register a fresh user AFTER the templates exist → the async clone handler
    // runs for this user.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tpl_clone_target",
        &["assistants::read"],
    )
    .await;

    // Poll the new user's assistant list until the cloned default appears.
    let list_names = || {
        let client = client.clone();
        let url = server.api_url("/assistants");
        let token = user.token.clone();
        async move {
            let body: serde_json::Value = client
                .get(url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            body["assistants"]
                .as_array()
                .map(|a| a.iter().filter_map(|x| x["name"].as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                .unwrap_or_default()
        }
    };

    let mut names: Vec<String> = Vec::new();
    for _ in 0..40 {
        names = list_names().await;
        if names.iter().any(|n| n == &default_name) {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    }
    assert!(
        names.iter().any(|n| n == &default_name),
        "the default+enabled template must be cloned to the new user; got {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == &nondefault_name),
        "the non-default template must NOT be cloned; got {names:?}"
    );
}

// =====================================================
// Message-assistant attribution: persistence + ON CONFLICT idempotence
// (migration 75; chat_extension/repository.rs::insert_message_assistant)
// =====================================================

/// A message sent with a selected assistant records a durable
/// `message_assistant` row (readable via GET /messages/{id}/assistant), and the
/// repository's `ON CONFLICT (message_id) DO NOTHING` makes the attribution
/// immutable — a later duplicate insert for the same message can't overwrite
/// the original (the snapshot survives re-sends / restarts since it lives in
/// Postgres, not memory).
#[tokio::test]
async fn message_assistant_attribution_persists_and_is_idempotent() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "msg_attr_user",
        &[
            "conversations::create",
            "conversations::read",
            "messages::create",
            "messages::read",
            "llm_models::read",
            "assistants::create",
            "assistants::read",
        ],
    )
    .await;

    // Stub chat model (no real LLM) + two assistants.
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = crate::chat::helpers::parse_uuid(&model["id"]);

    let mk_assistant = |name: &'static str| {
        let token = user.token.clone();
        let api = server.api_url("/assistants");
        async move {
            let resp = reqwest::Client::new()
                .post(api)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "name": name, "description": "attr", "instructions": "be brief" }))
                .send()
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
            let body: serde_json::Value = resp.json().await.unwrap();
            Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
        }
    };
    let assistant_id = mk_assistant("Primary Assistant").await;
    let other_assistant_id = mk_assistant("Other Assistant").await;

    // Conversation + branch.
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_id), Some("attr"))
            .await;
    let conv_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    // Send a message WITH the assistant selected → the assistant chat extension
    // snapshots the attribution onto the user message.
    let send = reqwest::Client::new()
        .post(server.api_url(&format!("/conversations/{conv_id}/messages")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "content": "hi",
            "model_id": model_id.to_string(),
            "branch_id": branch_id.to_string(),
            "assistant_id": assistant_id.to_string(),
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(send.status(), StatusCode::OK, "send should 200");
    let send_body: serde_json::Value = send.json().await.unwrap();
    let user_message_id =
        Uuid::parse_str(send_body["user_message_id"].as_str().unwrap()).unwrap();

    // Poll the attribution endpoint until the (stream-time) snapshot lands.
    let client = reqwest::Client::new();
    let mut got: Option<Uuid> = None;
    for _ in 0..40 {
        let resp = client
            .get(server.api_url(&format!("/messages/{user_message_id}/assistant")))
            .header("Authorization", format!("Bearer {}", user.token))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value = resp.json().await.unwrap();
        if let Some(id) = body["assistant_id"].as_str() {
            got = Some(Uuid::parse_str(id).unwrap());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    assert_eq!(
        got,
        Some(assistant_id),
        "the persisted attribution must be the assistant the message was sent with"
    );

    // ON CONFLICT DO NOTHING: a duplicate insert for the SAME message_id (with a
    // DIFFERENT assistant) must NOT overwrite the original snapshot.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query("INSERT INTO message_assistant (message_id, assistant_id) VALUES ($1, $2) ON CONFLICT (message_id) DO NOTHING")
        .bind(user_message_id)
        .bind(other_assistant_id)
        .execute(&pool)
        .await
        .expect("duplicate insert must not error (ON CONFLICT)");
    pool.close().await;

    let after = client
        .get(server.api_url(&format!("/messages/{user_message_id}/assistant")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let after_body: serde_json::Value = after.json().await.unwrap();
    assert_eq!(
        after_body["assistant_id"].as_str().map(|s| Uuid::parse_str(s).unwrap()),
        Some(assistant_id),
        "ON CONFLICT must keep the ORIGINAL attribution, not the duplicate"
    );
}

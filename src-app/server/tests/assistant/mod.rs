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

/// Admin visibility: the template list (`WHERE is_template = true`, no enabled
/// filter — repository.rs:421) must include DISABLED templates so an admin can
/// re-enable or manage them. Previously only enabled-default templates were
/// exercised.
#[tokio::test]
async fn test_template_list_includes_disabled_templates() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tmpl_disabled_admin",
        &[
            "assistant_templates::create",
            "assistant_templates::edit",
            "assistant_templates::read",
        ],
    )
    .await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin.token);

    // Create a template (enabled by default).
    let created: serde_json::Value = client
        .post(server.api_url("/assistant-templates"))
        .header("Authorization", &bearer)
        .json(&json!({ "name": "Disabled Template Visible" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let template_id = created["id"].as_str().expect("template id").to_string();
    assert_eq!(created["enabled"], true);

    // Disable it.
    let upd = client
        .put(server.api_url(&format!("/assistant-templates/{template_id}")))
        .header("Authorization", &bearer)
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(upd.status(), StatusCode::OK, "disable should 200");
    let upd_body: serde_json::Value = upd.json().await.unwrap();
    assert_eq!(upd_body["enabled"], false, "template should be disabled now");

    // The admin template list must STILL include the now-disabled template.
    let list: serde_json::Value = client
        .get(server.api_url("/assistant-templates"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let found = list["assistants"]
        .as_array()
        .expect("assistants array")
        .iter()
        .find(|a| a["id"] == json!(template_id))
        .expect("the disabled template must still appear in the admin list");
    assert_eq!(
        found["enabled"], false,
        "the listed template carries its disabled state"
    );
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

/// CONCURRENT default-assistant race (repository.rs clear-defaults + set
/// transaction). The existing test_only_one_default_per_user is SEQUENTIAL; this
/// fires two simultaneous "set as default" updates on two different assistants
/// and asserts the invariant holds — EXACTLY ONE default remains, never two.
#[tokio::test]
async fn test_concurrent_set_default_yields_exactly_one() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "assistant_race",
        &["assistants::create", "assistants::read", "assistants::edit"],
    )
    .await;

    // Two non-default assistants.
    let mk = |name: &str| {
        let url = server.api_url("/assistants");
        let token = user.token.clone();
        let name = name.to_string();
        async move {
            let r = reqwest::Client::new()
                .post(url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "name": name, "is_default": false }))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::CREATED);
            r.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string()
        }
    };
    let id1 = mk("Race A").await;
    let id2 = mk("Race B").await;

    // Concurrently set BOTH as default.
    let set_default = |id: String| {
        let url = server.api_url(&format!("/assistants/{id}"));
        let token = user.token.clone();
        async move {
            reqwest::Client::new()
                .put(url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "is_default": true }))
                .send()
                .await
                .unwrap()
                .status()
        }
    };
    let (s1, s2) = tokio::join!(set_default(id1.clone()), set_default(id2.clone()));
    assert!(s1.is_success() && s2.is_success(), "both updates should succeed: {s1} {s2}");

    // Exactly one of the user's assistants is default — the race must not leave two.
    let list: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/assistants?page=1&per_page=100"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let assistants = list["assistants"].as_array().or_else(|| list["items"].as_array()).expect("list array");
    let default_count = assistants.iter().filter(|a| a["is_default"] == true).count();
    assert_eq!(default_count, 1, "exactly one default must remain after the race; got {default_count}");
}

/// A user POSTing /assistants with is_template:true must NOT create a template:
/// the create handler force-overrides request.is_template = Some(false)
/// (handlers.rs:127-128). Prevents a non-admin from minting a global template
/// via the user endpoint.
#[tokio::test]
async fn test_user_cannot_create_template_via_assistants_endpoint() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "tmpl_override_user",
        &["assistants::create", "assistants::read"],
    )
    .await;

    let response = reqwest::Client::new()
        .post(server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": "Sneaky Template", "is_template": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["is_template"], false,
        "the user endpoint must force is_template=false even when the client sends true: {body}"
    );
    assert_eq!(body["created_by"].is_null(), false, "a user assistant is owned by the creator");
}

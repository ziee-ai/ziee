// Integration tests for Assistant module

use serde_json::json;
use reqwest::StatusCode;
use uuid::Uuid;

// =====================================================
// Permission Tests
// =====================================================

#[tokio::test]
async fn test_list_assistants_requires_read_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_create_user_assistant_requires_create_permission() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &[]).await;

    let payload = json!({
        "name": "My Assistant"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create"]).await;

    let payload = json!({
        "name": "Template Assistant"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants-template"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::read"]).await;

    let response = reqwest::Client::new()
        .get(&server.api_url("/assistants-template"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create"]).await;

    let payload = json!({
        "name": "My Assistant",
        "description": "My personal assistant",
        "instructions": "Be helpful and concise"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::read"]).await;

    // Create two assistants
    create_user_assistant(&server, &user.token, "Assistant 1").await;
    create_user_assistant(&server, &user.token, "Assistant 2").await;

    // List user assistants
    let response = reqwest::Client::new()
        .get(&server.api_url("/assistants"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::read"]).await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "Test Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Get by ID
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants/{}", assistant_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::edit", "assistants::read"]).await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "Original Name").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Update
    let payload = json!({
        "name": "Updated Name",
        "description": "New description"
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/assistants/{}", assistant_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::delete", "assistants::read"]).await;

    // Create assistant
    let assistant = create_user_assistant(&server, &user.token, "To Delete").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // Delete
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify deleted
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants/{}", assistant_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants-template::create"]).await;

    let payload = json!({
        "name": "Template Assistant",
        "description": "A template for all users"
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants-template"))
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
    let admin = crate::common::test_helpers::create_user_with_permissions(&server, "admin", &["assistants-template::create", "assistants-template::read"]).await;

    // Create templates
    create_template_assistant(&server, &admin.token, "Template 1").await;
    create_template_assistant(&server, &admin.token, "Template 2").await;

    // List templates
    let response = reqwest::Client::new()
        .get(&server.api_url("/assistants-template"))
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
    let admin = crate::common::test_helpers::create_user_with_permissions(&server, "admin", &["assistants-template::create", "assistants-template::read"]).await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "Template").await;
    let template_id = template["id"].as_str().unwrap();

    // Get by ID
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants-template/{}", template_id)))
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
    let admin = crate::common::test_helpers::create_user_with_permissions(&server, "admin", &["assistants-template::create", "assistants-template::edit"]).await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "Original Template").await;
    let template_id = template["id"].as_str().unwrap();

    // Update
    let payload = json!({
        "name": "Updated Template"
    });

    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/assistants-template/{}", template_id)))
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
    let admin = crate::common::test_helpers::create_user_with_permissions(&server, "admin", &["assistants-template::create", "assistants-template::delete"]).await;

    // Create template
    let template = create_template_assistant(&server, &admin.token, "To Delete").await;
    let template_id = template["id"].as_str().unwrap();

    // Delete
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/assistants-template/{}", template_id)))
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
    let user1 = crate::common::test_helpers::create_user_with_permissions(&server, "user1", &["assistants::create", "assistants::read"]).await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(&server, "user2", &["assistants::read"]).await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to read it
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user2.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_user_cannot_edit_other_users_assistant() {
    let server = crate::common::TestServer::start().await;
    let user1 = crate::common::test_helpers::create_user_with_permissions(&server, "user1", &["assistants::create"]).await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(&server, "user2", &["assistants::edit"]).await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to edit it
    let payload = json!({"name": "Hacked"});
    let response = reqwest::Client::new()
        .put(&server.api_url(&format!("/assistants/{}", assistant_id)))
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
    let user1 = crate::common::test_helpers::create_user_with_permissions(&server, "user1", &["assistants::create"]).await;
    let user2 = crate::common::test_helpers::create_user_with_permissions(&server, "user2", &["assistants::delete"]).await;

    // User1 creates assistant
    let assistant = create_user_assistant(&server, &user1.token, "User1 Assistant").await;
    let assistant_id = assistant["id"].as_str().unwrap();

    // User2 tries to delete it
    let response = reqwest::Client::new()
        .delete(&server.api_url(&format!("/assistants/{}", assistant_id)))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::read"]).await;

    let payload = json!({
        "name": "My Default",
        "is_default": true
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
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
        .get(&server.api_url("/assistants/default"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create", "assistants::read"]).await;

    // Create first default
    let payload1 = json!({
        "name": "Default 1",
        "is_default": true
    });

    let response1 = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
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
        .post(&server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload2)
        .send()
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::CREATED);
    let assistant2: serde_json::Value = response2.json().await.unwrap();

    // Verify assistant1 is no longer default
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants/{}", assistant1["id"].as_str().unwrap())))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::create"]).await;

    let payload = json!({
        "name": ""
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
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
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "user", &["assistants::read"]).await;

    let assistant_id = Uuid::new_v4();
    let response = reqwest::Client::new()
        .get(&server.api_url(&format!("/assistants/{}", assistant_id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =====================================================
// Helper Functions
// =====================================================

async fn create_user_assistant(server: &crate::common::TestServer, token: &str, name: &str) -> serde_json::Value {
    let payload = json!({
        "name": name
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

async fn create_template_assistant(server: &crate::common::TestServer, token: &str, name: &str) -> serde_json::Value {
    let payload = json!({
        "name": name
    });

    let response = reqwest::Client::new()
        .post(&server.api_url("/assistants-template"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

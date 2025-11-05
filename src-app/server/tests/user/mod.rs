use serde_json::json;
use uuid::Uuid;
use crate::common::test_helpers::{self, TestUser};

// ============================================================================
// Admin User Management Tests with Permission Checks
// ============================================================================

#[tokio::test]
async fn test_list_users_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create admin user with users::read permission
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::read"]).await;

    // Create regular user without permission
    let user = test_helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Admin should be able to list users
    let url = server.api_url("/users");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should list users");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("users").is_some(), "Should have users array");
    assert!(body.get("total").is_some(), "Should have total count");

    // Regular user without permission should get 403
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 403, "Regular user should be forbidden");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body.get("error_code").and_then(|v| v.as_str()), Some("INSUFFICIENT_PERMISSIONS"));
}

#[tokio::test]
async fn test_list_users_with_pagination() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::read", "users::create"]).await;

    // Create multiple users
    for i in 0..15 {
        test_helpers::create_test_user(&server, &admin.token, &format!("user{}", i), "password123").await;
    }

    // Test first page
    let url = format!("{}/api/users?page=1&per_page=10", server.base_url);
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["page"], 1);
    assert_eq!(body["per_page"], 10);
    let users = body["users"].as_array().expect("Should have users array");
    assert!(users.len() <= 10);

    // Test second page
    let url = format!("{}/api/users?page=2&per_page=10", server.base_url);
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_get_user_by_id() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::read", "users::create"]).await;

    // Create a test user
    let new_user = test_helpers::create_test_user(&server, &admin.token, "testuser", "password123").await;
    let user_id = new_user["id"].as_str().expect("Should have user ID");

    // Get user by ID
    let url = server.api_url(&format!("/users/{}", user_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["username"], "testuser");
    assert_eq!(body["id"], user_id);
}

#[tokio::test]
async fn test_get_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::read"]).await;

    // Try to get non-existent user
    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/users/{}", fake_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_create_user() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

    let url = server.api_url("/users");
    let payload = json!({
        "username": "newuser",
        "email": "newuser@example.com",
        "password": "SecurePass123!",
        "display_name": "New User"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create user");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["username"], "newuser");
    assert_eq!(body["email"], "newuser@example.com");
    assert!(body.get("id").is_some());
}

#[tokio::test]
async fn test_create_user_duplicate_username() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

    // Create first user
    test_helpers::create_test_user(&server, &admin.token, "duplicateuser", "password123").await;

    // Try to create user with same username
    let url = server.api_url("/users");
    let payload = json!({
        "username": "duplicateuser",
        "email": "another@example.com",
        "password": "SecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 409, "Should conflict");
}

#[tokio::test]
async fn test_create_user_duplicate_email() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

    // Create first user
    let url = server.api_url("/users");
    let payload = json!({
        "username": "user1",
        "email": "duplicate@example.com",
        "password": "SecurePass123!"
    });

    let _ = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    // Try to create user with same email
    let payload = json!({
        "username": "user2",
        "email": "duplicate@example.com",
        "password": "SecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 409, "Should conflict");
}

#[tokio::test]
async fn test_create_user_validation() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

    let url = server.api_url("/users");

    // Empty username
    let payload = json!({
        "username": "",
        "email": "test@example.com",
        "password": "SecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject empty username");

    // Empty email
    let payload = json!({
        "username": "testuser",
        "email": "",
        "password": "SecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject empty email");
}

#[tokio::test]
async fn test_update_user() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create", "users::edit"]).await;

    // Create user
    let user = test_helpers::create_test_user(&server, &admin.token, "updateuser", "password123").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Update user
    let url = server.api_url(&format!("/users/{}", user_id));
    let payload = json!({
        "username": "updateduser",
        "email": "updated@example.com",
        "display_name": "Updated Name"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["username"], "updateduser");
    assert_eq!(body["email"], "updated@example.com");
    assert_eq!(body["display_name"], "Updated Name");
}

#[tokio::test]
async fn test_update_user_partial() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create", "users::edit"]).await;

    // Create user
    let user = test_helpers::create_test_user(&server, &admin.token, "partialuser", "password123").await;
    let user_id = user["id"].as_str().expect("Should have user ID");
    let original_email = user["email"].as_str().unwrap();

    // Update only username
    let url = server.api_url(&format!("/users/{}", user_id));
    let payload = json!({
        "username": "partialupdated"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["username"], "partialupdated");
    assert_eq!(body["email"], original_email, "Email should not change");
}

#[tokio::test]
async fn test_update_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::edit"]).await;

    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/users/{}", fake_id));
    let payload = json!({
        "username": "doesntmatter"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_delete_user() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create", "users::delete", "users::read"]).await;

    // Create user
    let user = test_helpers::create_test_user(&server, &admin.token, "deleteuser", "password123").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Delete user
    let url = server.api_url(&format!("/users/{}", user_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204);

    // Verify user is deleted
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_delete_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::delete"]).await;

    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/users/{}", fake_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_toggle_user_active() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create", "users::toggle_status"]).await;

    // Create user (initially active)
    let user = test_helpers::create_test_user(&server, &admin.token, "toggleuser", "password123").await;
    let user_id = user["id"].as_str().expect("Should have user ID");
    assert_eq!(user["is_active"], true);

    // Toggle to inactive
    let url = server.api_url(&format!("/users/{}/toggle-active", user_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["user_id"], user_id);
    assert_eq!(body["is_active"], false);

    // Toggle back to active
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["is_active"], true);
}

#[tokio::test]
async fn test_reset_user_password() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::create", "users::reset_password"]).await;

    // Create user
    let user = test_helpers::create_test_user(&server, &admin.token, "resetuser", "oldpassword").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Reset password
    let url = server.api_url("/users/reset-password");
    let payload = json!({
        "user_id": user_id,
        "new_password": "NewSecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_reset_password_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(&server, "admin", &["users::reset_password"]).await;

    let fake_id = Uuid::new_v4();
    let url = server.api_url("/users/reset-password");
    let payload = json!({
        "user_id": fake_id.to_string(),
        "new_password": "NewSecurePass123!"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_multiple_permissions() {
    let server = crate::common::TestServer::start().await;

    // User with both read and edit permissions
    let user = test_helpers::create_user_with_permissions(&server, "multiuser", &["users::read", "users::edit"]).await;

    // Should be able to list users (requires users::read)
    let url = server.api_url("/users");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should list with read permission");

    // Should be able to update users (requires users::edit)
    let update_url = server.api_url(&format!("/users/{}", user.user_id));
    let payload = json!({
        "display_name": "Updated Display Name"
    });

    let response = reqwest::Client::new()
        .post(&update_url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should update with edit permission");
}

#[tokio::test]
async fn test_unauthorized_without_token() {
    let server = crate::common::TestServer::start().await;

    let url = server.api_url("/users");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should be unauthorized without token");
}


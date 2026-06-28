use crate::common::test_helpers::{self};
use serde_json::json;
use uuid::Uuid;

mod sync_emit_test;

// ============================================================================
// Admin User Management Tests with Permission Checks
// ============================================================================

#[tokio::test]
async fn test_list_users_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create admin user with users::read permission
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::read"]).await;

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
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("INSUFFICIENT_PERMISSIONS")
    );
}

#[tokio::test]
async fn test_list_users_with_pagination() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::read", "users::create"],
    )
    .await;

    // Create multiple users
    for i in 0..15 {
        test_helpers::create_test_user(&server, &admin.token, &format!("user{}", i), "password123")
            .await;
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
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::read", "users::create"],
    )
    .await;

    // Create a test user
    let new_user =
        test_helpers::create_test_user(&server, &admin.token, "testuser", "password123").await;
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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::read"]).await;

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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::create"]).await;

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
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::edit"],
    )
    .await;

    // Create user
    let user =
        test_helpers::create_test_user(&server, &admin.token, "updateuser", "password123").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Update user.
    //
    // Note: the email field used to be writable here, but was removed
    // from UpdateUserRequest as part of closing 03-user F-03 (silent
    // email rewrite → OAuth account takeover). The test still sends an
    // email key in the body to exercise the silent-drop path, and
    // asserts the email did NOT change.
    let url = server.api_url(&format!("/users/{}", user_id));
    let payload = json!({
        "username": "updateduser",
        "email": "attacker@evil.com",
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
    assert_eq!(body["display_name"], "Updated Name");
    // The previous behavior accepted email here and we'd assert it equaled
    // the new value — now we assert email is UNCHANGED, proving the
    // F-03 fix is in place.
    assert_ne!(
        body["email"], "attacker@evil.com",
        "email field must NOT be writable through update_user — see 03-user F-03"
    );
}

#[tokio::test]
async fn test_update_user_partial() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::edit"],
    )
    .await;

    // Create user
    let user =
        test_helpers::create_test_user(&server, &admin.token, "partialuser", "password123").await;
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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::edit"]).await;

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
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::delete", "users::read"],
    )
    .await;

    // Create user
    let user =
        test_helpers::create_test_user(&server, &admin.token, "deleteuser", "password123").await;
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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::delete"]).await;

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
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::toggle_status"],
    )
    .await;

    // Create user (initially active)
    let user =
        test_helpers::create_test_user(&server, &admin.token, "toggleuser", "password123").await;
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
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::reset_password"],
    )
    .await;

    // Create user
    let user =
        test_helpers::create_test_user(&server, &admin.token, "resetuser", "oldpassword").await;
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
    let admin =
        test_helpers::create_user_with_permissions(&server, "admin", &["users::reset_password"])
            .await;

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
    let user = test_helpers::create_user_with_permissions(
        &server,
        "multiuser",
        &["users::read", "users::edit"],
    )
    .await;

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

    assert_eq!(
        response.status(),
        401,
        "Should be unauthorized without token"
    );
}

// ============================================================================
// Admin User Protection Tests
// ============================================================================

#[tokio::test]
async fn test_cannot_disable_admin_user_via_toggle() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::toggle_status", "users::edit"],
    )
    .await;

    // Create a regular user and set as admin via direct database update
    let test_user =
        test_helpers::create_test_user(&server, &admin.token, "adminuser", "password123").await;
    let user_id = test_user["id"].as_str().expect("Should have user ID");

    // Mark user as admin directly in database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query("UPDATE users SET is_admin = true WHERE id = $1")
        .bind(Uuid::parse_str(user_id).unwrap())
        .execute(&pool)
        .await
        .expect("Failed to update user to admin");

    pool.close().await;

    // Attempt to toggle admin user (should fail with 400)
    let url = server.api_url(&format!("/users/{}/toggle-active", user_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject disabling admin user");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("CANNOT_DISABLE_ADMIN"),
        "Should have correct error code"
    );
    assert!(
        body.get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("Cannot disable admin"))
            .unwrap_or(false),
        "Should have descriptive error message"
    );
}

#[tokio::test]
async fn test_cannot_disable_admin_user_via_update() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::edit"],
    )
    .await;

    // Create a regular user and set as admin via direct database update
    let test_user =
        test_helpers::create_test_user(&server, &admin.token, "adminuser2", "password123").await;
    let user_id = test_user["id"].as_str().expect("Should have user ID");

    // Mark user as admin directly in database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query("UPDATE users SET is_admin = true WHERE id = $1")
        .bind(Uuid::parse_str(user_id).unwrap())
        .execute(&pool)
        .await
        .expect("Failed to update user to admin");

    pool.close().await;

    // Attempt to disable admin user via update (should fail with 400)
    let url = server.api_url(&format!("/users/{}", user_id));
    let payload = json!({
        "is_active": false
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject disabling admin user");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("CANNOT_DISABLE_ADMIN"),
        "Should have correct error code"
    );
    assert!(
        body.get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("Cannot disable admin"))
            .unwrap_or(false),
        "Should have descriptive error message"
    );
}

#[tokio::test]
async fn test_can_enable_admin_user() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::toggle_status"],
    )
    .await;

    // Create a regular user, set as admin, and disable via direct database update
    let test_user =
        test_helpers::create_test_user(&server, &admin.token, "adminuser3", "password123").await;
    let user_id = test_user["id"].as_str().expect("Should have user ID");

    // Mark user as admin AND inactive directly in database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query("UPDATE users SET is_admin = true, is_active = false WHERE id = $1")
        .bind(Uuid::parse_str(user_id).unwrap())
        .execute(&pool)
        .await
        .expect("Failed to update user");

    pool.close().await;

    // Attempt to enable admin user (should succeed)
    let url = server.api_url(&format!("/users/{}/toggle-active", user_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should allow enabling admin user");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["is_active"], true, "Admin user should be enabled");
}

#[tokio::test]
async fn test_can_disable_non_admin_user() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::toggle_status"],
    )
    .await;

    // Create a regular user (not admin)
    let test_user =
        test_helpers::create_test_user(&server, &admin.token, "regularuser", "password123").await;
    let user_id = test_user["id"].as_str().expect("Should have user ID");
    assert_eq!(test_user["is_active"], true);

    // Toggle to disable (should succeed for non-admin users)
    let url = server.api_url(&format!("/users/{}/toggle-active", user_id));
    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "Should allow disabling non-admin user"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(
        body["is_active"], false,
        "Non-admin user should be disabled"
    );
}

#[tokio::test]
async fn test_can_update_admin_user_other_fields() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["users::create", "users::edit"],
    )
    .await;

    // Create a regular user and set as admin via direct database update
    let test_user =
        test_helpers::create_test_user(&server, &admin.token, "adminuser4", "password123").await;
    let user_id = test_user["id"].as_str().expect("Should have user ID");

    // Mark user as admin directly in database
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query("UPDATE users SET is_admin = true WHERE id = $1")
        .bind(Uuid::parse_str(user_id).unwrap())
        .execute(&pool)
        .await
        .expect("Failed to update user to admin");

    pool.close().await;

    // Update admin user's other fields (should succeed). Note: the
    // email field used to be writable, but was removed from
    // UpdateUserRequest as part of closing 03-user F-03 (silent email
    // rewrite → OAuth account takeover). We still send it to exercise
    // the silent-drop path.
    let url = server.api_url(&format!("/users/{}", user_id));
    let payload = json!({
        "display_name": "Updated Admin Display Name",
        "email": "attacker@evil.com"
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        200,
        "Should allow updating admin user's other fields"
    );

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["display_name"], "Updated Admin Display Name");
    assert_ne!(
        body["email"], "attacker@evil.com",
        "email must NOT be writable through update_user — see 03-user F-03"
    );
    assert_eq!(body["is_active"], true, "Admin user should remain active");
}

// =====================================================
// Body-limit regression test — close 14-core F-01
// =====================================================
//
// The previous DefaultBodyLimit::disable() applied globally let any
// unauthenticated POST exhaust memory by streaming a multi-GB body.
// The fix sets a global 16MB cap so non-upload routes return 413 for
// any request body larger than that; the actual upload routes
// (file upload, model upload) opt into a higher per-route cap.

#[tokio::test]
async fn test_body_limit_rejects_oversized_post_to_register() {
    let server = crate::common::TestServer::start().await;

    // Construct a 20 MB JSON body — larger than the 16 MB global cap,
    // smaller than upload-route caps. The route is unauthenticated so
    // this exercises the unauth DoS scenario the audit flagged.
    let big_padding = "A".repeat(20 * 1024 * 1024);
    let body = serde_json::json!({
        "username": "x",
        "email": "x@example.com",
        "password": "x",
        "padding": big_padding,
    });

    let res = reqwest::Client::new()
        .post(server.api_url("/auth/register"))
        .json(&body)
        .send()
        .await
        .expect("request failed");

    assert_eq!(
        res.status(),
        413,
        "20 MB POST to /auth/register must be rejected with 413 (was: {})",
        res.status()
    );
}

// =====================================================
// Privilege-escalation regression test — close 03-user F-01
// =====================================================
//
// The previous UpdateUserRequest.permissions: Option<Vec<String>> let any
// holder of the `users::edit` permission rewrite the permissions array
// of any user (including themselves) via PUT /api/users/{id}. With
// permissions: ["*"] this was near-root escalation from a single
// sub-admin grant. Closes 03-user F-01 (Critical).
//
// The fix removes the permissions field from UpdateUserRequest entirely.
// Permission management is handled separately by group assignment +
// future dedicated set_permissions endpoint (A4).

#[tokio::test]
async fn test_users_edit_cannot_grant_wildcard_via_update() {
    let server = crate::common::TestServer::start().await;

    // Caller has users::edit but is NOT a wildcard / admin.
    let attacker = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "attacker",
        &["users::edit", "users::read"],
    )
    .await;

    // Victim is the attacker themselves — try to escalate.
    let target_user_id = attacker.user_id;

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/users/{}", target_user_id)))
        .header("Authorization", format!("Bearer {}", attacker.token))
        .json(&serde_json::json!({
            "permissions": ["*"]
        }))
        .send()
        .await
        .expect("request failed");

    // After the fix, the request body's unknown `permissions` field is
    // silently dropped by serde (the DTO no longer has it). The update
    // proceeds with no changes, so we expect 200 — BUT the user's
    // actual permissions in the DB must NOT contain '*'.
    assert!(
        res.status().is_success() || res.status() == 400,
        "expected 200 or 400, got {}",
        res.status()
    );

    // Verify in the DB by fetching the user (via /me which returns
    // current user's full record including permissions).
    let me = reqwest::Client::new()
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", attacker.token))
        .send()
        .await
        .expect("me fetch failed")
        .json::<serde_json::Value>()
        .await
        .expect("me parse failed");

    let perms = me
        .get("permissions")
        .and_then(|v| v.as_array())
        .expect("user must have permissions array");
    assert!(
        !perms.iter().any(|p| p.as_str() == Some("*")),
        "attacker successfully escalated to wildcard '*' — F-01 NOT fixed; got perms: {:?}",
        perms
    );
}

// =====================================================
// delete_user is_admin-guard regression — close 03-user F-02
// =====================================================
//
// toggle_user_active and update_user already refuse to act on admin
// users; delete_user has no such guard. A user with `users::delete` can
// DELETE the root admin and brick the deployment (unique_root_admin
// partial index prevents re-creation). Audit 03-user F-02 (Critical).

#[tokio::test]
async fn test_delete_user_refuses_to_delete_admin() {
    let server = crate::common::TestServer::start().await;

    // The TestServer harness sets up a setup-flow admin during startup;
    // we need to find that admin's user_id. Easiest path: register a
    // non-admin user with `users::read`, list users, find one with
    // is_admin: true.
    let client = reqwest::Client::new();

    // Create the root admin via the setup flow (TestServer starts with no admin).
    let setup_resp: serde_json::Value = client
        .post(server.api_url("/app/setup/admin"))
        .json(&serde_json::json!({
            "username": "delete_admin_target",
            "email": "delete_admin@example.com",
            "password": "SecurePass123!",
        }))
        .send()
        .await
        .expect("setup admin request failed")
        .json()
        .await
        .expect("setup admin parse failed");

    let admin_id = setup_resp
        .get("user")
        .and_then(|u| u.get("id"))
        .and_then(|id| id.as_str())
        .expect("setup response must have user.id")
        .to_string();

    // Now mint a user with users::delete and try to delete the admin.
    let attacker = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "attacker",
        &["users::delete"],
    )
    .await;

    let res = client
        .delete(server.api_url(&format!("/users/{}", admin_id)))
        .header("Authorization", format!("Bearer {}", attacker.token))
        .send()
        .await
        .expect("delete request failed");

    assert!(
        res.status() == 400 || res.status() == 403,
        "DELETE /users/{} (admin) must be rejected with 400 or 403, got {}",
        admin_id,
        res.status()
    );

    // Verify admin still exists by trying to log in as them.
    let login = client
        .post(server.api_url("/auth/login"))
        .json(&serde_json::json!({
            "username": "delete_admin_target",
            "password": "SecurePass123!",
        }))
        .send()
        .await
        .expect("login failed");

    assert!(
        login.status().is_success(),
        "admin {} was deleted — login now {}; deployment would be bricked",
        admin_id,
        login.status()
    );
}

// =====================================================
// create_user prevent-self-escalation — close 03-user F-04
// =====================================================
//
// CreateUserRequest.permissions accepts any string the caller writes
// including "*". A holder of users::create can mint a wildcard root by
// POSTing {"permissions": ["*"]}. Audit 03-user F-04 (High).
//
// The fix verifies that every permission the caller is trying to grant
// is one the caller themselves holds (via user perms or group union).
// Admins (is_admin=true) bypass this check.

#[tokio::test]
async fn test_create_user_refuses_granting_perms_caller_lacks() {
    let server = crate::common::TestServer::start().await;

    // Caller has users::create and users::read but NOT '*' and is NOT admin.
    let creator = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "creator",
        &["users::create", "users::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/users"))
        .header("Authorization", format!("Bearer {}", creator.token))
        .json(&serde_json::json!({
            "username": "minted_root",
            "email": "minted_root@example.com",
            "password": "SecurePass123!",
            "permissions": ["*"],
        }))
        .send()
        .await
        .expect("create request failed");

    assert!(
        res.status() == 403 || res.status() == 400,
        "create_user with permissions: ['*'] from non-admin caller must be rejected, got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_create_user_refuses_granting_unrelated_perm() {
    let server = crate::common::TestServer::start().await;

    // Caller has users::create but NOT users::delete (an admin-only
    // perm not granted by the default group).
    let creator = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "creator2",
        &["users::create", "users::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/users"))
        .header("Authorization", format!("Bearer {}", creator.token))
        .json(&serde_json::json!({
            "username": "minted_deleter",
            "email": "minted_deleter@example.com",
            "password": "SecurePass123!",
            "permissions": ["users::delete"],
        }))
        .send()
        .await
        .expect("create request failed");

    assert!(
        res.status() == 403 || res.status() == 400,
        "caller without users::delete should not be able to grant users::delete; got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_create_user_allows_granting_perm_caller_holds() {
    let server = crate::common::TestServer::start().await;

    // Caller holds both users::create AND files::read → can grant files::read.
    let creator = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "creator3",
        &["users::create", "files::read"],
    )
    .await;

    let res = reqwest::Client::new()
        .post(server.api_url("/users"))
        .header("Authorization", format!("Bearer {}", creator.token))
        .json(&serde_json::json!({
            "username": "files_reader_target",
            "email": "files_reader@example.com",
            "password": "SecurePass123!",
            "permissions": ["files::read"],
        }))
        .send()
        .await
        .expect("create request failed");

    assert!(
        res.status().is_success(),
        "caller with files::read should be able to grant files::read; got {}",
        res.status()
    );
}

// =====================================================
// Group privilege-escalation — close 02-permissions F-02
// =====================================================
//
// update_group's is_system check only protected `name` and `is_active`,
// NOT `permissions`. A holder of groups::edit could POST
// {"permissions": ["*"]} to the system default Users group and cascade
// '*' to every existing user — mass escalation. Audit 02-permissions
// F-02 (High).
//
// Tests live in tests/user/ because tests/user_group/ has its own broken
// helpers module (references missing TEST_CONFIG) and isn't registered
// in integration_tests.rs.

#[tokio::test]
async fn test_update_group_system_default_refuses_permission_change() {
    let server = crate::common::TestServer::start().await;
    let editor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "editor",
        &["groups::edit", "groups::read"],
    )
    .await;

    let groups: serde_json::Value = reqwest::Client::new()
        .get(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", editor.token))
        .send()
        .await
        .expect("list groups failed")
        .json()
        .await
        .expect("parse groups failed");

    let default_group_id = groups
        .get("groups")
        .and_then(|g| g.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|g| {
                    g.get("is_system").and_then(|v| v.as_bool()) == Some(true)
                        && g.get("is_default").and_then(|v| v.as_bool()) == Some(true)
                })
                .and_then(|g| g.get("id").and_then(|id| id.as_str()))
                .map(String::from)
        })
        .expect("no default system group found");

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/groups/{}", default_group_id)))
        .header("Authorization", format!("Bearer {}", editor.token))
        .json(&serde_json::json!({
            "permissions": ["*"]
        }))
        .send()
        .await
        .expect("update request failed");

    assert!(
        res.status() == 400 || res.status() == 403,
        "system default group permission change must be rejected, got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_update_group_prevents_self_escalation_on_custom_group() {
    let server = crate::common::TestServer::start().await;
    let editor = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "editor",
        &["groups::edit", "groups::read", "groups::create"],
    )
    .await;

    let new_group: serde_json::Value = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", editor.token))
        .json(&serde_json::json!({
            "name": format!("priv-esc-test-{}", Uuid::new_v4()),
            "description": "test",
            "permissions": []
        }))
        .send()
        .await
        .expect("create group failed")
        .json()
        .await
        .expect("parse failed");

    let group_id = new_group
        .get("id")
        .and_then(|id| id.as_str())
        .expect("group id missing");

    let res = reqwest::Client::new()
        .post(server.api_url(&format!("/groups/{}", group_id)))
        .header("Authorization", format!("Bearer {}", editor.token))
        .json(&serde_json::json!({
            "permissions": ["users::delete"]
        }))
        .send()
        .await
        .expect("update request failed");

    assert!(
        res.status() == 403 || res.status() == 400,
        "caller without users::delete cannot grant it via groups; got {}",
        res.status()
    );
}

/// delete_group must refuse to delete a SYSTEM group (400 SYSTEM_GROUP) — the
/// built-in Users/Administrators groups are load-bearing and deleting one
/// would brick auth/permission resolution. A non-system group deletes fine.
#[tokio::test]
async fn test_delete_group_refuses_system_group() {
    let server = crate::common::TestServer::start().await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "group_deleter",
        &["groups::delete", "groups::read", "groups::create"],
    )
    .await;
    let client = reqwest::Client::new();
    let bearer = format!("Bearer {}", admin.token);

    // Find a system group from the list.
    let groups: serde_json::Value = client
        .get(server.api_url("/groups"))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let system_group_id = groups["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["is_system"].as_bool() == Some(true))
        .and_then(|g| g["id"].as_str())
        .expect("a system group must exist")
        .to_string();

    let res = client
        .delete(server.api_url(&format!("/groups/{system_group_id}")))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "deleting a system group must be rejected");
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body.get("error_code").and_then(|v| v.as_str()),
        Some("SYSTEM_GROUP"),
        "must be the SYSTEM_GROUP error: {body}"
    );

    // A freshly created non-system group deletes cleanly (204).
    let created: serde_json::Value = client
        .post(server.api_url("/groups"))
        .header("Authorization", &bearer)
        .json(&serde_json::json!({ "name": "deletable-grp", "permissions": [] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let new_id = created["id"].as_str().expect("new group id");
    let del = client
        .delete(server.api_url(&format!("/groups/{new_id}")))
        .header("Authorization", &bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204, "a non-system group must delete cleanly");
}

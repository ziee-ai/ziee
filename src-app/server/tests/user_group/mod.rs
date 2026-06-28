use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Group Management Tests with Permission Checks
// ============================================================================

#[tokio::test]
async fn test_list_groups_requires_permission() {
    let server = crate::common::TestServer::start().await;

    // Create admin user with groups::read permission
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::read"]).await;

    // Create regular user without permission
    let user = helpers::create_user_with_permissions(&server, "regular", &[]).await;

    // Admin should be able to list groups
    let url = server.api_url("/groups");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Admin should list groups");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert!(body.get("groups").is_some(), "Should have groups array");
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
async fn test_list_groups_with_pagination() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::read", "groups::create"]).await;

    // Create multiple groups
    for i in 0..15 {
        helpers::create_test_group(&server, &admin.token, &format!("group{}", i)).await;
    }

    // Test first page
    let url = format!("{}/api/groups?page=1&per_page=10", server.base_url);
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
    let groups = body["groups"].as_array().expect("Should have groups array");
    assert!(groups.len() <= 10);
}

#[tokio::test]
async fn test_get_group_by_id() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::read", "groups::create"]).await;

    // Create a test group
    let new_group = helpers::create_test_group(&server, &admin.token, "testgroup").await;
    let group_id = new_group["id"].as_str().expect("Should have group ID");

    // Get group by ID
    let url = server.api_url(&format!("/groups/{}", group_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["name"], "testgroup");
    assert_eq!(body["id"], group_id);
}

#[tokio::test]
async fn test_get_group_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::read"]).await;

    // Try to get non-existent group
    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/groups/{}", fake_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_create_group() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create"]).await;

    let url = server.api_url("/groups");
    let payload = json!({
        "name": "newgroup",
        "description": "A new test group",
        "permissions": ["users::read", "groups::read"]
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 201, "Should create group");

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["name"], "newgroup");
    assert_eq!(body["description"], "A new test group");
    assert!(body.get("id").is_some());
    assert_eq!(body["permissions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_create_group_duplicate_name() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create"]).await;

    // Create first group
    helpers::create_test_group(&server, &admin.token, "duplicategroup").await;

    // Try to create group with same name
    let url = server.api_url("/groups");
    let payload = json!({
        "name": "duplicategroup",
        "description": "Another group",
        "permissions": []
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
async fn test_create_group_validation() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create"]).await;

    let url = server.api_url("/groups");

    // Empty name
    let payload = json!({
        "name": "",
        "description": "Test",
        "permissions": []
    });

    let response = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 400, "Should reject empty name");
}

#[tokio::test]
async fn test_update_group() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::edit"]).await;

    // Create group
    let group = helpers::create_test_group(&server, &admin.token, "updategroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    // Update group
    let url = server.api_url(&format!("/groups/{}", group_id));
    let payload = json!({
        "name": "updatedgroup",
        "description": "Updated description",
        "permissions": ["users::edit"]
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
    assert_eq!(body["name"], "updatedgroup");
    assert_eq!(body["description"], "Updated description");
}

#[tokio::test]
async fn test_update_group_partial() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::edit"]).await;

    // Create group
    let group = helpers::create_test_group(&server, &admin.token, "partialgroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");
    let original_description = group["description"].as_str();

    // Update only name
    let url = server.api_url(&format!("/groups/{}", group_id));
    let payload = json!({
        "name": "partialupdated"
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
    assert_eq!(body["name"], "partialupdated");

    if let Some(desc) = original_description {
        assert_eq!(body["description"], desc, "Description should not change");
    }
}

#[tokio::test]
async fn test_update_group_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::edit"]).await;

    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/groups/{}", fake_id));
    let payload = json!({
        "name": "doesntmatter"
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
async fn test_delete_group() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::delete", "groups::read"]).await;

    // Create group
    let group = helpers::create_test_group(&server, &admin.token, "deletegroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    // Delete group
    let url = server.api_url(&format!("/groups/{}", group_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204);

    // Verify group is deleted
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_delete_group_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::delete"]).await;

    let fake_id = Uuid::new_v4();
    let url = server.api_url(&format!("/groups/{}", fake_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_get_group_members() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::read", "users::create", "groups::assign_users"]).await;

    // Create group
    let group = helpers::create_test_group(&server, &admin.token, "membergroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    // Create and assign users
    let user1 = helpers::create_test_user_via_api(&server, &admin.token, "member1").await;
    let user2 = helpers::create_test_user_via_api(&server, &admin.token, "member2").await;

    helpers::assign_user_to_group(&server, &admin.token, &user1["id"].as_str().unwrap(), group_id).await;
    helpers::assign_user_to_group(&server, &admin.token, &user2["id"].as_str().unwrap(), group_id).await;

    // Get group members
    let url = server.api_url(&format!("/groups/{}/members", group_id));
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    let users = body["users"].as_array().expect("Should have users array");
    assert!(users.len() >= 2, "Should have at least 2 members");
}

#[tokio::test]
async fn test_assign_user_to_group() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "users::create", "groups::assign_users"]).await;

    // Create group and user
    let group = helpers::create_test_group(&server, &admin.token, "assigngroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    let user = helpers::create_test_user_via_api(&server, &admin.token, "assignuser").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Assign user to group
    let url = server.api_url("/groups/assign");
    let payload = json!({
        "user_id": user_id,
        "group_id": group_id
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
async fn test_assign_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::assign_users"]).await;

    let group = helpers::create_test_group(&server, &admin.token, "testgroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    let fake_user_id = Uuid::new_v4();

    let url = server.api_url("/groups/assign");
    let payload = json!({
        "user_id": fake_user_id.to_string(),
        "group_id": group_id
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
async fn test_assign_group_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["users::create", "groups::assign_users"]).await;

    let user = helpers::create_test_user_via_api(&server, &admin.token, "testuser").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    let fake_group_id = Uuid::new_v4();

    let url = server.api_url("/groups/assign");
    let payload = json!({
        "user_id": user_id,
        "group_id": fake_group_id.to_string()
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
async fn test_remove_user_from_group() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "users::create", "groups::assign_users"]).await;

    // Create group and user
    let group = helpers::create_test_group(&server, &admin.token, "removegroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    let user = helpers::create_test_user_via_api(&server, &admin.token, "removeuser").await;
    let user_id = user["id"].as_str().expect("Should have user ID");

    // Assign user to group first
    helpers::assign_user_to_group(&server, &admin.token, user_id, group_id).await;

    // Remove user from group
    let url = server.api_url(&format!("/groups/{}/{}/remove", user_id, group_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 204);
}

#[tokio::test]
async fn test_remove_user_not_found() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(&server, "admin", &["groups::create", "groups::assign_users"]).await;

    let group = helpers::create_test_group(&server, &admin.token, "testgroup").await;
    let group_id = group["id"].as_str().expect("Should have group ID");

    let fake_user_id = Uuid::new_v4();

    let url = server.api_url(&format!("/groups/{}/{}/remove", fake_user_id, group_id));
    let response = reqwest::Client::new()
        .delete(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_multiple_permissions_groups() {
    let server = crate::common::TestServer::start().await;

    // User with both read and edit permissions
    let user = helpers::create_user_with_permissions(&server, "multiuser", &["groups::read", "groups::edit"]).await;

    // Should be able to list groups (requires groups::read)
    let url = server.api_url("/groups");
    let response = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 200, "Should list with read permission");
}

#[tokio::test]
async fn test_unauthorized_without_token_groups() {
    let server = crate::common::TestServer::start().await;

    let url = server.api_url("/groups");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Should be unauthorized without token");
}

/// System groups (is_system = true, e.g. the seeded Administrators/Users
/// groups) must reject mutations to their core attributes — name,
/// deactivation, and permissions — even from a `groups::edit` holder, while
/// still allowing a benign field (description) to change. This guards
/// 02-permissions F-02: without it any edit-holder could rewrite the default
/// Users group's permissions to `['*']` and cascade wildcard to every user.
#[tokio::test]
async fn test_update_group_system_group_protection() {
    use crate::common::TEST_CONFIG;

    let server = crate::common::TestServer::start().await;
    let admin =
        helpers::create_user_with_permissions(&server, "admin", &["groups::edit"]).await;

    // Seed a SYSTEM group directly (is_system = true) — the API offers no way
    // to create one, mirroring the seeded Administrators/Users groups.
    let database_url = format!(
        "postgresql://{}:{}@{}:{}/{}",
        TEST_CONFIG.pg_username,
        TEST_CONFIG.pg_password,
        TEST_CONFIG.pg_bind_address,
        TEST_CONFIG.pg_port,
        server.database_name
    );
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database");

    let group_id = Uuid::new_v4();
    let group_name = format!("sys_group_{}", &group_id.to_string()[..8]);
    let perms: Vec<String> = vec!["users::read".to_string()];
    sqlx::query(
        "INSERT INTO groups (id, name, description, permissions, is_system, is_active, created_at, updated_at)
         VALUES ($1, $2, $3, $4, true, true, NOW(), NOW())",
    )
    .bind(group_id)
    .bind(&group_name)
    .bind("Protected system group")
    .bind(&perms)
    .execute(&pool)
    .await
    .expect("Failed to seed system group");
    pool.close().await;

    let url = server.api_url(&format!("/groups/{}", group_id));
    let client = reqwest::Client::new();

    // Each forbidden mutation must be rejected with 400 / SYSTEM_GROUP.
    for (label, payload) in [
        ("rename", json!({ "name": "renamed_system_group" })),
        ("permissions", json!({ "permissions": ["*"] })),
        ("deactivate", json!({ "is_active": false })),
    ] {
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin.token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");
        assert_eq!(
            response.status(),
            400,
            "System group {label} mutation must be rejected"
        );
        let body: serde_json::Value =
            response.json().await.expect("Failed to parse JSON");
        assert_eq!(
            body.get("error_code").and_then(|v| v.as_str()),
            Some("SYSTEM_GROUP"),
            "System group {label} rejection must carry the SYSTEM_GROUP code"
        );
    }

    // A benign, allowed field (description) on the SAME system group still
    // succeeds — proving the guard is targeted, not a blanket block.
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "description": "Updated by admin" }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        response.status(),
        200,
        "Description-only update on a system group must be allowed"
    );
    let body: serde_json::Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["description"], "Updated by admin");
    assert_eq!(
        body["name"], group_name,
        "Name must be unchanged after the allowed update"
    );
}

// ============================================================================
// Helper Functions Module
// ============================================================================

mod helpers {
    use super::*;
    use crate::common::TestServer;

    /// Test user with token and ID
    pub struct TestUser {
        pub token: String,
        pub user_id: String,
    }

    /// Create a user with specific permissions for testing
    pub async fn create_user_with_permissions(server: &TestServer, username: &str, permissions: &[&str]) -> TestUser {
        use crate::common::TEST_CONFIG;

        let unique_username = format!("{}_{}", username, &Uuid::new_v4().to_string()[..8]);

        // Register user via API to get a valid JWT token
        let register_response = reqwest::Client::new()
            .post(&server.api_url("/auth/register"))
            .json(&json!({
                "username": &unique_username,
                "email": format!("{}@example.com", unique_username),
                "password": "password123"
            }))
            .send()
            .await
            .expect("Failed to register user");

        assert_eq!(register_response.status(), 201, "Registration should succeed");

        let register_body: serde_json::Value = register_response
            .json()
            .await
            .expect("Failed to parse register response");

        let token = register_body["access_token"]
            .as_str()
            .expect("access_token missing")
            .to_string();
        let user_id = register_body["user"]["id"]
            .as_str()
            .expect("user id missing")
            .to_string();

        // If permissions are needed, create a group and assign user to it
        if !permissions.is_empty() {
            // Connect to database to assign permissions
            let database_url = format!(
                "postgresql://{}:{}@{}:{}/{}",
                TEST_CONFIG.pg_username,
                TEST_CONFIG.pg_password,
                TEST_CONFIG.pg_bind_address,
                TEST_CONFIG.pg_port,
                server.database_name
            );

            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
                .expect("Failed to connect to test database");

            let group_id = Uuid::new_v4();
            let group_name = format!("test_group_{}", &group_id.to_string()[..8]);
            let permissions_json: Vec<String> = permissions.iter().map(|s| s.to_string()).collect();

            sqlx::query(
                "INSERT INTO groups (id, name, description, permissions, is_system, is_active, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, false, true, NOW(), NOW())"
            )
            .bind(group_id)
            .bind(&group_name)
            .bind("Test group for permissions")
            .bind(&permissions_json)
            .execute(&pool)
            .await
            .expect("Failed to create test group");

            // Assign user to group
            let user_uuid = Uuid::parse_str(&user_id).expect("Invalid user ID");
            sqlx::query(
                "INSERT INTO user_groups (user_id, group_id, assigned_at)
                 VALUES ($1, $2, NOW())"
            )
            .bind(user_uuid)
            .bind(group_id)
            .execute(&pool)
            .await
            .expect("Failed to assign user to group");

            pool.close().await;
        }

        TestUser { token, user_id }
    }

    /// Create a test group via API
    pub async fn create_test_group(server: &TestServer, admin_token: &str, name: &str) -> serde_json::Value {
        let url = server.api_url("/groups");
        let payload = json!({
            "name": name,
            "description": format!("Test group {}", name),
            "permissions": []
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin_token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Failed to create test group");
        response.json().await.expect("Failed to parse JSON")
    }

    /// Create a test user via API
    pub async fn create_test_user_via_api(server: &TestServer, admin_token: &str, username: &str) -> serde_json::Value {
        let url = server.api_url("/users");
        let payload = json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": "password123"
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin_token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 201, "Failed to create test user");
        response.json().await.expect("Failed to parse JSON")
    }

    /// Assign user to group
    pub async fn assign_user_to_group(server: &TestServer, admin_token: &str, user_id: &str, group_id: &str) {
        let url = server.api_url("/groups/assign");
        let payload = json!({
            "user_id": user_id,
            "group_id": group_id
        });

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", admin_token))
            .json(&payload)
            .send()
            .await
            .expect("Request failed");

        assert_eq!(response.status(), 204, "Failed to assign user to group");
    }

    /// Generate a test JWT token for a user
    fn generate_test_jwt(user_id: String) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use serde::{Deserialize, Serialize};
        use std::time::{SystemTime, UNIX_EPOCH};

        #[derive(Debug, Serialize, Deserialize)]
        struct Claims {
            sub: String,
            exp: usize,
            iat: usize,
            iss: String,
            aud: String,
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as usize;

        let claims = Claims {
            sub: user_id,
            exp: now + 86400, // 24 hours
            iat: now,
            iss: "ziee-test".to_string(),
            aud: "ziee-test-api".to_string(),
        };

        let secret = "test-secret-key-for-jwt-tokens-min-32-chars-long";
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("Failed to generate JWT")
    }
}

/// Assigning a user who is ALREADY in the group is an idempotent no-op
/// (ON CONFLICT DO NOTHING): the second assign still returns 204 and the user
/// appears exactly once in the membership — no duplicate row, no error.
#[tokio::test]
async fn test_assign_user_already_in_group_is_idempotent() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(
        &server,
        "admin",
        &["groups::create", "groups::read", "users::create", "groups::assign_users"],
    )
    .await;

    let group = helpers::create_test_group(&server, &admin.token, "idemgroup").await;
    let group_id = group["id"].as_str().expect("group ID");
    let user = helpers::create_test_user_via_api(&server, &admin.token, "idemuser").await;
    let user_id = user["id"].as_str().expect("user ID");

    let assign = |client: reqwest::Client| {
        let url = server.api_url("/groups/assign");
        let payload = json!({ "user_id": user_id, "group_id": group_id });
        async move {
            client
                .post(&url)
                .header("Authorization", format!("Bearer {}", admin.token))
                .json(&payload)
                .send()
                .await
                .expect("assign request failed")
        }
    };

    // First assign + a redundant second assign — both must succeed (204).
    assert_eq!(assign(reqwest::Client::new()).await.status(), 204);
    assert_eq!(
        assign(reqwest::Client::new()).await.status(),
        204,
        "re-assigning an existing member must still be 204 (idempotent)"
    );

    // The user must appear exactly once in the membership.
    let members: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{}/members", group_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("members request failed")
        .json()
        .await
        .expect("parse members");
    let count = members["users"]
        .as_array()
        .expect("users array")
        .iter()
        .filter(|u| u["id"].as_str() == Some(user_id))
        .count();
    assert_eq!(count, 1, "double-assign must not duplicate the membership row");
}

/// Assigning a user who is ALREADY in the group is an idempotent no-op:
/// the second assign still succeeds (204, ON CONFLICT DO NOTHING) and the
/// group still has exactly ONE membership row for that user (no duplicate).
#[tokio::test]
async fn test_assign_user_already_in_group_is_noop() {
    let server = crate::common::TestServer::start().await;
    let admin = helpers::create_user_with_permissions(
        &server,
        "noop_admin",
        &["groups::create", "users::create", "groups::assign_users", "groups::read"],
    )
    .await;

    let group = helpers::create_test_group(&server, &admin.token, "noopgroup").await;
    let group_id = group["id"].as_str().expect("group id");
    let user = helpers::create_test_user_via_api(&server, &admin.token, "noopuser").await;
    let user_id = user["id"].as_str().expect("user id");

    let assign = || {
        let url = server.api_url("/groups/assign");
        let token = admin.token.clone();
        async move {
            reqwest::Client::new()
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&json!({ "user_id": user_id, "group_id": group_id }))
                .send()
                .await
                .expect("assign")
        }
    };

    assert_eq!(assign().await.status(), 204, "first assign");
    assert_eq!(assign().await.status(), 204, "second assign is an idempotent no-op");

    // Exactly one membership row — the no-op did not duplicate it.
    let members: serde_json::Value = reqwest::Client::new()
        .get(server.api_url(&format!("/groups/{group_id}/members?page=1&per_page=100")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("members")
        .json()
        .await
        .expect("members body");
    let count = members["users"]
        .as_array()
        .expect("users array")
        .iter()
        .filter(|u| u["id"].as_str() == Some(user_id))
        .count();
    assert_eq!(count, 1, "double-assign must not duplicate the membership: {members}");
}

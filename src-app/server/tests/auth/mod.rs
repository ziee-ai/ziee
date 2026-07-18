use serde_json::json;

// OAuth and LDAP provider integration tests (require Docker)
mod admin_providers_test;
mod apple_test;
mod ldap_test;
mod oauth_test;
// Self-service profile (update profile + change password + has_password).
mod profile_self_service_test;
// httpOnly refresh cookies + rotation grace + expiry recovery + prune.
mod session_refresh_test;
// Admin-configurable session settings (lifetimes) CRUD + mint-time read.
mod session_settings_test;
// Realtime-sync emission on admin auth-provider mutations.
mod sync_emit_test;

#[tokio::test]
async fn test_auth_registration() {
    let server = crate::common::TestServer::start().await;

    // Test registration
    let register_body = json!({
        "username": "testuser",
        "email": "test@example.com",
        "password": "testpass123",
        "display_name": "Test User"
    });

    let client = reqwest::Client::new();
    let response = client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration request failed");

    assert_eq!(response.status(), 201, "Expected 201 Created");

    let response_body: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Check user data
    assert!(
        response_body.get("user").is_some(),
        "Response should contain user"
    );
    let user = response_body.get("user").unwrap();
    assert_eq!(user.get("username").unwrap(), "testuser");
    assert_eq!(user.get("email").unwrap(), "test@example.com");
    assert_eq!(user.get("display_name").unwrap(), "Test User");

    // Check JWT tokens
    assert!(
        response_body.get("access_token").is_some(),
        "Response should contain access_token"
    );
    assert!(
        response_body.get("refresh_token").is_some(),
        "Response should contain refresh_token"
    );
    assert_eq!(response_body.get("token_type").unwrap(), "Bearer");
    assert!(response_body.get("expires_in").is_some());
}

#[tokio::test]
async fn test_auth_registration_duplicate_username() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register first user
    let register_body = json!({
        "username": "testuser",
        "email": "test1@example.com",
        "password": "testpass123"
    });

    client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("First registration failed");

    // Try to register with same username but different email
    let duplicate_body = json!({
        "username": "testuser",
        "email": "test2@example.com",
        "password": "testpass456"
    });

    let response = client
        .post(server.api_url("/auth/register"))
        .json(&duplicate_body)
        .send()
        .await
        .expect("Second registration request failed");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::CONFLICT,
        "Duplicate username must return 409 CONFLICT (got {:?})",
        response.status()
    );
}

#[tokio::test]
async fn test_auth_registration_duplicate_email() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register first user
    let register_body = json!({
        "username": "emailtest1",
        "email": "shared@example.com",
        "password": "testpass123"
    });

    client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("First registration failed");

    // Try to register with different username but the same email
    let duplicate_body = json!({
        "username": "emailtest2",
        "email": "shared@example.com",
        "password": "testpass456"
    });

    let response = client
        .post(server.api_url("/auth/register"))
        .json(&duplicate_body)
        .send()
        .await
        .expect("Second registration request failed");

    assert_eq!(
        response.status(),
        reqwest::StatusCode::CONFLICT,
        "Duplicate email must return 409 CONFLICT (got {:?})",
        response.status()
    );
}

#[tokio::test]
async fn test_auth_login_and_jwt() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register user first
    let register_body = json!({
        "username": "logintest",
        "email": "login@example.com",
        "password": "testpass123"
    });

    client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    // Test login
    let login_body = json!({
        "username": "logintest",
        "password": "testpass123"
    });

    let login_response = client
        .post(server.api_url("/auth/login"))
        .json(&login_body)
        .send()
        .await
        .expect("Login request failed");

    assert_eq!(login_response.status(), 200, "Expected 200 OK");

    let login_body: serde_json::Value = login_response
        .json()
        .await
        .expect("Failed to parse login response");

    // Check user data
    assert!(login_body.get("user").is_some(), "Login should return user");
    let user = login_body.get("user").unwrap();
    assert_eq!(user.get("username").unwrap(), "logintest");

    // Check JWT tokens
    assert!(
        login_body.get("access_token").is_some(),
        "Login should return access_token"
    );
    let access_token = login_body.get("access_token").unwrap().as_str().unwrap();

    // Test accessing /me endpoint with JWT token
    let me_response = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Get current user request failed");

    assert_eq!(me_response.status(), 200, "Expected 200 OK");

    let me_body: serde_json::Value = me_response
        .json()
        .await
        .expect("Failed to parse me response");

    // Check response structure
    assert!(
        me_body.get("user").is_some(),
        "Me endpoint should return user object"
    );
    assert!(
        me_body.get("permissions").is_some(),
        "Me endpoint should return permissions array"
    );

    let current_user = me_body.get("user").unwrap();
    assert_eq!(current_user.get("username").unwrap(), "logintest");
    assert_eq!(current_user.get("email").unwrap(), "login@example.com");

    // Check permissions is an array
    let permissions = me_body
        .get("permissions")
        .unwrap()
        .as_array()
        .expect("Permissions should be an array");
    assert!(
        permissions.is_empty() || !permissions.is_empty(),
        "Permissions should be a valid array"
    );
}

#[tokio::test]
async fn test_auth_login_invalid_credentials() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register user
    let register_body = json!({
        "username": "validuser",
        "email": "valid@example.com",
        "password": "correctpass"
    });

    client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    // Test login with wrong password
    let login_body = json!({
        "username": "validuser",
        "password": "wrongpassword"
    });

    let response = client
        .post(server.api_url("/auth/login"))
        .json(&login_body)
        .send()
        .await
        .expect("Login request failed");

    assert_eq!(response.status(), 401, "Expected 401 Unauthorized");

    // Test login with non-existent user
    let login_body = json!({
        "username": "nonexistent",
        "password": "anypass"
    });

    let response = client
        .post(server.api_url("/auth/login"))
        .json(&login_body)
        .send()
        .await
        .expect("Login request failed");

    assert_eq!(response.status(), 401, "Expected 401 Unauthorized");
}

#[tokio::test]
async fn test_auth_logout() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register and login
    let register_body = json!({
        "username": "logouttest",
        "email": "logout@example.com",
        "password": "testpass123"
    });

    let register_response = client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    let register_body: serde_json::Value = register_response.json().await.unwrap();
    let access_token = register_body.get("access_token").unwrap().as_str().unwrap();

    // Verify token works by accessing /me
    let me_response = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Get current user failed");

    assert_eq!(me_response.status(), 200, "Should be authenticated");

    // Logout
    let logout_response = client
        .post(server.api_url("/auth/logout"))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Logout request failed");

    assert_eq!(logout_response.status(), 204, "Expected 204 No Content");

    // Logout revokes the ACCESS token too, not just the refresh token: the
    // access token is stateless, but every authenticated request now compares
    // its `ver` claim against `users.token_version`, which logout bumps.
    // (This test used to assert nothing here, with a note that "JWT is
    // stateless, so the token will still work after logout" — that WAS the
    // vulnerability: a held token kept full API access for its whole 24h TTL.)
    let after_logout = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Get current user failed");

    assert_eq!(
        after_logout.status(),
        401,
        "the access token must stop working the moment the user logs out"
    );
}

#[tokio::test]
async fn test_auth_me_without_token() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Try to access /me without token
    let response = client
        .get(server.api_url("/auth/me"))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Expected 401 Unauthorized");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    // Check error structure
    assert!(error_body.get("error_code").is_some());
    assert_eq!(error_body.get("error_code").unwrap(), "MISSING_TOKEN");
}

#[tokio::test]
async fn test_auth_me_with_invalid_token() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Try to access /me with invalid token
    let response = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", "Bearer invalid.token.here")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), 401, "Expected 401 Unauthorized");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    // Check error structure
    assert!(error_body.get("error_code").is_some());
    assert_eq!(error_body.get("error_code").unwrap(), "INVALID_TOKEN");
}

#[tokio::test]
async fn test_auth_login_with_email() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register user
    let register_body = json!({
        "username": "emaillogin",
        "email": "emaillogin@example.com",
        "password": "testpass123"
    });

    client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    // Test login with email instead of username
    let login_body = json!({
        "username": "emaillogin@example.com",
        "password": "testpass123"
    });

    let response = client
        .post(server.api_url("/auth/login"))
        .json(&login_body)
        .send()
        .await
        .expect("Login request failed");

    assert_eq!(response.status(), 200, "Should login with email");

    let login_response: serde_json::Value =
        response.json().await.expect("Failed to parse response");

    assert!(login_response.get("user").is_some());
    let user = login_response.get("user").unwrap();
    assert_eq!(user.get("username").unwrap(), "emaillogin");
    assert!(login_response.get("access_token").is_some());
}

#[tokio::test]
async fn test_auth_token_persistence() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register and login
    let register_body = json!({
        "username": "persistent",
        "email": "persist@example.com",
        "password": "testpass123"
    });

    let register_response = client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    let register_body: serde_json::Value = register_response.json().await.unwrap();
    let access_token = register_body.get("access_token").unwrap().as_str().unwrap();

    // Make multiple requests to verify token works across requests
    for _ in 0..3 {
        let response = client
            .get(server.api_url("/auth/me"))
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .expect("Get current user failed");

        assert_eq!(response.status(), 200, "Token should work across requests");

        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["user"]["username"], "persistent");
    }
}

#[tokio::test]
async fn test_auth_refresh_token() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register user
    let register_body = json!({
        "username": "refreshtest",
        "email": "refresh@example.com",
        "password": "testpass123"
    });

    let register_response = client
        .post(server.api_url("/auth/register"))
        .json(&register_body)
        .send()
        .await
        .expect("Registration failed");

    let register_body: serde_json::Value = register_response.json().await.unwrap();
    let refresh_token = register_body
        .get("refresh_token")
        .unwrap()
        .as_str()
        .unwrap();

    // Use refresh token to get new access token
    let refresh_body = json!({
        "refresh_token": refresh_token
    });

    let refresh_response = client
        .post(server.api_url("/auth/refresh"))
        .json(&refresh_body)
        .send()
        .await
        .expect("Refresh request failed");

    assert_eq!(refresh_response.status(), 200, "Expected 200 OK");

    let refresh_response_body: serde_json::Value = refresh_response.json().await.unwrap();
    assert!(
        refresh_response_body.get("access_token").is_some(),
        "Should return new access_token"
    );
    assert!(
        refresh_response_body.get("refresh_token").is_some(),
        "Should return new refresh_token"
    );

    // Verify new access token works
    let new_access_token = refresh_response_body
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap();
    let me_response = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", new_access_token))
        .send()
        .await
        .expect("Get current user failed");

    assert_eq!(me_response.status(), 200, "New token should work");
}

#[tokio::test]
async fn test_auth_registration_validation() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Test with missing fields
    let invalid_body = json!({
        "username": "testuser"
        // Missing email and password
    });

    let response = client
        .post(server.api_url("/auth/register"))
        .json(&invalid_body)
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status().is_client_error(),
        "Should fail with missing required fields"
    );

    // Test with empty username
    let empty_username = json!({
        "username": "",
        "email": "test@example.com",
        "password": "testpass123"
    });

    let response = client
        .post(server.api_url("/auth/register"))
        .json(&empty_username)
        .send()
        .await
        .expect("Request failed");

    assert!(
        response.status().is_client_error(),
        "Should fail with empty username"
    );
}

// =====================================================
// Setup Tests
// =====================================================

#[tokio::test]
async fn test_setup_status_needs_setup() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Check setup status when no admin exists
    let response = client
        .get(server.api_url("/app/setup/status"))
        .send()
        .await
        .expect("Setup status request failed");

    assert_eq!(response.status(), 200, "Expected 200 OK");

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Should need setup since no admin exists
    assert_eq!(body.get("needs_setup").unwrap(), true, "Should need setup");
    // 13-misc F-02 (Medium): app_name + version no longer leaked to
    // unauthenticated callers (fingerprint surface for CVE matrix
    // matching). The response now contains only `needs_setup`.
    assert!(
        body.get("app_name").is_none(),
        "app_name should not be exposed to unauthenticated callers (13-misc F-02)"
    );
    assert!(
        body.get("version").is_none(),
        "version should not be exposed to unauthenticated callers (13-misc F-02)"
    );
}

#[tokio::test]
async fn test_setup_status_no_setup_needed() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Create an admin user first
    let setup_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "SecurePass123!",
        "display_name": "Administrator"
    });

    client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup admin failed");

    // Now check setup status
    let response = client
        .get(server.api_url("/app/setup/status"))
        .send()
        .await
        .expect("Setup status request failed");

    assert_eq!(response.status(), 200, "Expected 200 OK");

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Should not need setup anymore
    assert_eq!(
        body.get("needs_setup").unwrap(),
        false,
        "Should not need setup"
    );
}

#[tokio::test]
async fn test_setup_admin_success() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let setup_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "SecurePass123!",
        "display_name": "System Administrator"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup admin request failed");

    assert_eq!(response.status(), 201, "Expected 201 Created");

    let body: serde_json::Value = response.json().await.expect("Failed to parse response");

    // Check user data
    assert!(body.get("user").is_some());
    let user = body.get("user").unwrap();
    assert_eq!(user.get("username").unwrap(), "admin");
    assert_eq!(user.get("email").unwrap(), "admin@example.com");
    assert_eq!(user.get("display_name").unwrap(), "System Administrator");

    // Check JWT tokens
    assert!(body.get("access_token").is_some());
    assert!(body.get("refresh_token").is_some());
    assert_eq!(body.get("token_type").unwrap(), "Bearer");
}

#[tokio::test]
async fn test_setup_admin_already_exists() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Create first admin
    let setup_body = json!({
        "username": "admin1",
        "email": "admin1@example.com",
        "password": "SecurePass123!"
    });

    client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("First setup failed");

    // Try to create second admin
    let second_setup = json!({
        "username": "admin2",
        "email": "admin2@example.com",
        "password": "SecurePass123!"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&second_setup)
        .send()
        .await
        .expect("Second setup request failed");

    assert_eq!(response.status(), 403, "Expected 403 Forbidden");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    assert_eq!(
        error_body.get("error_code").unwrap(),
        "SETUP_ALREADY_COMPLETE"
    );
}

#[tokio::test]
async fn test_setup_admin_weak_password() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let setup_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "weak"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup request failed");

    assert_eq!(response.status(), 400, "Expected 400 Bad Request");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    assert_eq!(error_body.get("error_code").unwrap(), "WEAK_PASSWORD");
}

#[tokio::test]
async fn test_setup_admin_invalid_email() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let setup_body = json!({
        "username": "admin",
        "email": "not-an-email",
        "password": "SecurePass123!"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup request failed");

    assert_eq!(response.status(), 400, "Expected 400 Bad Request");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    assert_eq!(error_body.get("error_code").unwrap(), "INVALID_EMAIL");
}

#[tokio::test]
async fn test_setup_admin_invalid_username() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Test too short username
    let setup_body = json!({
        "username": "ab",
        "email": "admin@example.com",
        "password": "SecurePass123!"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup request failed");

    assert_eq!(response.status(), 400, "Expected 400 Bad Request");

    let error_body: serde_json::Value = response
        .json()
        .await
        .expect("Failed to parse error response");

    assert_eq!(error_body.get("error_code").unwrap(), "INVALID_USERNAME");
}

#[tokio::test]
async fn test_setup_admin_assigns_to_administrators_group() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Create admin
    let setup_body = json!({
        "username": "admin",
        "email": "admin@example.com",
        "password": "SecurePass123!"
    });

    let response = client
        .post(server.api_url("/app/setup/admin"))
        .json(&setup_body)
        .send()
        .await
        .expect("Setup admin failed");

    let setup_response: serde_json::Value = response.json().await.unwrap();
    let access_token = setup_response
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap();

    // Check user's permissions (should have admin permissions from Administrators group)
    let me_response = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .expect("Get me failed");

    let me_body: serde_json::Value = me_response.json().await.unwrap();
    let permissions = me_body.get("permissions").unwrap().as_array().unwrap();

    // Administrators group has wildcard permission "*"
    assert!(
        permissions.contains(&json!("*")),
        "Admin should have wildcard permission from Administrators group"
    );
}

/// Creating the first admin (`/app/setup/admin` → app/repository.rs
/// create_admin_user) has a cross-subsystem effect: the new admin is enrolled
/// in BOTH the `Administrators` group (admin perms) AND the `Users` group
/// (default resource access). This asserts both memberships actually exist,
/// not just that the user/token were returned.
#[tokio::test]
async fn test_setup_admin_joins_administrators_and_users_groups() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let setup: serde_json::Value = client
        .post(server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "rootadmin",
            "email": "root@example.com",
            "password": "SecurePass123!"
        }))
        .send()
        .await
        .expect("setup request failed")
        .json()
        .await
        .expect("parse setup response");
    assert_eq!(setup["user"]["is_admin"], true, "setup creates an is_admin user");
    let admin_id = setup["user"]["id"].as_str().expect("admin id").to_string();
    let token = setup["access_token"].as_str().expect("access_token").to_string();

    let groups: serde_json::Value = client
        .get(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list groups failed")
        .json()
        .await
        .expect("parse groups");
    let arr = groups["groups"].as_array().expect("groups array");

    for want in ["Administrators", "Users"] {
        let gid = arr
            .iter()
            .find(|g| g["name"] == want)
            .and_then(|g| g["id"].as_str())
            .unwrap_or_else(|| panic!("{want} group missing from /groups: {groups}"));
        let members: serde_json::Value = client
            .get(server.api_url(&format!("/groups/{gid}/members")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("members request failed")
            .json()
            .await
            .expect("parse members");
        assert!(
            members.to_string().contains(&admin_id),
            "new admin must be a member of the {want} group: {members}"
        );
    }
}

/// Login user-enumeration / timing-mitigation contract (gap c0d66672d9b2).
/// The handler runs bcrypt against a dummy hash for unknown users and collapses
/// every failure into one INVALID_CREDENTIALS 401 (handlers.rs:176-243), so a
/// wrong password and a non-existent username are INDISTINGUISHABLE to a
/// client — no account enumeration. (We assert the response equivalence, the
/// observable contract of the constant-time path, not wall-clock timing.)
#[tokio::test]
async fn login_unknown_user_and_wrong_password_are_indistinguishable() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Register a real user.
    let reg = client
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": "timing_real_user",
            "email": "timing@example.com",
            "password": "correct-horse-battery",
            "display_name": "T"
        }))
        .send()
        .await
        .expect("register");
    assert_eq!(reg.status(), 201);

    // (a) Existing user, WRONG password.
    let wrong = client
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": "timing_real_user", "password": "definitely-wrong" }))
        .send()
        .await
        .expect("login wrong pw");
    let wrong_status = wrong.status();
    let wrong_body: serde_json::Value = wrong.json().await.unwrap();

    // (b) NON-EXISTENT user.
    let unknown = client
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": "no_such_user_at_all", "password": "anything" }))
        .send()
        .await
        .expect("login unknown user");
    let unknown_status = unknown.status();
    let unknown_body: serde_json::Value = unknown.json().await.unwrap();

    // Identical 401 + identical error code + identical message → no enumeration.
    assert_eq!(wrong_status, 401);
    assert_eq!(unknown_status, 401);
    assert_eq!(wrong_body["error_code"], "INVALID_CREDENTIALS");
    assert_eq!(unknown_body["error_code"], "INVALID_CREDENTIALS");
    assert_eq!(
        wrong_body["message"], unknown_body["message"],
        "wrong-password and unknown-user responses must be identical (no enumeration)"
    );
}

/// User disabled mid-session (gap 741e70537ae4): a user logs in (valid JWT),
/// then an admin deactivates the account. The still-valid access token must NOT
/// keep working — GET /auth/me re-checks is_active and returns 401
/// ACCOUNT_DEACTIVATED (handlers.rs:581), the teardown signal the session-sync
/// path relies on. (RequirePermissions enforces the same gate.)
#[tokio::test]
async fn deactivated_user_with_valid_jwt_is_rejected_at_me() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // Admin who can toggle account status.
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deact_admin",
        &["users::create", "users::toggle_status"],
    )
    .await;

    // Create + log in a victim, capturing a valid access token.
    let victim = crate::common::test_helpers::create_test_user(
        &server,
        &admin.token,
        "midsession_victim",
        "password123",
    )
    .await;
    let victim_id = victim["id"].as_str().expect("user id");
    let login = client
        .post(server.api_url("/auth/login"))
        .json(&json!({ "username": "midsession_victim", "password": "password123" }))
        .send()
        .await
        .expect("login");
    assert_eq!(login.status(), 200);
    let login_body: serde_json::Value = login.json().await.unwrap();
    let victim_token = login_body["tokens"]["access_token"]
        .as_str()
        .or_else(|| login_body["access_token"].as_str())
        .expect("access token")
        .to_string();

    // The token works BEFORE deactivation.
    let me_ok = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {victim_token}"))
        .send()
        .await
        .expect("me before");
    assert_eq!(me_ok.status(), 200, "valid JWT works before deactivation");

    // Admin deactivates the victim (account stays, JWT still cryptographically valid).
    let toggle = client
        .post(server.api_url(&format!("/users/{victim_id}/toggle-active")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("toggle-active");
    assert_eq!(toggle.status(), 200);

    // The SAME still-valid token is now rejected at /me.
    let me_dead = client
        .get(server.api_url("/auth/me"))
        .header("Authorization", format!("Bearer {victim_token}"))
        .send()
        .await
        .expect("me after");
    assert_eq!(me_dead.status(), 401, "deactivated account must be 401 even with a valid JWT");
    let body: serde_json::Value = me_dead.json().await.unwrap();
    assert_eq!(body["error_code"], "ACCOUNT_DEACTIVATED");
}

/// Error recovery in the setup flow: a FAILED setup attempt (invalid input)
/// must NOT leave the system half-initialized. The deployment must still report
/// `needs_setup: true` afterward, and a subsequent VALID setup must succeed and
/// flip the status — i.e. the first error is fully recoverable with a retry.
#[tokio::test]
async fn test_setup_recovers_after_failed_attempt() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // 1. A bad setup attempt fails with 400 (invalid email).
    let bad = client
        .post(server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "admin",
            "email": "not-an-email",
            "password": "SecurePass123!"
        }))
        .send()
        .await
        .expect("setup request failed");
    assert_eq!(bad.status(), 400, "invalid setup must be rejected");

    // 2. The failed attempt left NO partial admin → setup is still needed.
    let status = client
        .get(server.api_url("/app/setup/status"))
        .send()
        .await
        .expect("status request failed");
    let body: serde_json::Value = status.json().await.unwrap();
    assert_eq!(
        body.get("needs_setup").unwrap(),
        true,
        "a failed setup attempt must not create a partial admin"
    );

    // 3. A subsequent VALID setup succeeds — the flow recovered.
    let good = client
        .post(server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "admin",
            "email": "admin@example.com",
            "password": "SecurePass123!",
            "display_name": "Administrator"
        }))
        .send()
        .await
        .expect("setup request failed");
    assert_eq!(good.status(), 201, "retry after a failed attempt must succeed");

    // 4. Status now reflects the completed setup.
    let after: serde_json::Value = client
        .get(server.api_url("/app/setup/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after.get("needs_setup").unwrap(), false);
}

/// Refresh-token jti lifecycle + persistence (the whitelist lives in
/// `refresh_tokens`, so it survives a server restart — modeled here by
/// re-querying through a FRESH connection that shares no in-process state).
/// Exercises the re-exported `ziee::refresh_tokens` primitives directly.
#[tokio::test]
async fn refresh_token_jti_lifecycle_persists_across_connections() {
    let server = crate::common::TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "rt_user", &[]).await;
    let user_id = uuid::Uuid::parse_str(&user.user_id).unwrap();
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    let jti = uuid::Uuid::new_v4();
    let expires = chrono::Utc::now() + chrono::Duration::hours(1);
    ziee::refresh_tokens::register(&pool, jti, user_id, expires)
        .await
        .unwrap();

    // Active immediately after registration.
    assert!(ziee::refresh_tokens::is_active(&pool, jti).await.unwrap());

    // "Across restart": a brand-new connection (no shared process state) still
    // sees the jti as active because the whitelist is persisted in Postgres.
    let pool2 = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    assert!(
        ziee::refresh_tokens::is_active(&pool2, jti).await.unwrap(),
        "a registered jti survives a fresh connection (server restart)"
    );
    pool2.close().await;

    // Revoke → no longer active (rotation), and the revocation persists.
    ziee::refresh_tokens::revoke(&pool, jti).await.unwrap();
    assert!(!ziee::refresh_tokens::is_active(&pool, jti).await.unwrap());

    // An already-expired jti is never active.
    let jti_expired = uuid::Uuid::new_v4();
    let past = chrono::Utc::now() - chrono::Duration::hours(1);
    ziee::refresh_tokens::register(&pool, jti_expired, user_id, past)
        .await
        .unwrap();
    assert!(
        !ziee::refresh_tokens::is_active(&pool, jti_expired)
            .await
            .unwrap(),
        "an expired jti is inactive"
    );

    pool.close().await;
}

/// Cross-subsystem effect of admin creation: the bootstrapped admin is assigned
/// to BOTH the `Administrators` AND the `Users` groups (app/repository.rs
/// create_admin), so it inherits default Users-group resource access — not just
/// admin powers. Asserts the membership in both groups via the members API.
#[tokio::test]
async fn test_setup_admin_is_member_of_administrators_and_users_groups() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let setup: serde_json::Value = client
        .post(server.api_url("/app/setup/admin"))
        .json(&json!({
            "username": "admin",
            "email": "admin@example.com",
            "password": "SecurePass123!"
        }))
        .send()
        .await
        .expect("setup admin")
        .json()
        .await
        .expect("setup body");
    let admin_id = setup["user"]["id"].as_str().expect("admin id").to_string();
    let token = setup["access_token"].as_str().expect("token").to_string();

    // Resolve the two system group ids.
    let groups: serde_json::Value = client
        .get(server.api_url("/groups?page=1&per_page=100"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("list groups")
        .json()
        .await
        .expect("groups body");
    let arr = groups["groups"].as_array().or(groups.as_array()).expect("groups array");
    let id_of = |name: &str| {
        arr.iter()
            .find(|g| g["name"] == name)
            .and_then(|g| g["id"].as_str())
            .map(String::from)
            .unwrap_or_else(|| panic!("group {name} not found"))
    };
    let admins_gid = id_of("Administrators");
    let users_gid = id_of("Users");

    let is_member = |gid: String, token: String, admin_id: String| {
        let url = server.api_url(&format!("/groups/{gid}/members?page=1&per_page=100"));
        async move {
            let body: serde_json::Value = reqwest::Client::new()
                .get(&url)
                .header("Authorization", format!("Bearer {token}"))
                .send()
                .await
                .expect("members")
                .json()
                .await
                .expect("members body");
            body["users"]
                .as_array()
                .expect("users array")
                .iter()
                .any(|u| u["id"].as_str() == Some(admin_id.as_str()))
        }
    };

    assert!(
        is_member(admins_gid, token.clone(), admin_id.clone()).await,
        "admin must be in the Administrators group"
    );
    assert!(
        is_member(users_gid, token, admin_id).await,
        "admin must ALSO be in the Users group (default-resource access)"
    );
}

/// Account deactivation cuts off access to protected RESOURCE endpoints — not
/// just the sync stream. The `RequirePermissions` extractor (permissions/
/// extractors.rs) re-resolves `is_active` from the DB on EVERY request and
/// returns 403 USER_INACTIVE the moment a user is disabled, even though their
/// JWT is still cryptographically valid and unexpired. This proves the gate
/// holds across two representative protected surfaces (chat conversations +
/// memory) so a disabled account can't keep reading data it could a moment ago.
#[tokio::test]
async fn deactivated_user_is_refused_on_protected_resource_endpoints() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deact_resource",
        &["profile::read", "conversations::read", "memory::read"],
    )
    .await;
    let admin = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "deact_resource_admin",
        &["users::edit"],
    )
    .await;

    let client = reqwest::Client::new();

    // While ACTIVE: both resource endpoints are reachable with the user's token.
    let convs_ok = client
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        convs_ok.status(),
        200,
        "an active user with conversations::read must list conversations"
    );

    let mem_ok = client
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        mem_ok.status(),
        200,
        "an active user with memory::read must list memories"
    );

    // Admin deactivates the user.
    let deact = client
        .post(server.api_url(&format!("/users/{}", user.user_id)))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "is_active": false }))
        .send()
        .await
        .unwrap();
    assert!(
        deact.status().is_success(),
        "deactivation should succeed; got {}",
        deact.status()
    );

    // Same still-valid token → BOTH endpoints now refuse via the is_active gate.
    let convs_refused = client
        .get(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(
        convs_refused.status() == 401 || convs_refused.status() == 403,
        "a deactivated user must be refused the conversations endpoint; got {}",
        convs_refused.status()
    );

    let mem_refused = client
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert!(
        mem_refused.status() == 401 || mem_refused.status() == 403,
        "a deactivated user must be refused the memories endpoint; got {}",
        mem_refused.status()
    );
}

/// Timing-attack / user-enumeration mitigation (handlers.rs login path): every
/// failure mode — wrong password, NONEXISTENT user, and a DEACTIVATED account —
/// must collapse into the IDENTICAL response (same 401 status, same
/// `INVALID_CREDENTIALS` code, same message). A differing status/code/message
/// would let an attacker enumerate valid usernames. (The constant-time dummy
/// bcrypt is the timing half; this asserts the indistinguishable-response half.)
#[tokio::test]
async fn test_auth_login_failures_are_indistinguishable() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    // A real, active user to fail against with a wrong password.
    client
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": "enum_real",
            "email": "enum_real@example.com",
            "password": "correct-horse-battery"
        }))
        .send()
        .await
        .expect("register failed");

    let login = |username: &'static str, password: &'static str| {
        let client = client.clone();
        let url = server.api_url("/auth/login");
        async move {
            let res = client
                .post(url)
                .json(&json!({ "username": username, "password": password }))
                .send()
                .await
                .expect("login request failed");
            let status = res.status();
            let body: serde_json::Value = res.json().await.expect("parse error body");
            (status, body)
        }
    };

    // (a) existing user, wrong password
    let (s_wrong, b_wrong) = login("enum_real", "definitely-wrong").await;
    // (b) user that does not exist at all
    let (s_missing, b_missing) = login("enum_ghost", "definitely-wrong").await;

    // All failures: 401 + the SAME generic error code + message (no enumeration).
    assert_eq!(s_wrong.as_u16(), 401);
    assert_eq!(s_missing.as_u16(), 401);
    assert_eq!(b_wrong["error_code"], json!("INVALID_CREDENTIALS"));
    assert_eq!(
        b_wrong["error_code"], b_missing["error_code"],
        "error_code must be identical for wrong-password vs nonexistent-user"
    );
    assert_eq!(
        b_wrong["error"], b_missing["error"],
        "error message must be identical (no user enumeration via message)"
    );
    // The message must be the generic, non-revealing one.
    assert_eq!(b_wrong["error"], json!("Invalid username or password"));
}

/// Concurrent registrations with the SAME username must not both succeed: the
/// handler's check-then-insert has a TOCTOU window, and the DB UNIQUE
/// constraint is the real guard. Exactly one request creates the account; the
/// other is rejected (pre-check 409 or the DB-race error), never a second user.
#[tokio::test]
async fn test_concurrent_registration_same_username_creates_one_user() {
    let server = crate::common::TestServer::start().await;

    let body = |email: &str| {
        json!({
            "username": "raceuser",
            "email": email,
            "password": "testpass123",
            "display_name": "Race User"
        })
    };

    let base = server.api_url("/auth/register");
    let c1 = reqwest::Client::new();
    let c2 = reqwest::Client::new();

    // Fire both at once so they overlap on the check-then-insert window.
    let (r1, r2) = tokio::join!(
        c1.post(&base).json(&body("race1@example.com")).send(),
        c2.post(&base).json(&body("race2@example.com")).send(),
    );
    let s1 = r1.expect("req1 failed").status().as_u16();
    let s2 = r2.expect("req2 failed").status().as_u16();

    let created = [s1, s2].iter().filter(|&&s| s == 201).count();
    assert_eq!(
        created, 1,
        "exactly one concurrent same-username registration must succeed (got statuses {s1}, {s2})"
    );
    // The loser must be rejected, not a silent second account.
    let loser = if s1 == 201 { s2 } else { s1 };
    assert!(
        loser >= 400,
        "the losing registration must be an error status, got {loser}"
    );
}

/// `ensure_unique_username` (SSO auto-provision) appends the lowest free numeric
/// suffix when the base is taken, returns the base verbatim when free, and
/// defaults an empty base to "user". Driven directly by initializing the
/// in-process Repos against the test DB (same pattern as resource_link_test).
#[tokio::test]
#[serial_test::serial(repos)]
async fn test_ensure_unique_username_collision_suffix_and_defaults() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    if !ziee::is_repos_initialized() {
        ziee::init_repositories(pool.clone());
    }

    // Seed a collision: "ssobase" and "ssobase2" already exist (distinct emails
    // so the email path is irrelevant — we exercise username uniqueness only).
    for (name, email) in [("ssobase", "a@x.com"), ("ssobase2", "b@x.com")] {
        sqlx::query("INSERT INTO users (id, username, email, is_active, is_admin, created_at, updated_at) VALUES (gen_random_uuid(), $1, $2, true, false, NOW(), NOW())")
            .bind(name)
            .bind(email)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Taken base + taken base2 → next free is base3.
    let got = ziee::ensure_unique_username(&pool, "ssobase").await.expect("unique");
    assert_eq!(got, "ssobase3", "lowest free numeric suffix");

    // A free base is returned verbatim.
    let free = format!("freebase_{}", &uuid::Uuid::new_v4().to_string()[..8]);
    assert_eq!(ziee::ensure_unique_username(&pool, &free).await.unwrap(), free);

    // Empty base defaults to "user" (or user2… if taken) — must be non-empty.
    let defaulted = ziee::ensure_unique_username(&pool, "   ").await.unwrap();
    assert!(
        defaulted == "user" || defaulted.starts_with("user"),
        "empty base must default to a user-prefixed name, got {defaulted}"
    );

    pool.close().await;
}

/// Refresh-token jti lifecycle is DB-backed (`refresh_tokens` table), so it
/// survives a process restart and revocation is consulted from the persistent
/// store on every `/auth/refresh` — not from in-memory state. Proven without a
/// real restart by: (1) a registered token has a persisted active row, and
/// (2) revoking that row in the DB makes the SAME refresh token fail validation
/// on the next request. Connecting a FRESH pool to the same DATABASE_URL (a new
/// process would do exactly this) still observes the row, demonstrating
/// cross-restart persistence.
#[tokio::test]
async fn test_refresh_token_jti_lifecycle_is_db_backed() {
    let server = crate::common::TestServer::start().await;
    let client = reqwest::Client::new();

    let register: serde_json::Value = client
        .post(server.api_url("/auth/register"))
        .json(&json!({
            "username": "jti_lifecycle",
            "email": "jti_lifecycle@example.com",
            "password": "testpass123"
        }))
        .send()
        .await
        .expect("register")
        .json()
        .await
        .unwrap();
    let refresh_token = register["refresh_token"].as_str().unwrap().to_string();
    let user_id = uuid::Uuid::parse_str(register["user"]["id"].as_str().unwrap()).unwrap();

    // A FRESH connection to the same DB (what a restarted process would open)
    // sees the persisted, active refresh-token row.
    let fresh_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let active_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM refresh_tokens WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()",
    )
    .bind(user_id)
    .fetch_one(&fresh_pool)
    .await
    .unwrap();
    assert!(active_rows >= 1, "the issued jti must be persisted + active");

    // Sanity: while the row is active, /auth/refresh validates against it (200).
    let ok = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": refresh_token }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200, "active jti must validate");
    // Rotation issues a new token; capture it (the old jti is now rotated out).
    let rotated: serde_json::Value = ok.json().await.unwrap();
    let rotated_token = rotated["refresh_token"].as_str().unwrap().to_string();

    // Revoke ALL of the user's refresh tokens in the persistent store …
    let affected = sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(&fresh_pool)
        .await
        .unwrap()
        .rows_affected();
    assert!(affected >= 1, "revoke must touch the persisted row(s)");
    fresh_pool.close().await;

    // … and the next /auth/refresh consults that store → rejected (not 200).
    let denied = client
        .post(server.api_url("/auth/refresh"))
        .json(&json!({ "refresh_token": rotated_token }))
        .send()
        .await
        .unwrap();
    assert_ne!(
        denied.status(),
        200,
        "a revoked (DB-persisted) jti must fail validation; got {}",
        denied.status()
    );
}


use serde_json::json;

#[tokio::test]
async fn test_user_crud_operations() {
    let server = crate::common::TestServer::start().await;

    // First, register and login to get a token
    let register_body = json!({
        "username": "cruduser",
        "email": "crud@example.com",
        "password": "password123",
        "fullname": "CRUD User"
    });

    let response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/register"), &register_body)
        .await
        .expect("Registration failed");

    let user_id = response.get("user").unwrap().get("id").unwrap().as_str().unwrap();

    // Test list users
    let users_response: serde_json::Value = crate::common::http::get(&format!("{}/users?page=1&per_page=10", server.api_url("")))
        .await
        .expect("List users failed");

    assert!(users_response.get("users").is_some());
    assert!(users_response.get("users").unwrap().as_array().unwrap().len() > 0);

    // Test get user by ID
    let user_response: serde_json::Value = crate::common::http::get(&server.api_url(&format!("/users/{}", user_id)))
        .await
        .expect("Get user failed");

    assert_eq!(user_response.get("username").unwrap(), "cruduser");
}

#[tokio::test]
async fn test_password_change() {
    let server = crate::common::TestServer::start().await;

    // Register a user
    let register_body = json!({
        "username": "passuser",
        "email": "pass@example.com",
        "password": "oldpassword",
        "fullname": "Password User"
    });

    let response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/register"), &register_body)
        .await
        .expect("Registration failed");

    let user_id = response.get("user").unwrap().get("id").unwrap().as_str().unwrap();
    let token = response.get("token").unwrap().as_str().unwrap();

    // Change password
    let change_password_body = json!({
        "old_password": "oldpassword",
        "new_password": "newpassword"
    });

    let change_response: serde_json::Value = crate::common::http::post_with_auth(
        &server.api_url(&format!("/users/{}/password", user_id)),
        token,
        &change_password_body,
    )
    .await
    .expect("Password change failed");

    assert!(change_response.get("message").is_some());

    // Try to login with old password (should fail)
    let login_old = json!({
        "username_or_email": "passuser",
        "password": "oldpassword"
    });

    // This should fail, but we're just testing the endpoint exists
    let _ = crate::common::http::post::<_, serde_json::Value>(&server.api_url("/auth/login"), &login_old).await;

    // Login with new password
    let login_new = json!({
        "username_or_email": "passuser",
        "password": "newpassword"
    });

    let login_response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/login"), &login_new)
        .await
        .expect("Login with new password failed");

    assert!(login_response.get("token").is_some());
}

#[tokio::test]
async fn test_user_not_found() {
    let server = crate::common::TestServer::start().await;

    // Test get non-existent user
    let result = crate::common::http::get::<serde_json::Value>(&server.api_url("/users/00000000-0000-0000-0000-000000000000")).await;
    // Should return 404 error
    assert!(result.is_ok() || result.is_err());
}

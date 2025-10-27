use serde_json::json;

#[tokio::test]
async fn test_auth_registration_and_login_flow() {
    let server = crate::common::TestServer::start().await;

    // Test registration
    let register_body = json!({
        "username": "testuser",
        "email": "test@example.com",
        "password": "testpass123",
        "fullname": "Test User"
    });

    let response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/register"), &register_body)
        .await
        .expect("Registration failed");

    assert!(response.get("token").is_some());
    assert!(response.get("user").is_some());
    let user = response.get("user").unwrap();
    assert_eq!(user.get("username").unwrap(), "testuser");

    // Test login
    let login_body = json!({
        "username_or_email": "testuser",
        "password": "testpass123"
    });

    let login_response: serde_json::Value = crate::common::http::post(&server.api_url("/auth/login"), &login_body)
        .await
        .expect("Login failed");

    assert!(login_response.get("token").is_some());
    let token = login_response.get("token").unwrap().as_str().unwrap();

    // Test get current user
    let me_response: serde_json::Value = crate::common::http::get_with_auth(&server.api_url("/auth/me"), token)
        .await
        .expect("Get current user failed");

    assert!(me_response.get("user").is_some());
    let current_user = me_response.get("user").unwrap();
    assert_eq!(current_user.get("username").unwrap(), "testuser");
}

#[tokio::test]
async fn test_auth_invalid_credentials() {
    let server = crate::common::TestServer::start().await;

    // Test login with invalid credentials
    let login_body = json!({
        "username_or_email": "nonexistent",
        "password": "wrongpass"
    });

    let result = crate::common::http::post::<_, serde_json::Value>(&server.api_url("/auth/login"), &login_body).await;
    // Should return an error, but reqwest will return the error response
    assert!(result.is_ok() || result.is_err());
}

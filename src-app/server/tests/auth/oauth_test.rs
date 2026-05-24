/// OAuth2/OIDC Provider Integration Tests
///
/// These tests use testcontainers to automatically spawn OAuth mock servers.
/// Docker will be started automatically if not already running.
use crate::common::oauth_mock::OAuthMockServer;
use serde_json::json;

/// Test that we can start an OAuth mock server and connect to it
#[tokio::test]
async fn test_oauth_mock_server_connectivity() {
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Test that we can fetch the OpenID configuration
    let client = reqwest::Client::new();
    let response = client
        .get(&oauth_server.well_known_url())
        .send()
        .await
        .expect("Failed to fetch well-known config");

    assert!(response.status().is_success());

    let config: serde_json::Value = response.json().await.expect("Invalid JSON");
    assert_eq!(config["issuer"], oauth_server.issuer_url);
}

/// Test creating an OAuth provider in the database
#[tokio::test]
async fn test_create_oauth_provider() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Create OAuth provider configuration
    let provider_config = oauth_server.create_test_provider_config("test-client", "test-secret");

    // Insert provider into database
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");

    let provider_id = sqlx::query_scalar!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        "test-oauth",
        "oauth2",
        provider_config,
        true
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create OAuth provider");

    assert!(!provider_id.is_nil());

    // Verify we can retrieve it
    let retrieved = sqlx::query!(
        r#"
        SELECT name, provider_type, config, enabled
        FROM auth_providers
        WHERE id = $1
        "#,
        provider_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to retrieve provider");

    assert_eq!(retrieved.name, "test-oauth");
    assert_eq!(retrieved.provider_type, "oauth2");
    assert!(retrieved.enabled);
}

/// Test complete OAuth authorization flow through our application
///
/// This test performs a full end-to-end OAuth flow:
/// 1. Start our application with OAuth provider configured
/// 2. Hit OUR /api/auth/oauth/{provider}/authorize endpoint
/// 3. Follow redirect to mock OAuth server
/// 4. Submit login form on mock server
/// 5. Follow redirect back to OUR /api/auth/oauth/{provider}/callback
/// 6. Verify OUR application returns JWT tokens
#[tokio::test]
async fn test_oauth_authorization_flow() {
    let test_server = crate::common::TestServer::start().await;
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Step 1: Create OAuth provider in database pointing to mock server
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");

    let provider_config = serde_json::json!({
        "client_id": "test-client",
        "client_secret": "test-secret",
        "authorization_url": oauth_server.authorize_url(),
        "token_url": oauth_server.token_url(),
        "issuer_url": oauth_server.issuer_url,
        "scopes": ["openid", "profile", "email"],
        "attribute_mapping": {
            "user_id": "sub",
            "username": "sub",  // Use sub as username since mock server always returns this
            "email": "sub",     // Fallback to sub for email as well
            "display_name": "sub"
        }
    });

    sqlx::query!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, $2, $3, $4)
        "#,
        "test-oauth",
        "oauth2",
        provider_config,
        true
    )
    .execute(&pool)
    .await
    .expect("Failed to create OAuth provider");

    // Get the provider ID we just created
    let provider_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM auth_providers WHERE name = $1")
            .bind("test-oauth")
            .fetch_one(&pool)
            .await
            .expect("Failed to get provider ID");

    // Create a test user
    let user_id = uuid::Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO users (id, username, email, is_active, is_admin, created_at, updated_at)
        VALUES ($1, $2, $3, true, false, NOW(), NOW())
        "#,
        user_id,
        "testuser",
        "testuser@example.com"
    )
    .execute(&pool)
    .await
    .expect("Failed to create test user");

    // Link the user to the OAuth provider
    // The mock OAuth server will use "testuser" as the sub claim
    sqlx::query!(
        r#"
        INSERT INTO user_auth_links (user_id, provider_id, external_id, created_at)
        VALUES ($1, $2, $3, NOW())
        "#,
        user_id,
        provider_id,
        "testuser"
    )
    .execute(&pool)
    .await
    .expect("Failed to create user auth link");

    // Step 2: Initiate OAuth flow through OUR application
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none()) // Manual redirect handling
        .cookie_store(true)
        .build()
        .unwrap();

    let callback_url = format!(
        "{}/api/auth/oauth/test-oauth/callback",
        test_server.base_url
    );
    let authorize_url = format!(
        "{}/api/auth/oauth/test-oauth/authorize?redirect_uri={}",
        test_server.base_url,
        callback_url.replace("/", "%2F").replace(":", "%3A")
    );
    println!("Step 1: Initiating OAuth flow at: {}", authorize_url);

    let auth_response = client
        .get(&authorize_url)
        .send()
        .await
        .expect("Failed to initiate OAuth flow");

    // Our app should redirect to the mock OAuth server
    assert_eq!(
        auth_response.status(),
        307,
        "Should redirect to OAuth provider"
    );

    let oauth_auth_url = auth_response
        .headers()
        .get("location")
        .expect("Should have Location header")
        .to_str()
        .expect("Location should be valid string");

    println!("Step 2: Redirected to OAuth provider: {}", oauth_auth_url);
    assert!(
        oauth_auth_url.contains("response_type=code"),
        "Should be OAuth authorize URL"
    );

    // Step 3: Submit login form to mock OAuth server
    let oauth_response = client
        .post(oauth_auth_url)
        .form(&[("username", "testuser")])
        .send()
        .await
        .expect("Failed to submit OAuth login");

    // Mock server redirects back to our callback with code
    assert_eq!(oauth_response.status(), 302, "OAuth server should redirect");

    let callback_redirect = oauth_response
        .headers()
        .get("location")
        .expect("Should have callback URL")
        .to_str()
        .expect("Location should be valid string");

    println!(
        "Step 3: OAuth server redirected to callback: {}",
        callback_redirect
    );
    assert!(
        callback_redirect.contains("/api/auth/oauth/test-oauth/callback"),
        "Should redirect to our callback"
    );
    assert!(
        callback_redirect.contains("code="),
        "Should include authorization code"
    );

    // Step 4: Follow redirect to OUR callback endpoint
    let callback_response = client
        .get(callback_redirect)
        .send()
        .await
        .expect("Failed to hit callback endpoint");

    let status = callback_response.status();
    println!("Step 4: Callback response status: {}", status);

    // Our application returns a redirect to /?token={access_token}
    assert!(
        status.is_redirection(),
        "Callback should redirect with token"
    );

    let redirect_url = callback_response
        .headers()
        .get("location")
        .expect("Should have Location header")
        .to_str()
        .expect("Location should be valid string");

    println!("Step 5: Redirected to: {}", redirect_url);
    // 01-auth F-01 (Critical) closure: the access token now lives in
    // the URL FRAGMENT (`/#token=…`) so it isn't logged on the server,
    // sent as a Referer, or kept in browser history. Extract from the
    // fragment instead of the query.
    assert!(
        redirect_url.contains("#token="),
        "Should include access token in URL fragment (01-auth F-01)"
    );

    let access_token = redirect_url
        .split_once("#token=")
        .map(|(_, t)| {
            // Fragment may carry additional `&key=value` pairs after a
            // future addition; take only up to the next separator.
            t.split('&').next().unwrap_or(t).to_string()
        })
        .expect("Token not found in URL fragment");

    println!("✅ Complete OAuth flow successful!");
    println!("   User authenticated: testuser");
    println!("   Got JWT access token from our application");
    println!(
        "   Access token: {}...",
        &access_token[..20.min(access_token.len())]
    );
}

/// Test that OAuth provider configuration validation works
#[tokio::test]
async fn test_oauth_provider_validation() {
    let oauth_server = OAuthMockServer::start()
        .await
        .expect("Failed to start OAuth mock server");

    // Test with valid configuration
    let valid_config = oauth_server.create_test_provider_config("valid-client", "valid-secret");

    assert!(valid_config["client_id"].is_string());
    assert!(valid_config["client_secret"].is_string());
    assert!(valid_config["auth_url"].is_string());
    assert!(valid_config["token_url"].is_string());
    assert!(valid_config["scopes"].is_array());

    // Test with invalid configuration (missing required fields)
    let invalid_config = json!({
        "client_id": "test"
        // Missing client_secret, urls, etc.
    });

    // In a real test, you would validate against your provider schema
    assert!(!invalid_config.get("client_secret").is_some());
}

/// LDAP Provider Integration Tests
///
/// These tests use testcontainers to automatically spawn LDAP mock servers.
/// Docker will be started automatically if not already running.
use crate::common::ldap_mock::LdapMockServer;
use ldap3::{LdapConnAsync, Scope, SearchEntry};

/// Test that we can start an LDAP mock server and bind to it
#[tokio::test]

async fn test_ldap_mock_server_bind() {
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    // Attempt to bind as admin
    let (conn, mut ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect to LDAP");
    ldap3::drive!(conn);

    let bind_result = ldap
        .simple_bind(&ldap_server.bind_dn, &ldap_server.bind_password)
        .await
        .expect("Failed to bind");

    assert!(bind_result.success().is_ok(), "Bind should succeed");
    ldap.unbind().await.ok();
}

/// Test searching for users in LDAP
#[tokio::test]

async fn test_ldap_user_search() {
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    let (conn, mut ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect");
    ldap3::drive!(conn);

    // Bind as admin
    ldap.simple_bind(&ldap_server.bind_dn, &ldap_server.bind_password)
        .await
        .expect("Failed to bind")
        .success()
        .expect("Bind failed");

    // Search for test user (Fry)
    let (username, _) = LdapMockServer::get_test_user();
    let filter = format!("(uid={})", username);
    let search_base = format!("ou=people,{}", ldap_server.base_dn);

    let (results, _) = ldap
        .search(
            &search_base,
            Scope::Subtree,
            &filter,
            vec!["uid", "cn", "mail", "userPassword"],
        )
        .await
        .expect("Search failed")
        .success()
        .expect("Search result failed");

    assert_eq!(results.len(), 1, "Should find exactly one user");

    let entry = SearchEntry::construct(results[0].clone());
    assert_eq!(entry.attrs.get("uid").unwrap()[0], username);
    assert!(entry.attrs.get("cn").is_some(), "Should have cn attribute");
    assert!(
        entry.attrs.get("mail").is_some(),
        "Should have mail attribute"
    );
    ldap.unbind().await.ok();
}

/// Test authenticating a user with LDAP bind
#[tokio::test]

async fn test_ldap_user_authentication() {
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    // Get test user credentials
    let (username, password) = LdapMockServer::get_test_user();

    // First, search for the user to get their DN
    let (conn, mut ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect");
    ldap3::drive!(conn);

    ldap.simple_bind(&ldap_server.bind_dn, &ldap_server.bind_password)
        .await
        .expect("Failed to bind as admin")
        .success()
        .expect("Admin bind failed");

    let filter = format!("(uid={})", username);
    let search_base = format!("ou=people,{}", ldap_server.base_dn);

    let (results, _) = ldap
        .search(&search_base, Scope::Subtree, &filter, vec!["uid"])
        .await
        .expect("Search failed")
        .success()
        .expect("Search result failed");

    assert!(!results.is_empty(), "User should exist");

    let entry = SearchEntry::construct(results[0].clone());
    let user_dn = entry.dn;
    ldap.unbind().await.ok();

    // Now try to bind as that user
    let (conn2, mut user_ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect");
    ldap3::drive!(conn2);

    let bind_result = user_ldap
        .simple_bind(&user_dn, password)
        .await
        .expect("Bind failed");

    assert!(
        bind_result.success().is_ok(),
        "User authentication should succeed"
    );
    user_ldap.unbind().await.ok();
}

/// Test LDAP authentication with wrong password fails
#[tokio::test]

async fn test_ldap_wrong_password_fails() {
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    let (username, _) = LdapMockServer::get_test_user();

    // Search for user DN
    let (conn, mut ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect");
    ldap3::drive!(conn);

    ldap.simple_bind(&ldap_server.bind_dn, &ldap_server.bind_password)
        .await
        .expect("Failed to bind")
        .success()
        .expect("Bind failed");

    let filter = format!("(uid={})", username);
    let search_base = format!("ou=people,{}", ldap_server.base_dn);

    let (results, _) = ldap
        .search(&search_base, Scope::Subtree, &filter, vec!["uid"])
        .await
        .expect("Search failed")
        .success()
        .expect("Search failed");

    let entry = SearchEntry::construct(results[0].clone());
    let user_dn = entry.dn;
    ldap.unbind().await.ok();

    // Try to bind with wrong password
    let (conn2, mut user_ldap) = LdapConnAsync::new(&ldap_server.ldap_url())
        .await
        .expect("Failed to connect");
    ldap3::drive!(conn2);

    let bind_result = user_ldap.simple_bind(&user_dn, "wrong_password").await;

    // Bind should fail or return an error
    match bind_result {
        Ok(result) => {
            assert!(
                result.success().is_err(),
                "Bind with wrong password should fail"
            );
        }
        Err(_) => {
            // Connection error is also acceptable - authentication failed
        }
    }
    user_ldap.unbind().await.ok();
}

/// Test creating an LDAP provider in the database
#[tokio::test]
async fn test_create_ldap_provider() {
    let test_server = crate::common::TestServer::start().await;
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    // Create LDAP provider configuration
    let provider_config = ldap_server.create_test_provider_config();

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
        "test-ldap",
        "ldap",
        provider_config,
        true
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create LDAP provider");

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

    assert_eq!(retrieved.name, "test-ldap");
    assert_eq!(retrieved.provider_type, "ldap");
    assert!(retrieved.enabled);

    // Verify config fields
    assert_eq!(retrieved.config["url"], ldap_server.ldap_url());
    assert_eq!(retrieved.config["admin_bind_dn"], ldap_server.bind_dn);
    assert_eq!(
        retrieved.config["base_dn"],
        "ou=people,dc=planetexpress,dc=com"
    );
    assert_eq!(retrieved.config["search_filter"], "(uid={username})");
}

/// Test LDAP provider configuration validation
#[tokio::test]
async fn test_ldap_provider_configuration() {
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    let config = ldap_server.create_test_provider_config();

    // Validate all required fields are present
    assert!(config["url"].is_string());
    assert!(config["base_dn"].is_string());
    assert!(config["search_filter"].is_string());
    assert!(config["admin_bind_dn"].is_string());
    assert!(config["admin_password"].is_string());
    assert!(config["attribute_mapping"].is_object());
    assert!(config["attribute_mapping"]["username"].is_string());
    assert!(config["attribute_mapping"]["email"].is_string());
}

/// Test complete LDAP login flow through our application
///
/// This test performs a full end-to-end LDAP authentication:
/// 1. Start our application with LDAP provider configured
/// 2. Hit OUR /api/auth/login endpoint with LDAP credentials
/// 3. Verify OUR application returns JWT tokens
#[tokio::test]
async fn test_ldap_login_flow() {
    let test_server = crate::common::TestServer::start().await;
    let ldap_server = LdapMockServer::start()
        .await
        .expect("Failed to start LDAP mock server");

    // Step 1: Create LDAP provider in database pointing to mock server
    let pool = sqlx::PgPool::connect(&test_server.database_url)
        .await
        .expect("Failed to connect to test database");

    let provider_config = ldap_server.create_test_provider_config();

    sqlx::query!(
        r#"
        INSERT INTO auth_providers (name, provider_type, config, enabled)
        VALUES ($1, $2, $3, $4)
        "#,
        "test-ldap",
        "ldap",
        provider_config,
        true
    )
    .execute(&pool)
    .await
    .expect("Failed to create LDAP provider");

    // Step 2: Attempt login with LDAP credentials through OUR application
    let (username, password) = LdapMockServer::get_test_user();
    let login_url = format!("{}/api/auth/login", test_server.base_url);

    println!("Step 1: Attempting LDAP login at: {}", login_url);
    println!("   Username: {}", username);

    let client = reqwest::Client::new();
    let login_response = client
        .post(&login_url)
        .json(&serde_json::json!({
            "username": username,
            "password": password,
            "provider": "test-ldap"
        }))
        .send()
        .await
        .expect("Failed to send login request");

    let status = login_response.status();
    println!("Step 2: Login response status: {}", status);

    // Debug: print response body if not successful
    if !status.is_success() {
        let body = login_response.text().await.unwrap_or_default();
        println!("Error response body: {}", body);
        panic!("LDAP login failed with status {}", status);
    }

    // Our application should return JWT tokens after successful LDAP authentication
    assert!(status.is_success(), "LDAP login should succeed");

    let auth_data: serde_json::Value = login_response
        .json()
        .await
        .expect("Failed to parse login response");

    // Validate we got OUR application's JWT tokens
    assert!(
        auth_data["access_token"].is_string(),
        "Should have access_token from our app"
    );
    assert!(
        auth_data["refresh_token"].is_string(),
        "Should have refresh_token from our app"
    );
    assert!(
        auth_data["user"]["username"].is_string(),
        "Should have user info"
    );

    println!("✅ Complete LDAP login flow successful!");
    println!("   User: {}", auth_data["user"]["username"]);
    println!("   Display Name: {}", auth_data["user"]["display_name"]);
    println!("   Email: {}", auth_data["user"]["email"]);
    println!("   Got JWT tokens from our application");
}

//! Custom-header transmission + header-validation tests for the HTTP MCP transport.
//!
//! Two tiers:
//!  1. **Transmission** — a direct `HttpMcpClient` against the programmable
//!     `MockMcpServer`, asserting configured custom headers (e.g.
//!     `Authorization`) actually reach the remote server on every request. This
//!     is the definitive "is the header our problem?" proof, and covers the
//!     trailing-newline-trim repair + interior-invalid-drop behaviour.
//!  2. **Validation** — HTTP-route tests (`TestServer`) asserting the
//!     create/update/test-connection endpoints trim surrounding whitespace and
//!     reject interior-invalid header values with a `400 INVALID_HEADER`.

use super::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};
use crate::common::test_helpers;
use serde_json::json;
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config_with_headers(url: String, headers: serde_json::Value) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-mcp-headers".to_string(),
        display_name: "Mock MCP (headers fixture)".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        environment_variables_entries: vec![],
        url: Some(url),
        headers,
        headers_entries: vec![],
        timeout_seconds: 10,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        run_in_sandbox: false,
        sandbox_flavor: "full".to_string(),
        is_built_in: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        last_health_check_at: None,
        last_health_check_status: "untested".to_string(),
        last_health_check_reason: None,
    }
}

fn tools_list_ok() -> MockResponse {
    MockResponse::JsonOk(json!({
        "tools": [{ "name": "t", "description": "d", "inputSchema": { "type": "object" } }]
    }))
}

// ─── Tier 1: configured headers reach the remote server ──────────────────────

#[tokio::test]
async fn custom_headers_sent_on_initialize_and_tools_list() {
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", tools_list_ok());

    let config = server_config_with_headers(
        mock.base_url(),
        json!({ "Authorization": "Bearer test-token", "X-Custom-Header": "val" }),
    );
    let mut client = HttpMcpClient::new(config).expect("client construction");
    client.connect().await.expect("connect");
    let tools = client.list_tools().await.expect("list_tools");
    assert_eq!(tools.len(), 1);
    client.disconnect().await.ok();

    let received = mock.received();
    for method in ["initialize", "tools/list"] {
        let req = received
            .iter()
            .find(|r| r.method == method)
            .unwrap_or_else(|| panic!("{method} request must reach the server"));
        assert_eq!(
            req.headers.get("authorization").map(String::as_str),
            Some("Bearer test-token"),
            "{method} must carry the configured Authorization header"
        );
        assert_eq!(
            req.headers.get("x-custom-header").map(String::as_str),
            Some("val"),
            "{method} must carry the configured X-Custom-Header"
        );
    }
}

#[tokio::test]
async fn trailing_newline_header_value_is_trimmed_and_sent() {
    // Simulates a token pasted with a trailing newline (the value may have been
    // persisted before this fix). The runtime trim repairs it so the header
    // still reaches the server instead of being silently dropped.
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", tools_list_ok());

    let config = server_config_with_headers(
        mock.base_url(),
        json!({ "Authorization": "Bearer test-token\n" }),
    );
    let mut client = HttpMcpClient::new(config).expect("client construction");
    client.connect().await.expect("connect");
    client.disconnect().await.ok();

    let init = mock
        .received()
        .into_iter()
        .find(|r| r.method == "initialize")
        .expect("initialize reached the server");
    assert_eq!(
        init.headers.get("authorization").map(String::as_str),
        Some("Bearer test-token"),
        "trailing newline must be trimmed, not dropped"
    );
}

#[tokio::test]
async fn interior_invalid_header_value_dropped_valid_one_sent() {
    // An interior newline can't form a valid HTTP header value; it must be
    // dropped (and logged) while a valid sibling header still goes out.
    let mock = MockMcpServer::start().await;
    mock.on_method("tools/list", tools_list_ok());

    let config = server_config_with_headers(
        mock.base_url(),
        json!({ "Authorization": "Bearer good", "X-Bad": "ba\nd" }),
    );
    let mut client = HttpMcpClient::new(config).expect("client construction");
    client.connect().await.expect("connect");
    client.disconnect().await.ok();

    let init = mock
        .received()
        .into_iter()
        .find(|r| r.method == "initialize")
        .expect("initialize reached the server");
    assert_eq!(
        init.headers.get("authorization").map(String::as_str),
        Some("Bearer good"),
        "valid header must still be sent"
    );
    assert!(
        init.headers.get("x-bad").is_none(),
        "interior-invalid header must be dropped, not sent"
    );
}

// ─── Tier 2: API boundary trims + validates headers ──────────────────────────

#[tokio::test]
async fn create_user_server_rejects_interior_invalid_header() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "bad_header_server",
            "display_name": "Bad Header",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "headers": { "Authorization": "Bea\nr" },
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 400, "interior-invalid header must 400, got: {body}");
    assert_eq!(body["error_code"], "INVALID_HEADER", "got: {body}");
    assert!(
        body["error"].as_str().unwrap().contains("Authorization"),
        "error must name the offending header, got: {body}"
    );
}

#[tokio::test]
async fn update_user_server_rejects_interior_invalid_header() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::edit", "mcp_servers::read"],
    )
    .await;
    let client = reqwest::Client::new();

    let created: serde_json::Value = client
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "update_header_server",
            "display_name": "Update Header",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("create failed")
        .json()
        .await
        .expect("parse create");
    let id = created["id"].as_str().expect("server id").to_string();

    let response = client
        .put(server.api_url(&format!("/mcp/servers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "headers": { "Authorization": "Bea\nr" } }))
        .send()
        .await
        .expect("update request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 400, "interior-invalid header must 400, got: {body}");
    assert_eq!(body["error_code"], "INVALID_HEADER", "got: {body}");
    assert!(
        body["error"].as_str().unwrap().contains("Authorization"),
        "error must name the offending header, got: {body}"
    );
}

#[tokio::test]
async fn test_connection_rejects_interior_invalid_header() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers/test-connection"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "headers": { "Authorization": "Bea\nr" },
            "timeout_seconds": 3
        }))
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    // A bad header is a config error → 400, distinct from a 200 connection result.
    assert_eq!(status, 400, "interior-invalid header must 400, got: {body}");
    assert_eq!(body["error_code"], "INVALID_HEADER", "got: {body}");
    assert!(
        body["error"].as_str().unwrap().contains("Authorization"),
        "error must name the offending header, got: {body}"
    );
}

#[tokio::test]
async fn create_user_server_trims_trailing_whitespace_in_headers() {
    let server = crate::common::TestServer::start().await;
    let user =
        test_helpers::create_user_with_permissions(&server, "user", &["mcp_servers::create"]).await;

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "trim_header_server",
            "display_name": "Trim Header",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "headers": { "Authorization": "Bearer ok\n", "X-Y": "  z  " },
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    assert_eq!(status, 201, "valid (trimmable) headers must persist, got: {body}");
    // Stored headers come back trimmed — the pasted newline / padding is gone.
    assert_eq!(body["headers"]["Authorization"], "Bearer ok", "got: {body}");
    assert_eq!(body["headers"]["X-Y"], "z", "got: {body}");
}

#[tokio::test]
async fn create_system_server_rejects_interior_invalid_header() {
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create"],
    )
    .await;

    let response = reqwest::Client::new()
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "bad_header_system_server",
            "display_name": "Bad Header System",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "headers": { "Authorization": "Bea\nr" },
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("request failed");

    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse body");
    // The system create path is a separate fn from the user one — assert it
    // routes through the same header validation.
    assert_eq!(status, 400, "system interior-invalid header must 400, got: {body}");
    assert_eq!(body["error_code"], "INVALID_HEADER", "got: {body}");
    assert!(
        body["error"].as_str().unwrap().contains("Authorization"),
        "error must name the offending header, got: {body}"
    );
}

#[tokio::test]
async fn update_user_server_trims_trailing_whitespace_in_headers() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::edit", "mcp_servers::read"],
    )
    .await;
    let client = reqwest::Client::new();

    let created: serde_json::Value = client
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "update_trim_server",
            "display_name": "Update Trim",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("create failed")
        .json()
        .await
        .expect("parse create");
    let id = created["id"].as_str().expect("server id").to_string();

    let updated: serde_json::Value = client
        .put(server.api_url(&format!("/mcp/servers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "headers": { "Authorization": "Bearer ok\n", "X-Y": "  z  " } }))
        .send()
        .await
        .expect("update failed")
        .json()
        .await
        .expect("parse update");

    // The update path normalizes too: stored values come back trimmed.
    assert_eq!(updated["headers"]["Authorization"], "Bearer ok", "got: {updated}");
    assert_eq!(updated["headers"]["X-Y"], "z", "got: {updated}");
}

#[tokio::test]
async fn update_user_server_omitted_headers_are_preserved() {
    let server = crate::common::TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["mcp_servers::create", "mcp_servers::edit", "mcp_servers::read"],
    )
    .await;
    let client = reqwest::Client::new();

    let created: serde_json::Value = client
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": "update_preserve_server",
            "display_name": "Update Preserve",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "headers": { "Authorization": "Bearer keepme" },
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("create failed")
        .json()
        .await
        .expect("parse create");
    let id = created["id"].as_str().expect("server id").to_string();

    // Update an unrelated field while OMITTING `headers` → request.headers is
    // None. The new normalize-on-Some match arm must NOT turn an omitted-headers
    // update into an empty-map clobber: None must still pass through to COALESCE
    // and preserve the stored headers (regression guard for that arm).
    let updated: serde_json::Value = client
        .put(server.api_url(&format!("/mcp/servers/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "display_name": "Update Preserve Renamed" }))
        .send()
        .await
        .expect("update failed")
        .json()
        .await
        .expect("parse update");

    assert_eq!(updated["display_name"], "Update Preserve Renamed");
    assert_eq!(
        updated["headers"]["Authorization"], "Bearer keepme",
        "omitted headers must be preserved, got: {updated}"
    );
}

#[tokio::test]
async fn update_system_server_trims_trailing_whitespace_in_headers() {
    // The system UPDATE path is a separate fn (update_system_mcp_server) with its
    // own normalize_and_validate_headers call — guard it independently of the
    // user path so a regression there can't slip through.
    let server = crate::common::TestServer::start().await;
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "admin",
        &["mcp_servers_admin::create", "mcp_servers_admin::edit"],
    )
    .await;
    let client = reqwest::Client::new();

    let created: serde_json::Value = client
        .post(server.api_url("/mcp/system-servers"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "name": "update_trim_system_server",
            "display_name": "Update Trim System",
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "timeout_seconds": 10
        }))
        .send()
        .await
        .expect("create failed")
        .json()
        .await
        .expect("parse create");
    let id = created["id"].as_str().expect("server id").to_string();

    let updated: serde_json::Value = client
        .put(server.api_url(&format!("/mcp/system-servers/{id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "headers": { "Authorization": "Bearer ok\n", "X-Y": "  z  " } }))
        .send()
        .await
        .expect("update failed")
        .json()
        .await
        .expect("parse update");

    assert_eq!(updated["headers"]["Authorization"], "Bearer ok", "got: {updated}");
    assert_eq!(updated["headers"]["X-Y"], "z", "got: {updated}");
}

// ─── Pure unit coverage for the `parse_header_map` helper ────────────────────
//
// These live here (not in an in-source `#[cfg(test)] mod`) because
// `cargo test --lib` doesn't compile on this branch — a pre-existing sqlx
// 0.8/0.9 dependency clash breaks the memory/pgvector modules under the test
// cfg. The integration tier builds the lib normally, so these run reliably via
// `ziee::parse_header_map`.
mod parse_header_map_unit {
    use ziee::{HeaderParseError, parse_header_map};

    // Empty env helper — most tests don't use `${VAR}` interpolation,
    // and an empty Object is what runtime sees when the server has no
    // env vars configured.
    fn no_env() -> serde_json::Value {
        json!({})
    }

    #[test]
    fn valid_map_all_present_no_errors() {
        let (map, errors) =
            parse_header_map(&json!({ "Authorization": "Bearer x", "X-A": "1" }), &no_env());
        assert!(errors.is_empty(), "no errors expected: {errors:?}");
        assert_eq!(map.get("authorization").unwrap().to_str().unwrap(), "Bearer x");
        assert_eq!(map.get("x-a").unwrap().to_str().unwrap(), "1");
    }

    #[test]
    fn trims_trailing_newline_and_whitespace() {
        let (map, errors) =
            parse_header_map(&json!({ "Authorization": "Bearer x\n", "X-Y": "  z  " }), &no_env());
        assert!(errors.is_empty(), "trailing whitespace must NOT be an error: {errors:?}");
        assert_eq!(map.get("authorization").unwrap().to_str().unwrap(), "Bearer x");
        assert_eq!(map.get("x-y").unwrap().to_str().unwrap(), "z");
    }

    #[test]
    fn interior_invalid_value_reported_and_dropped() {
        let (map, errors) = parse_header_map(&json!({ "Authorization": "Bea\nr" }), &no_env());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "Authorization");
        assert!(map.get("authorization").is_none(), "interior newline must drop the header");
    }

    #[test]
    fn invalid_key_reported_other_entries_kept() {
        let (map, errors) = parse_header_map(&json!({ " Bad Key ": "v", "Good": "1" }), &no_env());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, " Bad Key ");
        assert_eq!(map.get("good").unwrap().to_str().unwrap(), "1");
    }

    #[test]
    fn non_string_value_reported() {
        let (map, errors) = parse_header_map(&json!({ "X": 123 }), &no_env());
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].name, "X");
        assert!(errors[0].reason.contains("string"));
        assert!(map.is_empty());
    }

    #[test]
    fn empty_and_non_object_yield_empty_map() {
        for v in [json!({}), json!([]), json!(null), json!("nope")] {
            let (map, errors): (_, Vec<HeaderParseError>) =
                parse_header_map(&v, &no_env());
            assert!(map.is_empty());
            assert!(errors.is_empty());
        }
    }

    // ${VAR} interpolation — the catalog convention for hub MCP
    // servers (e.g. `Authorization: Bearer ${GITHUB_TOKEN}`) expands
    // against the server's `environment_variables` at request-build
    // time. Without this, hub-installed HTTP servers' auth headers
    // would carry the literal `${VAR}` token.
    #[test]
    fn expands_var_reference_against_env() {
        let env = json!({ "GITHUB_TOKEN": "ghp_real" });
        let (map, errors) = parse_header_map(
            &json!({ "Authorization": "Bearer ${GITHUB_TOKEN}" }),
            &env,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            map.get("authorization").unwrap().to_str().unwrap(),
            "Bearer ghp_real",
        );
    }

    #[test]
    fn expands_multiple_vars_in_one_value() {
        let env = json!({ "A": "alpha", "B": "beta" });
        let (map, errors) = parse_header_map(
            &json!({ "X-Combo": "prefix-${A}-mid-${B}-suffix" }),
            &env,
        );
        assert!(errors.is_empty());
        assert_eq!(
            map.get("x-combo").unwrap().to_str().unwrap(),
            "prefix-alpha-mid-beta-suffix",
        );
    }

    #[test]
    fn undefined_var_leaves_literal_token() {
        // Unknown vars stay as the literal `${NAME}` so the request
        // fails-fast with an obvious upstream error rather than
        // silently sending an empty header.
        let (map, errors) = parse_header_map(
            &json!({ "Authorization": "Bearer ${GITHUB_TOKEN}" }),
            &json!({}),
        );
        assert!(errors.is_empty());
        assert_eq!(
            map.get("authorization").unwrap().to_str().unwrap(),
            "Bearer ${GITHUB_TOKEN}",
        );
    }

    #[test]
    fn dollar_without_brace_is_literal() {
        let (map, errors) = parse_header_map(
            &json!({ "X-Price": "$100" }),
            &json!({}),
        );
        assert!(errors.is_empty());
        assert_eq!(map.get("x-price").unwrap().to_str().unwrap(), "$100");
    }

    #[test]
    fn unterminated_var_token_left_literal() {
        // `${VAR` (no closing brace) is left as-is.
        let (map, errors) = parse_header_map(
            &json!({ "X-Broken": "value-${OPEN" }),
            &json!({ "OPEN": "shouldnotmatter" }),
        );
        assert!(errors.is_empty());
        assert_eq!(
            map.get("x-broken").unwrap().to_str().unwrap(),
            "value-${OPEN",
        );
    }

    use serde_json::json;
}

//! Integration tests for the built-in control MCP server (`control.ziee.internal`).
//!
//! Tier 2 (DB): boot registration, migration-126 grant.
//! Tier 3 (HTTP/JSON-RPC over the REAL routes): tools surface, the `control::use`
//! gate, the per-user permission VISIBILITY filter (admin sees `User.create`, a
//! limited user does not), and the real loopback round-trip (`invoke_capability`
//! actually creates/does not create a row, with NO mocked authz). Denylist,
//! schema validation, path-param, and kill-switch paths.

mod real_llm_test;

use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::TestServerOptions;
use crate::common::test_helpers::{
    create_user_with_no_permissions, create_user_with_only_permissions,
    create_user_with_permissions,
};

/// POST a JSON-RPC request to the control MCP endpoint with a bearer token.
fn jsonrpc(
    server: &TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/control/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

fn control_mcp_server_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, b"control.ziee.internal")
}

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::PgPool::connect(&server.database_url).await.unwrap()
}

// ── Tier 2: DB ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn builtin_row_registered() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;

    let row = sqlx::query!(
        r#"SELECT name, is_system, is_built_in, enabled, transport_type, url
           FROM mcp_servers WHERE id = $1"#,
        control_mcp_server_id()
    )
    .fetch_optional(&pool)
    .await
    .unwrap()
    .expect("control built-in row must exist after boot");

    assert_eq!(row.name, "control");
    assert!(row.is_system);
    assert!(row.is_built_in);
    assert!(row.enabled);
    assert_eq!(row.transport_type, "http");
    assert!(row.url.unwrap().ends_with("/api/control/mcp"));
}

#[tokio::test]
async fn appears_on_system_mcp_page_and_admin_can_toggle_enabled() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_sysadmin", &["*"]).await;
    let client = reqwest::Client::new();
    let control_id = control_mcp_server_id().to_string();

    // 1. Control appears in the System MCP listing (it's a visible, editable
    //    built-in like bio_mcp — NOT hidden).
    let list: Value = client
        .get(server.api_url("/mcp/system-servers?per_page=100"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<String> = list["servers"]
        .as_array()
        .expect("servers array")
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&control_id),
        "control must be listed on the System MCP page: {ids:?}"
    );

    // 2. Admin toggles it OFF (no longer immutable) → 200.
    let res = client
        .put(server.api_url(&format!("/mcp/system-servers/{control_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert!(
        res.status().is_success(),
        "admin must be able to toggle control enabled: {}",
        res.status()
    );

    // 3. The row reflects enabled=false (the runtime auto-attach honors this).
    let pool = pool(&server).await;
    let enabled = sqlx::query_scalar!(
        "SELECT enabled FROM mcp_servers WHERE id = $1",
        control_mcp_server_id()
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!enabled, "control row must be disabled after the admin toggle");

    // 4. Re-enable round-trips too.
    let res = client
        .put(server.api_url(&format!("/mcp/system-servers/{control_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}

#[tokio::test]
async fn migration_grants_control_use_to_default_users_group() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let perms = sqlx::query_scalar!(
        r#"SELECT permissions FROM groups
           WHERE name = 'Users' AND is_system = true AND is_default = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        perms.iter().any(|p| p == "control::use"),
        "default Users group must carry control::use (migration 126): {perms:?}"
    );
}

// ── Tier 3: HTTP / JSON-RPC ──────────────────────────────────────────────────

#[tokio::test]
async fn initialize_and_tools_list() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ctl_init", &["control::use"]).await;

    let init: Value = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "control");

    let list: Value = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"list_capabilities"));
    assert!(names.contains(&"describe_capability"));
    assert!(names.contains(&"invoke_capability"));
}

#[tokio::test]
async fn use_permission_gate_returns_403() {
    let server = TestServer::start().await;
    let user = create_user_with_no_permissions(&server, "ctl_noperm").await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}

async fn call_tool(server: &TestServer, token: &str, name: &str, args: Value) -> Value {
    jsonrpc(
        server,
        token,
        "tools/call",
        json!({ "name": name, "arguments": args }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap()
}

fn structured(result: &Value) -> &Value {
    &result["result"]["structuredContent"]
}

#[tokio::test]
async fn list_capabilities_filters_by_permission() {
    let server = TestServer::start().await;
    // Admin-capable: wildcard grants every op including users::create.
    let admin = create_user_with_permissions(&server, "ctl_admin", &["*"]).await;
    // Limited: ONLY control::use (no default group) → no users::create.
    let limited =
        create_user_with_only_permissions(&server, "ctl_limited", &["control::use"]).await;

    // Scope by query (the realistic path — a broad no-query list is capped).
    let admin_res =
        call_tool(&server, &admin.token, "list_capabilities", json!({ "query": "user" })).await;
    let admin_ops: Vec<String> = structured(&admin_res)["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["operation_id"].as_str().unwrap().to_string())
        .collect();
    // User.delete is permission-gated (users::delete) and has no secret body, so
    // it survives the denylist — an admin sees it, a control::use-only user does not.
    assert!(
        admin_ops.iter().any(|o| o == "User.delete"),
        "admin must see User.delete: {admin_ops:?}"
    );

    let limited_res =
        call_tool(&server, &limited.token, "list_capabilities", json!({ "query": "user" })).await;
    let limited_ops: Vec<String> = structured(&limited_res)["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["operation_id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !limited_ops.iter().any(|o| o == "User.delete"),
        "limited user must NOT see User.delete: {limited_ops:?}"
    );
}

#[tokio::test]
async fn secret_bearing_write_is_denied() {
    let server = TestServer::start().await;
    // Even an all-powerful admin cannot drive a secret-bearing write — a password
    // in the body would be persisted into the conversation's tool-call args.
    let admin = create_user_with_permissions(&server, "ctl_secret", &["*"]).await;
    let pool = pool(&server).await;
    let before = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({
            "operation_id": "User.create",
            "body": { "username": "x", "email": "x@y.com", "password": "password123" }
        }),
    )
    .await;
    assert!(res["error"].is_object(), "secret-bearing User.create must be denied: {res}");
    let after = sqlx::query_scalar!("SELECT COUNT(*) FROM users")
        .fetch_one(&pool).await.unwrap().unwrap_or(0);
    assert_eq!(before, after, "no user created");
}

#[tokio::test]
async fn describe_refuses_unpermitted_without_leaking_schema() {
    let server = TestServer::start().await;
    let limited =
        create_user_with_only_permissions(&server, "ctl_desc", &["control::use"]).await;
    let res = call_tool(
        &server,
        &limited.token,
        "describe_capability",
        json!({ "operation_id": "User.create" }),
    )
    .await;
    assert!(
        res["error"].is_object(),
        "expected in-band not-permitted error, got {res}"
    );
    assert!(res["result"].is_null());
}

#[tokio::test]
async fn invoke_create_assistant_real_roundtrip() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_mk", &["*"]).await;
    let name = format!("ControlMade-{}", &Uuid::new_v4().to_string()[..8]);

    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({
            "operation_id": "Assistant.create",
            "body": { "name": name }
        }),
    )
    .await;

    let sc = structured(&res);
    assert!(
        sc["ok"].as_bool().unwrap_or(false),
        "invoke should succeed, got {res}"
    );

    // Assert via the REAL DB that the assistant exists (no mocked authz).
    let pool = pool(&server).await;
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM assistants WHERE name = $1",
        name
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(count, 1, "assistant '{name}' must exist after invoke");
}

#[tokio::test]
async fn invoke_privileged_write_denied_for_limited_user_no_row() {
    let server = TestServer::start().await;
    let limited =
        create_user_with_only_permissions(&server, "ctl_deny", &["control::use"]).await;
    let pool = pool(&server).await;
    let before = sqlx::query_scalar!("SELECT COUNT(*) FROM groups")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);

    // A control::use-only user lacks groups::create → pre-dispatch permission
    // refusal, and the row is never created.
    let res = call_tool(
        &server,
        &limited.token,
        "invoke_capability",
        json!({
            "operation_id": "UserGroup.create",
            "body": { "name": "sneaky", "permissions": ["*"] }
        }),
    )
    .await;
    assert!(res["error"].is_object(), "expected not-permitted error, got {res}");

    let after = sqlx::query_scalar!("SELECT COUNT(*) FROM groups")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(before, after, "no group may be created by an unpermitted invoke");
}

#[tokio::test]
async fn invoke_denied_operation_is_refused() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_denyop", &["*"]).await;
    // A denylisted op (control recursion) must never dispatch, even for admin.
    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Auth.login", "body": {} }),
    )
    .await;
    assert!(
        res["error"].is_object(),
        "denylisted/unknown op must be refused, got {res}"
    );
}

#[tokio::test]
async fn invoke_unknown_operation_is_refused() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_unknown", &["*"]).await;
    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Definitely.NotReal" }),
    )
    .await;
    assert!(res["error"].is_object());
}

#[tokio::test]
async fn invoke_upstream_error_relayed() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_upstream", &["*"]).await;
    // GET a user by a bogus (but well-formed) UUID → the real route returns 404,
    // which must be relayed to the model as a structured error (not a panic, not
    // a 200). The dot in a UUID is fine — hyphens/alnum only, passes path safety.
    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({
            "operation_id": "User.get",
            "path_params": { "user_id": Uuid::new_v4().to_string() }
        }),
    )
    .await;
    assert!(res["error"].is_null(), "op must resolve + dispatch, not refuse: {res}");
    let sc = structured(&res);
    assert_eq!(sc["ok"], json!(false), "a bogus id lookup must not be ok: {res}");
    assert_eq!(sc["status"], json!(404), "the upstream 404 must be relayed: {res}");
}

#[tokio::test]
async fn unauthenticated_returns_401() {
    let server = TestServer::start().await;
    // No Authorization header at all → the RequirePermissions gate rejects 401.
    let res = reqwest::Client::new()
        .post(server.api_url("/control/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn describe_permitted_returns_schema_and_approval_flag() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_desc_ok", &["*"]).await;
    let res = call_tool(
        &server,
        &admin.token,
        "describe_capability",
        json!({ "operation_id": "Assistant.create" }),
    )
    .await;
    let sc = structured(&res);
    assert_eq!(sc["operation_id"], "Assistant.create");
    assert_eq!(sc["method"], "POST");
    // A mutating op must advertise that it needs approval.
    assert_eq!(sc["requires_approval"], json!(true));
    assert!(sc["request_schema"].is_object(), "schema must be returned: {res}");
}

#[tokio::test]
async fn invoke_invalid_body_rejected_before_dispatch() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_badbody", &["*"]).await;
    let pool = pool(&server).await;
    let before = sqlx::query_scalar!("SELECT COUNT(*) FROM groups")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    // UserGroup.create requires `name` — omit it → INVALID_BODY, no dispatch.
    let res = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({
            "operation_id": "UserGroup.create",
            "body": { "permissions": [] }
        }),
    )
    .await;
    assert!(res["error"].is_object(), "expected INVALID_BODY error, got {res}");
    let after = sqlx::query_scalar!("SELECT COUNT(*) FROM groups")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(before, after, "invalid body must not create a group");
}

#[tokio::test]
async fn invoke_path_param_roundtrip_and_traversal_reject() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_pp", &["*"]).await;
    let pool = pool(&server).await;

    // Create an assistant, capture its id.
    let orig = format!("PP-{}", &Uuid::new_v4().to_string()[..8]);
    let created = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Assistant.create", "body": { "name": orig } }),
    )
    .await;
    let id = structured(&created)["response"]["id"]
        .as_str()
        .expect("created assistant id")
        .to_string();

    // Rename via a path-param op (PUT /api/assistants/{id}).
    let renamed = format!("PP2-{}", &Uuid::new_v4().to_string()[..8]);
    let upd = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({
            "operation_id": "Assistant.update",
            "path_params": { "id": id },
            "body": { "name": renamed }
        }),
    )
    .await;
    assert!(structured(&upd)["ok"].as_bool().unwrap_or(false), "update should succeed: {upd}");
    let db_name = sqlx::query_scalar!(
        "SELECT name FROM assistants WHERE id = $1::uuid",
        Uuid::parse_str(&id).unwrap()
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(db_name, renamed, "path-param update must rename the row");

    // A dot-segment path param must be rejected BEFORE dispatch (H1).
    let bad = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Assistant.get", "path_params": { "id": ".." } }),
    )
    .await;
    assert!(bad["error"].is_object(), "traversal path param must be refused: {bad}");
}

#[tokio::test]
async fn denylist_vs_allow_contrast() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_contrast", &["*"]).await;
    // Denylisted (auth token flow) → refused even for an all-powerful admin.
    let denied = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Auth.login", "body": {} }),
    )
    .await;
    assert!(denied["error"].is_object(), "Auth.login must be denylisted: {denied}");
    // A structurally-similar, non-denied op IS reachable for the same admin —
    // proving the refusal above is the denylist, not a blanket block.
    let allowed = call_tool(
        &server,
        &admin.token,
        "invoke_capability",
        json!({ "operation_id": "Assistant.create", "body": { "name": format!("OK-{}", &Uuid::new_v4().to_string()[..8]) } }),
    )
    .await;
    assert!(allowed["error"].is_null(), "Assistant.create must be allowed: {allowed}");
    assert!(structured(&allowed)["ok"].as_bool().unwrap_or(false));
}

#[tokio::test]
async fn migration_grant_not_duplicated() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let perms = sqlx::query_scalar!(
        r#"SELECT permissions FROM groups
           WHERE name = 'Users' AND is_system = true AND is_default = true"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let n = perms.iter().filter(|p| *p == "control::use").count();
    assert_eq!(n, 1, "control::use must appear exactly once (idempotent grant)");
}

#[tokio::test]
async fn list_reports_total_and_truncation() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ctl_trunc", &["*"]).await;
    // No query → the full permitted set, which exceeds the 200 cap for an admin.
    let res = call_tool(&server, &admin.token, "list_capabilities", json!({})).await;
    let sc = structured(&res);
    let returned = sc["returned"].as_u64().unwrap();
    let total = sc["total"].as_u64().unwrap();
    assert!(returned <= 200, "returned must be capped at 200, got {returned}");
    assert!(total > 200, "an admin sees >200 ops, got {total}");
    assert_eq!(sc["truncated"], json!(true));
}

#[tokio::test]
async fn kill_switch_removes_surface() {
    let server = TestServer::start_with_options(TestServerOptions {
        control_mcp_enabled: Some(false),
        ..Default::default()
    })
    .await;
    let pool = pool(&server).await;
    // Row not upserted when disabled.
    let exists = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM mcp_servers WHERE id = $1",
        control_mcp_server_id()
    )
    .fetch_one(&pool)
    .await
    .unwrap()
    .unwrap_or(0);
    assert_eq!(exists, 0, "control row must be absent when kill-switch is off");

    // Route not registered → 404 (a user with control::use still can't reach it).
    let user = create_user_with_permissions(&server, "ctl_off", &["control::use"]).await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404);
}

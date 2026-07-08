//! Tier 2 — office_bridge MCP endpoint + admin settings REST.
//!
//! - TEST-2 (MCP): `POST /api/office-bridge/mcp` JSON-RPC `initialize` returns
//!   `serverInfo` + `protocolVersion`; `tools/list` returns the 7 office tool
//!   descriptors; both gated (401 without auth, 403 without `office_bridge::use`).
//! - TEST-3 (settings / migrations): `GET /api/office-bridge/settings` returns
//!   the singleton defaults (enabled, port 44300), proving migration 132 applied;
//!   a default-Users member passes the `office_bridge::use` MCP gate, proving
//!   migration 133 granted the perm to the Users group.
//! - TEST-5 (settings authz): `GET`/`PUT /api/office-bridge/settings` are 403
//!   without the admin perms, and the response body never carries a bridge
//!   token / secret.

use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_no_permissions;
use crate::common::test_helpers::create_user_with_permissions;
use crate::office_bridge::jsonrpc;

/// The admin read+manage perms. Held by the Administrators group's `*` wildcard
/// in production; here we mint a custom group carrying exactly these two.
fn admin_perms() -> &'static [&'static str] {
    &["office_bridge::admin::read", "office_bridge::admin::manage"]
}

/// The seven `office` tool descriptors `tools/list` must advertise (ITEM-9).
const EXPECTED_TOOLS: &[&str] = &[
    "list_open_documents",
    "read_document",
    "edit_document",
    "add_comment",
    "set_track_changes",
    "get_tracked_changes",
    "get_selection",
];

// ─────────────────────────────── TEST-2 (MCP) ───────────────────────────────

/// TEST-2 — `initialize` returns the office_bridge serverInfo + protocolVersion.
#[tokio::test]
async fn test2_initialize_returns_server_info_and_protocol_version() {
    let server = TestServer::start_desktop().await;
    let user = create_user_with_permissions(&server, "ob_init", &["office_bridge::use"]).await;
    let res = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "office_bridge");
    assert!(
        body["result"]["serverInfo"]["version"].is_string(),
        "serverInfo.version present: {body}"
    );
    assert!(
        body["result"]["protocolVersion"].is_string(),
        "protocolVersion present: {body}"
    );
    assert_eq!(body["result"]["protocolVersion"], "2025-11-25");
}

/// TEST-2 — `tools/list` advertises all seven office tool descriptors.
#[tokio::test]
async fn test2_tools_list_returns_the_seven_office_tools() {
    let server = TestServer::start_desktop().await;
    let user = create_user_with_permissions(&server, "ob_list", &["office_bridge::use"]).await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in EXPECTED_TOOLS {
        assert!(names.contains(expected), "tools/list missing `{expected}`: {names:?}");
    }
    assert_eq!(names.len(), 7, "exactly 7 office tools expected: {names:?}");
    // Each descriptor carries an inputSchema (the MCP tool contract).
    for t in body["result"]["tools"].as_array().unwrap() {
        assert!(
            t["inputSchema"]["type"] == "object",
            "tool `{}` needs an object inputSchema: {t}",
            t["name"]
        );
    }
}

/// TEST-2 (gate) — no Authorization header at all → 401 (the JWT extractor
/// rejects before the permission check).
#[tokio::test]
async fn test2_mcp_without_auth_is_401() {
    let server = TestServer::start_desktop().await;
    let res = reqwest::Client::new()
        .post(server.api_url("/office-bridge/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401, "missing bearer must be 401");
}

/// TEST-2 (gate) — a valid token WITHOUT `office_bridge::use` → 403.
#[tokio::test]
async fn test2_mcp_without_use_permission_is_403() {
    let server = TestServer::start_desktop().await;
    // Stripped from all groups → no office_bridge::use.
    let user = create_user_with_no_permissions(&server, "ob_noperm").await;
    for method in ["initialize", "tools/list"] {
        let res = jsonrpc(&server, &user.token, method, json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 403, "`{method}` must be gated by office_bridge::use");
    }
}

// ───────────────────────── TEST-3 (settings / migrations) ────────────────────

/// TEST-3 — `GET /api/office-bridge/settings` returns the singleton defaults
/// seeded by migration 132: enabled = true, port = 44300.
#[tokio::test]
async fn test3_get_settings_returns_singleton_defaults() {
    let server = TestServer::start_desktop().await;
    let admin = create_user_with_permissions(&server, "ob_get_admin", admin_perms()).await;
    let res = reqwest::Client::new()
        .get(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], true, "default enabled: {row}");
    assert_eq!(row["port"], 44300, "default port: {row}");
    // The singleton was seeded (migration 132's INSERT), so last_connected_at is
    // null until a pane connects; cert_fingerprint is null until the cert mints.
    assert!(row["last_connected_at"].is_null(), "never-connected: {row}");
}

/// TEST-3 — a user whose ONLY source of `office_bridge::use` is default-Users
/// membership (migration 133) must pass the MCP `use` gate. `initialize` needs
/// only the perm (no settings/DB tool path), so a 200 here proves migration 133
/// granted the perm to the Users group; a 403 would mean the grant is missing.
#[tokio::test]
async fn test3_default_users_group_grants_office_bridge_use() {
    let server = TestServer::start_desktop().await;
    // Empty perm list → registered + auto-joined to the default Users group,
    // with NO custom-group perms. Its only office_bridge::use is migration 133's.
    let user = create_user_with_permissions(&server, "ob_default_only", &[]).await;
    let res = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        200,
        "default-Users member must pass the office_bridge::use gate (migration 133)"
    );
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "office_bridge");
}

// ─────────────────────────── TEST-5 (settings authz) ─────────────────────────

/// TEST-5 — `GET /api/office-bridge/settings` is 403 without
/// `office_bridge::admin::read`. A default-Users member holds `office_bridge::use`
/// but NOT the admin read perm, so it must be gated out of the admin surface.
#[tokio::test]
async fn test5_get_settings_without_admin_read_is_403() {
    let server = TestServer::start_desktop().await;
    let user = create_user_with_permissions(&server, "ob_plain_get", &["office_bridge::use"]).await;
    let res = reqwest::Client::new()
        .get(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "GET settings requires office_bridge::admin::read");
}

/// TEST-5 — `PUT /api/office-bridge/settings` is 403 without
/// `office_bridge::admin::manage`.
#[tokio::test]
async fn test5_put_settings_without_admin_manage_is_403() {
    let server = TestServer::start_desktop().await;
    let user = create_user_with_permissions(&server, "ob_plain_put", &["office_bridge::use"]).await;
    let res = reqwest::Client::new()
        .put(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403, "PUT settings requires office_bridge::admin::manage");
}

/// TEST-5 — the settings response body must NEVER carry a bridge token or any
/// secret (models.rs invariant). GET as admin and PUT a change; assert neither
/// response leaks a `token`/`secret` field, and that the shape is exactly the
/// four non-secret settings fields.
#[tokio::test]
async fn test5_settings_body_never_contains_a_token_or_secret() {
    let server = TestServer::start_desktop().await;
    let admin = create_user_with_permissions(&server, "ob_secret_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    // GET
    let res = client
        .get(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    let lower = body.to_lowercase();
    assert!(!lower.contains("token"), "GET settings must not leak a token: {body}");
    assert!(!lower.contains("secret"), "GET settings must not leak a secret: {body}");
    let row: Value = serde_json::from_str(&body).unwrap();
    let obj = row.as_object().expect("settings object");
    for key in obj.keys() {
        assert!(
            ["enabled", "port", "last_connected_at", "cert_fingerprint"].contains(&key.as_str()),
            "unexpected settings field `{key}` (possible secret leak): {body}"
        );
    }

    // PUT (a valid change) — same guarantee on the write response.
    let res = client
        .put(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "port": 44300 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body = res.text().await.unwrap();
    let lower = body.to_lowercase();
    assert!(!lower.contains("token"), "PUT settings must not leak a token: {body}");
    assert!(!lower.contains("secret"), "PUT settings must not leak a secret: {body}");
}

/// TEST-5 (admin happy path, complements the 403s) — an admin holding both
/// perms can GET and PUT; the PUT round-trips the port and enabled toggle.
#[tokio::test]
async fn test5_admin_can_get_and_update_settings() {
    let server = TestServer::start_desktop().await;
    let admin = create_user_with_permissions(&server, "ob_rw_admin", admin_perms()).await;
    let client = reqwest::Client::new();

    let res = client
        .put(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false, "port": 44310 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let row: Value = res.json().await.unwrap();
    assert_eq!(row["enabled"], false);
    assert_eq!(row["port"], 44310);

    // Out-of-range port is rejected (handler validation, defense-in-depth).
    let res = client
        .put(server.api_url("/office-bridge/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "port": 70000 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "out-of-range port must be 400");
}

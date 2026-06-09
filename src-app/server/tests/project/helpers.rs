//! Test helpers shared across the project test modules.

#![allow(dead_code)]

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::common::TestServer;
use crate::common::test_helpers::TestUser;

/// Default permission bundle for "a user who can use Projects".
/// Matches the v1 grant in migration 54 (Administrators) — for tests we
/// grant directly to keep the harness minimal.
pub fn full_project_permissions() -> &'static [&'static str] {
    &[
        "projects::create",
        "projects::read",
        "projects::edit",
        "projects::delete",
        "conversations::create",
        "conversations::read",
        "conversations::edit",
        "conversations::delete",
        "files::upload",
        "files::read",
        // The R4 validate_mcp_server_access validator rejects any
        // server_id the caller can't access. Tests that POST MCP
        // settings need to create a real server first; granting
        // mcp_servers::create lets the test fixture do that. (Read is
        // implicit through ownership of the created server.)
        "mcp_servers::create",
        "mcp_servers::read",
    ]
}

/// Create a real MCP server owned by `user`. Required by the project
/// MCP-settings tests now that R4's `validate_mcp_server_access`
/// validator rejects dangling server_ids. Returns the new server's
/// JSON; callers typically just need `["id"]`.
///
/// Uses http transport because the MCP user policy auto-filters
/// `stdio` out of `allowed_transports` whenever `code_sandbox.enabled`
/// is false (test default), so a user-stdio create would 422 with
/// `MCP_TRANSPORT_NOT_ALLOWED` here. The project tests only need a
/// real server id; transport is irrelevant to what they assert.
/// `enabled: false` skips the connection-health probe (which would
/// auto-disable any real-URL probe to example.com anyway).
pub async fn create_user_mcp_server(server: &TestServer, user: &TestUser, name: &str) -> Value {
    let resp = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "name": name,
            "display_name": name,
            "description": "test mcp server",
            "enabled": false,
            "transport_type": "http",
            "url": "https://example.com/mcp",
            "timeout_seconds": 30,
        }))
        .send()
        .await
        .expect("send create mcp server");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create mcp server: {}",
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.expect("parse mcp server")
}

/// Create a project; returns the resulting JSON.
pub async fn create_project(server: &TestServer, user: &TestUser, name: &str) -> Value {
    let resp = reqwest::Client::new()
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "name": name }))
        .send()
        .await
        .expect("send create project");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create project: {}",
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.expect("parse project")
}

/// Create a project with instructions/description + defaults.
pub async fn create_project_with(
    server: &TestServer,
    user: &TestUser,
    payload: Value,
) -> Value {
    let resp = reqwest::Client::new()
        .post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&payload)
        .send()
        .await
        .expect("send create project");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "create project: {}",
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.expect("parse project")
}

/// Fetch project by ID. Returns the parsed body + the HTTP status.
pub async fn get_project(
    server: &TestServer,
    user: &TestUser,
    id: &str,
) -> (StatusCode, Option<Value>) {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/projects/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send get project");
    let status = resp.status();
    let body = if status == StatusCode::OK {
        Some(resp.json().await.expect("parse get response"))
    } else {
        None
    };
    (status, body)
}

/// Delete a project.
pub async fn delete_project(
    server: &TestServer,
    user: &TestUser,
    id: &str,
) -> StatusCode {
    reqwest::Client::new()
        .delete(server.api_url(&format!("/projects/{}", id)))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send delete project")
        .status()
}

/// Create a bare conversation outside any project; returns its id.
pub async fn create_unfiled_conversation(server: &TestServer, user: &TestUser) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("send create conv");
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = resp.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

/// Create a conversation inside a project; returns its id.
///
/// Two HTTP calls: chat creates the conversation unfiled, then the
/// project assign endpoint files it. This mirrors the production
/// frontend flow (chat extension's `afterCreateConversation` hook
/// calls assign after chat auto-creates) so test coverage exercises
/// the real chat↔project decoupling shape.
pub async fn create_project_conversation(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
) -> String {
    let conv_id = create_unfiled_conversation(server, user).await;
    attach_conversation_to_project(server, user, project_id, &conv_id).await;
    conv_id
}

/// Assign an existing conversation to a project via the project
/// assign endpoint. Panics on non-200 so callers can rely on the
/// happy path having succeeded.
pub async fn attach_conversation_to_project(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
    conversation_id: &str,
) {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!(
            "/projects/{}/conversations/{}",
            project_id, conversation_id
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("send assign");
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "assign conv to project: {}",
        resp.text().await.unwrap_or_default()
    );
}

/// Create a conversation with explicit model_id, then assign to a
/// project. Used by real-LLM tests that need to pin the conversation
/// to a specific provider model.
pub async fn create_project_conversation_with_model(
    server: &TestServer,
    user: &TestUser,
    project_id: &str,
    model_id: &str,
) -> Value {
    let create_resp = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": model_id }))
        .send()
        .await
        .expect("send create conv");
    assert_eq!(
        create_resp.status(),
        StatusCode::CREATED,
        "create conv: {}",
        create_resp.text().await.unwrap_or_default()
    );
    let conv: Value = create_resp.json().await.unwrap();
    let conv_id = conv["id"].as_str().unwrap();
    attach_conversation_to_project(server, user, project_id, conv_id).await;
    conv
}

/// Upload a small text file to the user's library; returns the file row.
pub async fn upload_file(server: &TestServer, user: &TestUser, filename: &str, body: &str) -> Value {
    use reqwest::multipart;
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(body.as_bytes().to_vec())
            .file_name(filename.to_string())
            .mime_str("text/plain")
            .unwrap(),
    );
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(form)
        .send()
        .await
        .expect("send upload file");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "upload file: {}",
        resp.text().await.unwrap_or_default()
    );
    resp.json().await.expect("parse file")
}

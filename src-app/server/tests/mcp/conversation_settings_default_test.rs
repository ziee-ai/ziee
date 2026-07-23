//! Conversation MCP settings — approval-mode default + no-clobber contract.
//!
//! Regression coverage for "MCP auto-approve doesn't survive past turn 1": the
//! client's initial per-conversation auto-persist (which exists to snapshot the
//! enabled-server list) used to also pin `approval_mode: manual_approve`, so a
//! deployment whose default is auto-approve ran the first turn's tool without a
//! prompt and then prompted from turn 2 on.
//!
//! `approval_mode` is now OPTIONAL on the PUT and resolved by an inline `COALESCE`
//! inside the existing upsert:
//!   - absent + no row  → the server's `ApprovalMode::default()`
//!   - absent + row     → the row's existing value, untouched
//!   - present          → set explicitly
//!
//! # Branch-agnostic on purpose
//!
//! These files are shared with `deploy-schedule`, where `ApprovalMode::default()`
//! is `auto_approve` instead of `manual_approve`. So nothing here hardcodes the
//! EXPECTED default: it is read from `GET /api/mcp/defaults`.`default_approval_mode`,
//! which the server derives from the same constant. Values the test SUPPLIES are
//! literals — those are branch-independent by construction.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers;
use crate::common::TestServer;

// ============================================================================
// Helpers
// ============================================================================

/// A user who can create a conversation and read/write its MCP settings.
async fn arrange(label: &str) -> (TestServer, test_helpers::TestUser) {
    let server = TestServer::start().await;
    let user = test_helpers::create_user_with_permissions(
        &server,
        label,
        &[
            "conversations::create",
            "conversations::read",
            "conversations::edit",
        ],
    )
    .await;
    (server, user)
}

async fn new_conversation(server: &TestServer, token: &str) -> Uuid {
    let response = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {}", token))
        .json(&json!({}))
        .send()
        .await
        .expect("create conversation");
    assert_eq!(response.status(), 201, "should create conversation");
    let body: serde_json::Value = response.json().await.expect("parse conversation");
    Uuid::parse_str(body["id"].as_str().expect("conversation id")).expect("uuid")
}

/// The server's own default, as reported to clients. This is what a scope with no
/// stored settings resolves to — read rather than hardcoded so the assertions hold
/// on both `khoi` (manual_approve) and `deploy-schedule` (auto_approve).
async fn server_default_approval_mode(server: &TestServer, token: &str) -> String {
    let response = reqwest::Client::new()
        .get(server.api_url("/mcp/defaults"))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("get mcp defaults");
    assert_eq!(response.status(), 200, "defaults GET should succeed");
    let body: serde_json::Value = response.json().await.expect("parse defaults");
    body["default_approval_mode"]
        .as_str()
        .expect("default_approval_mode must be present")
        .to_string()
}

async fn put_settings(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
    payload: serde_json::Value,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conversation_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", token))
        .json(&payload)
        .send()
        .await
        .expect("put mcp settings");
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("parse settings response");
    assert_eq!(status, 200, "settings PUT should succeed; got: {body}");
    body
}

async fn get_settings(
    server: &TestServer,
    token: &str,
    conversation_id: Uuid,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .get(server.api_url(&format!("/conversations/{conversation_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .expect("get mcp settings");
    assert_eq!(response.status(), 200, "settings GET should succeed");
    let body: serde_json::Value = response.json().await.expect("parse settings");
    body["settings"].clone()
}

// ============================================================================
// TEST-11 — an omitted approval_mode on a fresh conversation takes the default
// ============================================================================

#[tokio::test]
async fn test_omitted_approval_mode_on_insert_uses_the_server_default() {
    let (server, user) = arrange("default_insert").await;
    let expected = server_default_approval_mode(&server, &user.token).await;
    let conversation_id = new_conversation(&server, &user.token).await;

    // Nothing stored yet.
    let before = get_settings(&server, &user.token, conversation_id).await;
    assert!(
        before.is_null(),
        "a fresh conversation must have no stored settings; got: {before}"
    );

    // Exactly what the client's turn-1 auto-persist now sends: the server-list
    // snapshot, and NO approval_mode.
    let disabled = json!([{ "server_id": Uuid::new_v4(), "tools": [] }]);
    let written = put_settings(
        &server,
        &user.token,
        conversation_id,
        json!({ "disabled_servers": disabled }),
    )
    .await;

    assert_eq!(
        written["approval_mode"], expected,
        "an insert with approval_mode omitted must take the SERVER default"
    );

    let stored = get_settings(&server, &user.token, conversation_id).await;
    assert_eq!(
        stored["approval_mode"], expected,
        "and that default must be what a later turn reads back"
    );
}

// ============================================================================
// TEST-12 — an omitted approval_mode NEVER mutates a pre-existing row
// ============================================================================

#[tokio::test]
async fn test_omitted_approval_mode_preserves_an_explicit_choice() {
    let (server, user) = arrange("default_preserve").await;

    // Both directions, so this can't pass by coincidence on either branch: one of
    // these two always differs from the compiled default.
    for explicit in ["manual_approve", "auto_approve"] {
        let conversation_id = new_conversation(&server, &user.token).await;

        put_settings(
            &server,
            &user.token,
            conversation_id,
            json!({ "approval_mode": explicit, "disabled_servers": [] }),
        )
        .await;

        // A later save that carries no approval mode (a server-list snapshot, a
        // loop-settings tweak) must leave the user's choice alone.
        let after = put_settings(
            &server,
            &user.token,
            conversation_id,
            json!({ "disabled_servers": [] }),
        )
        .await;
        assert_eq!(
            after["approval_mode"], explicit,
            "an omitted approval_mode must PRESERVE the stored {explicit}, never overwrite it"
        );

        let stored = get_settings(&server, &user.token, conversation_id).await;
        assert_eq!(
            stored["approval_mode"], explicit,
            "the preserved {explicit} must survive a re-read"
        );
    }
}

// ============================================================================
// TEST-13 — explicit choices round-trip; the server-list snapshot still lands
// ============================================================================

#[tokio::test]
async fn test_explicit_approval_mode_round_trips() {
    let (server, user) = arrange("default_explicit").await;

    for mode in ["disabled", "manual_approve", "auto_approve"] {
        let conversation_id = new_conversation(&server, &user.token).await;
        let written = put_settings(
            &server,
            &user.token,
            conversation_id,
            json!({ "approval_mode": mode, "disabled_servers": [] }),
        )
        .await;
        assert_eq!(written["approval_mode"], mode, "{mode} must persist verbatim");

        let stored = get_settings(&server, &user.token, conversation_id).await;
        assert_eq!(stored["approval_mode"], mode, "{mode} must read back verbatim");
    }
}

#[tokio::test]
async fn test_omitted_approval_mode_still_persists_the_server_list() {
    let (server, user) = arrange("default_serverlist").await;
    let conversation_id = new_conversation(&server, &user.token).await;

    // The whole REASON the turn-1 write exists: snapshotting which servers are off.
    let off_a = Uuid::new_v4();
    let off_b = Uuid::new_v4();
    put_settings(
        &server,
        &user.token,
        conversation_id,
        json!({
            "disabled_servers": [
                { "server_id": off_a, "tools": [] },
                { "server_id": off_b, "tools": ["only_this_tool"] },
            ]
        }),
    )
    .await;

    let stored = get_settings(&server, &user.token, conversation_id).await;
    let disabled = stored["disabled_servers"]
        .as_array()
        .expect("disabled_servers array");
    assert_eq!(
        disabled.len(),
        2,
        "dropping approval_mode must not drop the server-list snapshot: {stored}"
    );
    assert!(
        disabled
            .iter()
            .any(|d| d["server_id"] == off_a.to_string() && d["tools"].as_array().unwrap().is_empty()),
        "whole-server disable must round-trip: {stored}"
    );
    assert!(
        disabled
            .iter()
            .any(|d| d["server_id"] == off_b.to_string()
                && d["tools"] == json!(["only_this_tool"])),
        "per-tool disable must round-trip: {stored}"
    );
}

// ============================================================================
// TEST-14 — the two COALESCE arms compose; neither clobbers the other
// ============================================================================

#[tokio::test]
async fn test_omitted_fields_preserve_both_mode_and_auto_approved_tools() {
    let (server, user) = arrange("default_compose").await;
    let conversation_id = new_conversation(&server, &user.token).await;
    let allowed_server = Uuid::new_v4();

    // A user who chose manual approval BUT allow-listed one specific tool — the
    // configuration this bug's workaround produced, and the one most damaged by a
    // clobbering write.
    put_settings(
        &server,
        &user.token,
        conversation_id,
        json!({
            "approval_mode": "manual_approve",
            "auto_approved_tools": [{ "server_id": allowed_server, "tools": ["query_rag"] }],
            "disabled_servers": [],
        }),
    )
    .await;

    // A later save that carries NEITHER field (the auto-persist shape).
    put_settings(
        &server,
        &user.token,
        conversation_id,
        json!({ "disabled_servers": [] }),
    )
    .await;

    let stored = get_settings(&server, &user.token, conversation_id).await;
    assert_eq!(
        stored["approval_mode"], "manual_approve",
        "explicit mode must survive: {stored}"
    );
    let auto = stored["auto_approved_tools"]
        .as_array()
        .expect("auto_approved_tools array");
    assert_eq!(auto.len(), 1, "the allow-list must survive: {stored}");
    assert_eq!(auto[0]["server_id"], allowed_server.to_string());
    assert_eq!(auto[0]["tools"], json!(["query_rag"]));
}

// ============================================================================
// TEST-17 — what the client is TOLD matches what it actually GETS
// ============================================================================

#[tokio::test]
async fn test_advertised_default_matches_what_a_fresh_conversation_receives() {
    let (server, user) = arrange("default_advertised").await;

    // The value the client renders (and used to have to guess) …
    let advertised = server_default_approval_mode(&server, &user.token).await;
    assert!(
        ["disabled", "auto_approve", "manual_approve"].contains(&advertised.as_str()),
        "default_approval_mode must be a valid mode; got {advertised}"
    );

    // … must be exactly the value the server persists for an un-customized
    // conversation. A drift between these two IS the bug: the modal said one
    // thing while the approval gate did another.
    let conversation_id = new_conversation(&server, &user.token).await;
    put_settings(
        &server,
        &user.token,
        conversation_id,
        json!({ "disabled_servers": [] }),
    )
    .await;
    let stored = get_settings(&server, &user.token, conversation_id).await;
    assert_eq!(
        stored["approval_mode"], advertised,
        "the advertised default and the persisted default must be the same value"
    );
}

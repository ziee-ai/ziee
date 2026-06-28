//! Per-conversation opt-out: POST hide-in-conversation removes a skill
//! from the available listing for that conversation only; DELETE
//! restores it.

use serde_json::Value as Json;
use uuid::Uuid;

use super::{FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog};

async fn available_names(server: &crate::common::TestServer, token: &str, conv_id: &str) -> Vec<String> {
    let body: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/skills/available?conversation_id={conv_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("available")
        .json()
        .await
        .expect("parse available");
    body["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .map(|s| s["name"].as_str().unwrap_or("").to_string())
        .collect()
}

#[tokio::test]
async fn hide_then_unhide_toggles_availability() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    let install = install_fixture_skill(&server, &admin.token).await;
    let skill_id = install["skill"]["id"].as_str().unwrap().to_string();

    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &admin.user_id).await;
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &admin.token,
        Some(Uuid::parse_str(model["id"].as_str().unwrap()).unwrap()),
        Some("hide conv"),
    )
    .await;
    let conv_id = conv["id"].as_str().unwrap().to_string();

    // Visible before hiding.
    let before = available_names(&server, &admin.token, &conv_id).await;
    assert!(
        before.iter().any(|n| n == FIXTURE_SKILL_NAME),
        "skill visible before hide: {before:?}"
    );

    // Hide it in this conversation.
    let hide = reqwest::Client::new()
        .post(server.api_url(&format!("/skills/{skill_id}/hide-in-conversation")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "conversation_id": conv_id }))
        .send()
        .await
        .expect("hide");
    assert_eq!(hide.status(), 204, "hide should 204");

    let after_hide = available_names(&server, &admin.token, &conv_id).await;
    assert!(
        !after_hide.iter().any(|n| n == FIXTURE_SKILL_NAME),
        "skill hidden after POST hide-in-conversation: {after_hide:?}"
    );

    // Unhide via DELETE → visible again.
    let unhide = reqwest::Client::new()
        .delete(server.api_url(&format!(
            "/skills/{skill_id}/hide-in-conversation/{conv_id}"
        )))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("unhide");
    assert_eq!(unhide.status(), 204, "unhide should 204");

    let after_unhide = available_names(&server, &admin.token, &conv_id).await;
    assert!(
        after_unhide.iter().any(|n| n == FIXTURE_SKILL_NAME),
        "skill visible again after DELETE: {after_unhide:?}"
    );
}

/// Send a `load_skill` tools/call to the skill MCP server scoped to a specific
/// conversation (via `x-conversation-id`).
async fn load_skill_in_conversation(
    server: &crate::common::TestServer,
    token: &str,
    conversation_id: &str,
    skill_name: &str,
) -> Json {
    reqwest::Client::new()
        .post(server.api_url("/skills/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .header("x-conversation-id", conversation_id)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "load_skill", "arguments": { "name": skill_name } },
        }))
        .send()
        .await
        .expect("load_skill")
        .json()
        .await
        .expect("parse load_skill")
}

/// The per-conversation hide is enforced at the MCP TOOL-CALL boundary, not
/// only in the REST availability listing: once a skill is hidden in a
/// conversation, `load_skill` scoped to THAT conversation is rejected
/// (`SKILL_HIDDEN`), while the SAME skill still loads in a different
/// (non-hidden) conversation. Regression guard for the
/// `is_hidden_in_conversation` check in skill_mcp/tools.rs.
#[tokio::test]
async fn load_skill_is_denied_in_a_conversation_where_it_is_hidden() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    let install = install_fixture_skill(&server, &admin.token).await;
    let skill_id = install["skill"]["id"].as_str().unwrap().to_string();

    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &admin.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let hidden_conv = crate::chat::helpers::create_conversation(
        &server, &admin.token, Some(model_id), Some("hidden conv"),
    )
    .await;
    let hidden_conv_id = hidden_conv["id"].as_str().unwrap().to_string();
    let open_conv = crate::chat::helpers::create_conversation(
        &server, &admin.token, Some(model_id), Some("open conv"),
    )
    .await;
    let open_conv_id = open_conv["id"].as_str().unwrap().to_string();

    // Sanity: load_skill works in the conversation before hiding.
    let pre = load_skill_in_conversation(&server, &admin.token, &hidden_conv_id, FIXTURE_SKILL_NAME).await;
    assert!(pre["error"].is_null(), "load_skill works before hide: {pre}");

    // Hide the skill in `hidden_conv` only.
    let hide = reqwest::Client::new()
        .post(server.api_url(&format!("/skills/{skill_id}/hide-in-conversation")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "conversation_id": hidden_conv_id }))
        .send()
        .await
        .expect("hide");
    assert_eq!(hide.status(), 204, "hide should 204");

    // The tool call is now DENIED in the hidden conversation …
    let denied = load_skill_in_conversation(&server, &admin.token, &hidden_conv_id, FIXTURE_SKILL_NAME).await;
    assert!(
        denied["error"].is_object(),
        "load_skill must be denied in the hidden conversation: {denied}"
    );
    let msg = serde_json::to_string(&denied["error"]).unwrap();
    assert!(
        msg.contains("SKILL_HIDDEN") || msg.to_lowercase().contains("hidden"),
        "denial must be the SKILL_HIDDEN error: {denied}"
    );

    // … but still works in a DIFFERENT conversation (scope is per-conversation).
    let still_ok = load_skill_in_conversation(&server, &admin.token, &open_conv_id, FIXTURE_SKILL_NAME).await;
    assert!(
        still_ok["error"].is_null(),
        "load_skill must still work in a non-hidden conversation: {still_ok}"
    );
}

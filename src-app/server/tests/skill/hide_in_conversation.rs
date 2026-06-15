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

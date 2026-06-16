//! Path B progressive-disclosure contract: the available-skills listing
//! the chat extension injects carries name + description + when_to_use,
//! but NEVER the SKILL.md body. The body loads only on demand via
//! `skill_mcp::load_skill` (covered in `skill_mcp_load`).

use serde_json::Value as Json;
use uuid::Uuid;

use super::{FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog};

#[tokio::test]
async fn available_listing_has_description_not_body() {
    let (server, _stub_guard, conv_id, token) = {
        let (server, _mock) = server_with_skill_catalog().await;
        let admin = admin_and_refresh(&server).await;
        install_fixture_skill(&server, &admin.token).await;

        // A conversation is needed as the listing scope. Stub model so
        // no API key is required.
        let (stub, model) =
            crate::chat::helpers::create_stub_model(&server, &admin.user_id).await;
        let conv = crate::chat::helpers::create_conversation(
            &server,
            &admin.token,
            Some(Uuid::parse_str(model["id"].as_str().unwrap()).unwrap()),
            Some("skill listing conv"),
        )
        .await;
        let conv_id = conv["id"].as_str().unwrap().to_string();
        (server, stub, conv_id, admin.token)
    };

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/skills/available?conversation_id={conv_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("available skills");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse available");
    assert_eq!(status, 200, "available should 200; got {status}: {body}");

    let skills = body["skills"].as_array().expect("skills array");
    let entry = skills
        .iter()
        .find(|s| s["name"] == FIXTURE_SKILL_NAME)
        .unwrap_or_else(|| panic!("installed skill must be available: {body}"));

    // Description + when_to_use present (so the model knows when to call).
    assert!(
        entry["description"]
            .as_str()
            .unwrap_or("")
            .contains("configure local + cloud LLM providers"),
        "listing carries description: {entry}"
    );
    assert!(
        entry["when_to_use"].as_str().unwrap_or("").contains("API key"),
        "listing carries when_to_use: {entry}"
    );

    // Path B: the full SKILL.md body marker must NOT appear ANYWHERE in
    // the available-skills response (no body, no reference content).
    let serialized = serde_json::to_string(&body).unwrap();
    assert!(
        !serialized.contains("THIS_IS_THE_SKILL_BODY_MARKER"),
        "available listing must NOT inject the SKILL.md body (Path B): {serialized}"
    );
    assert!(
        !serialized.contains("REFERENCE_FILE_MARKER"),
        "available listing must NOT inject reference file content (Path B)"
    );

    // The entry should be the lightweight shape: only id/name/
    // description/when_to_use, no `extracted_path` / `frontmatter_json`.
    assert!(
        entry.get("extracted_path").is_none(),
        "available entry must not leak extracted_path: {entry}"
    );
    assert!(
        entry.get("frontmatter_json").is_none(),
        "available entry must not leak frontmatter_json: {entry}"
    );
}

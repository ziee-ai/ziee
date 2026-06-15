//! Install the fixture skill from the (mock) hub catalog → assert the
//! full bundle pipeline: DB row + on-disk extract + frontmatter parse +
//! hub_entities tracking.

use serde_json::Value as Json;

use super::{FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog};

#[tokio::test]
async fn user_install_creates_row_extract_and_tracking() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;

    let body = install_fixture_skill(&server, &admin.token).await;
    let skill = &body["skill"];

    // Row identity + scope.
    assert_eq!(skill["name"], FIXTURE_SKILL_NAME, "name persisted: {body}");
    assert_eq!(skill["scope"], "user", "user endpoint forces scope=user: {body}");
    assert!(
        skill["owner_user_id"].is_string(),
        "user-scope skill must have an owner: {body}"
    );
    assert_eq!(skill["entry_point"], "SKILL.md", "entry_point: {body}");
    assert_eq!(skill["is_dev"], false, "hub install is not is_dev: {body}");

    // Frontmatter parsed into display_name / description / when_to_use +
    // the opaque frontmatter_json blob (Agent Skills spec).
    assert_eq!(
        skill["display_name"], "configure-llm-providers",
        "display_name from SKILL.md `name`: {body}"
    );
    assert!(
        skill["description"]
            .as_str()
            .unwrap_or("")
            .contains("configure local + cloud LLM providers"),
        "description from frontmatter: {body}"
    );
    assert!(
        skill["when_to_use"].as_str().unwrap_or("").contains("API key"),
        "when_to_use from frontmatter: {body}"
    );
    let fm = &skill["frontmatter_json"];
    assert!(fm.is_object(), "frontmatter_json is an object: {body}");
    assert_eq!(
        fm["allowed-tools"], "Read",
        "frontmatter preserves opaque fields (allowed-tools): {body}"
    );
    assert_eq!(
        fm["metadata"]["author"], "ziee",
        "frontmatter preserves nested metadata: {body}"
    );

    // On-disk extract: extracted_path exists with SKILL.md + the
    // reference file (the bundle shipped both).
    let extracted_path = skill["extracted_path"].as_str().expect("extracted_path string");
    let skill_md = std::path::Path::new(extracted_path).join("SKILL.md");
    assert!(
        skill_md.exists(),
        "SKILL.md must exist on disk at {}",
        skill_md.display()
    );
    let on_disk = std::fs::read_to_string(&skill_md).expect("read SKILL.md");
    assert!(
        on_disk.contains("THIS_IS_THE_SKILL_BODY_MARKER"),
        "extracted SKILL.md carries the body"
    );
    let ref_md = std::path::Path::new(extracted_path).join("references/provider-types.md");
    assert!(ref_md.exists(), "reference file extracted at {}", ref_md.display());

    // file_count + bundle_sha256 recorded.
    assert_eq!(skill["file_count"], 2, "two files in the bundle: {body}");
    assert!(
        skill["bundle_sha256"].as_str().unwrap_or("").len() == 64,
        "bundle_sha256 is a 64-char hex digest: {body}"
    );

    // Hub tracking row.
    let tracking = &body["hub_tracking"];
    assert_eq!(tracking["entity_type"], "skill", "tracking entity_type: {body}");
    assert_eq!(tracking["hub_category"], "skill", "tracking hub_category: {body}");
    assert_eq!(tracking["hub_id"], FIXTURE_SKILL_NAME, "tracking hub_id: {body}");

    // The skill now appears in GET /skills.
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list skills")
        .json()
        .await
        .expect("parse list");
    let found = list["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .any(|s| s["name"] == FIXTURE_SKILL_NAME);
    assert!(found, "installed skill appears in GET /skills: {list}");
}

#[tokio::test]
async fn system_install_creates_system_scope_row() {
    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/system/install-from-hub"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("system install");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse body");
    assert_eq!(status, 201, "system install should 201; got {status}: {body}");

    let skill = &body["skill"];
    assert_eq!(skill["scope"], "system", "system endpoint forces scope=system: {body}");
    assert!(
        skill["owner_user_id"].is_null(),
        "system-scope skill must have null owner: {body}"
    );

    // On-disk extract present for system scope too.
    let extracted_path = skill["extracted_path"].as_str().expect("extracted_path");
    assert!(
        std::path::Path::new(extracted_path).join("SKILL.md").exists(),
        "system skill SKILL.md extracted"
    );
}

//! Built-in capability skills: ziee's embedded self-documentation, synced
//! into the `skills` table as `scope='built_in'` rows on server boot. They
//! are always available to every user and NOT uninstallable.

use serde_json::Value as Json;

use crate::common::test_helpers::create_user_with_permissions;
use super::server_with_skill_catalog;

const A_BUILTIN: &str = "io.github.ziee/configure-llm-providers";

/// Poll GET /skills until the boot-synced built-ins show up (the sync is a
/// spawned task on server init), then return the parsed list.
async fn wait_for_builtins(server: &crate::common::TestServer, token: &str) -> Vec<Json> {
    for _ in 0..40 {
        let list: Json = reqwest::Client::new()
            .get(server.api_url("/skills"))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("list")
            .json()
            .await
            .expect("parse");
        let skills = list["skills"].as_array().cloned().unwrap_or_default();
        if skills.iter().any(|s| s["scope"] == "built_in") {
            return skills;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("built-in skills never appeared in GET /skills within ~10s");
}

#[tokio::test]
async fn builtin_skills_are_synced_listed_and_not_deletable() {
    let (server, _mock) = server_with_skill_catalog().await;
    let user = create_user_with_permissions(
        &server,
        "builtin_user",
        &["skills::read", "skills::install", "skills::manage"],
    )
    .await;

    let skills = wait_for_builtins(&server, &user.token).await;

    // The capability skill is present as a built_in-scope row.
    let builtin = skills
        .iter()
        .find(|s| s["name"] == A_BUILTIN)
        .unwrap_or_else(|| panic!("built-in {A_BUILTIN} not in list: {skills:?}"));
    assert_eq!(builtin["scope"], "built_in", "scope is built_in: {builtin}");
    assert!(
        builtin["display_name"].as_str().is_some(),
        "built-in has a display_name: {builtin}"
    );
    assert!(
        builtin["description"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("provider"),
        "built-in description carries its frontmatter: {builtin}"
    );

    // All 13 ziee capability skills are present (3 life-science skills were
    // added to the embedded `resources/builtin-skills/` set).
    let builtin_count = skills.iter().filter(|s| s["scope"] == "built_in").count();
    assert_eq!(
        builtin_count, 13,
        "all 13 built-in capability skills synced; got {builtin_count}"
    );

    // Not uninstallable: DELETE /skills/{id} is rejected (not user-scope / owner).
    let id = builtin["id"].as_str().expect("id");
    let status = reqwest::Client::new()
        .delete(server.api_url(&format!("/skills/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete")
        .status();
    assert!(
        status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_FOUND,
        "built-in skill must not be user-deletable; got {status}"
    );
}

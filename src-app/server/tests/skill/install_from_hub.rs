//! Install the fixture skill from the (mock) hub catalog → assert the
//! full bundle pipeline: DB row + on-disk extract + frontmatter parse +
//! hub_entities tracking.

use serde_json::Value as Json;

use super::{FIXTURE_SKILL_NAME, admin_and_refresh, install_fixture_skill, server_with_skill_catalog};

use super::{FIXTURE_REFERENCE_MD, FIXTURE_SKILL_MD, refresh_catalog};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};
use crate::hub::mock_release_server::{MockItem, MockVersion, spawn_mock_hub};

/// The download → sha256-verify path must REJECT a bundle whose manifest
/// advertises a sha256 that doesn't match the served bytes
/// (bundle.rs:189-197 → `AppError::unprocessable_entity`), surfacing a 422
/// through the install HTTP handler and creating no skill row. The mock
/// serves the real tar.gz but the manifest's `bundle.sha256` is forged to
/// all-zeros via `extra_json` (deep-merged over the computed sha).
#[tokio::test]
async fn install_rejects_bundle_with_sha256_mismatch() {
    const NAME: &str = "io.github.test/corrupt-skill";
    let catalog = vec![MockVersion {
        version: "9.9.2-test",
        prerelease: true,
        items: vec![MockItem {
            extra_json: Some(serde_json::json!({
                "bundle": {
                    "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
                }
            })),
            ..MockItem::bundle(
                "skill",
                NAME,
                vec![
                    ("SKILL.md", FIXTURE_SKILL_MD),
                    ("references/provider-types.md", FIXTURE_REFERENCE_MD),
                ],
            )
        }],
    }];
    let mock = spawn_mock_hub(catalog).await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(
        &server,
        "corrupt_skill_admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
        ],
    )
    .await;
    refresh_catalog(&server, &admin.token).await;

    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/install-from-hub"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "hub_id": NAME }))
        .send()
        .await
        .expect("install");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse install body");
    assert_eq!(
        status, 422,
        "forged-sha install must be 422 Unprocessable; got {status}: {body}"
    );
    let body_str = body.to_string();
    assert!(
        body_str.contains("BUNDLE_SHA256_MISMATCH") || body_str.to_lowercase().contains("sha256"),
        "error names the sha256 mismatch: {body}"
    );

    // The rejected install created no skill row.
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/skills"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("list skills")
        .json()
        .await
        .expect("parse list");
    assert!(
        !list.to_string().contains(NAME),
        "corrupt skill must not be installed: {list}"
    );
}

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

/// Skill realtime-sync emission: a USER install from the hub
/// (create_skill_from_hub, hub/handlers.rs:1789) publishes an owner-scoped
/// `skill`/`create` frame. The owner observes it; an unrelated user stays
/// silent. The Skill SyncEntity had no expect_event coverage.
#[tokio::test]
async fn user_skill_install_emits_owner_scoped_skill_create() {
    use crate::common::sync_probe::SyncProbe;
    use std::time::Duration;

    let (server, _mock) = server_with_skill_catalog().await;
    let owner = admin_and_refresh(&server).await;
    let other = create_user_with_permissions(&server, "skill_sync_other", &["skills::read"]).await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let _ = install_fixture_skill(&server, &owner.token).await;

    owner_probe
        .expect_event("skill", "create", Duration::from_secs(5))
        .await;
    other_probe.expect_silence(Duration::from_secs(1)).await;
}

/// Cross-subsystem: skill_mcp (load_skill) coexists with the memory built-in
/// (remember) in ONE conversation. The skill test suite is otherwise
/// skill-isolated; this proves the skill subsystem attaches ALONGSIDE another
/// subsystem (memory) for a tool-capable model — refuting the "no cross-subsystem
/// flows" gap.
#[tokio::test]
async fn skill_mcp_coexists_with_memory_builtin() {
    use crate::common::stub_chat::{register_stub_model, StubChat};

    let (server, _mock) = server_with_skill_catalog().await;
    let user = admin_and_refresh(&server).await;

    // Install a skill so the skill chat-extension attaches skill_mcp (load_skill).
    let _ = install_fixture_skill(&server, &user.token).await;

    // Enable memory deployment-wide + per-user extraction so `remember` attaches.
    let client = reqwest::Client::new();
    client
        .put(server.api_url("/memory/admin-settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "extraction_enabled": true }))
        .send()
        .await
        .unwrap();

    let stub = StubChat::start().await;
    let model_id = register_stub_model(
        &server, &user.token, &user.user_id, &stub.base_url, true, None,
    )
    .await;
    let model_uuid = crate::chat::helpers::parse_uuid(&serde_json::json!(model_id));
    let conversation =
        crate::chat::helpers::create_conversation(&server, &user.token, Some(model_uuid), None)
            .await;
    let conv_id = crate::chat::helpers::parse_uuid(&conversation["id"]);
    let branch_id = crate::chat::helpers::parse_uuid(&conversation["active_branch_id"]);

    let _ = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &user.token,
        conv_id,
        serde_json::json!({
            "content": "hello",
            "model_id": model_id,
            "branch_id": branch_id.to_string(),
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [] }
        }),
        &["complete"],
    )
    .await;

    let reqs = stub.requests();
    let first = reqs.first().expect("at least one recorded request");
    assert!(
        first.has_tool("load_skill"),
        "skill_mcp's load_skill must attach (a skill is installed); tools={:?}",
        first.tool_names
    );
    assert!(
        first.has_tool("remember"),
        "memory's remember must attach ALONGSIDE skill_mcp; tools={:?}",
        first.tool_names
    );
/// CHARACTERIZATION (documents CURRENT behavior + a known gap): deleting a
/// hub-installed SKILL does NOT remove its `hub_entities` tracking row — unlike
/// assistants/MCP servers, the event-driven CleanupHubEntitiesHandler does not
/// subscribe to skill deletion and there is no FK cascade, so the row leaks.
/// This pins the current state so a future fix (wiring skill-delete cleanup)
/// flips this assertion. See /tmp/discovered-claude-live4.md.
#[tokio::test]
async fn deleting_hub_skill_currently_leaves_hub_entities_row() {
    use sqlx::postgres::PgPoolOptions;

    let (server, _mock) = server_with_skill_catalog().await;
    let admin = admin_and_refresh(&server).await;
    let body = install_fixture_skill(&server, &admin.token).await;
    let skill_id = uuid::Uuid::parse_str(body["skill"]["id"].as_str().expect("skill id")).unwrap();

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();

    // The install created a hub_entities tracking row for the skill.
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM hub_entities WHERE entity_type = 'skill' AND entity_id = $1",
    )
    .bind(skill_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, 1, "install must create a hub_entities tracking row");

    // Delete the skill.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/skills/{skill_id}")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("delete skill");
    assert_eq!(del.status(), 204, "skill delete should 204");
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // CURRENT behavior: the tracking row LEAKS (no cleanup wired for skills).
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM hub_entities WHERE entity_type = 'skill' AND entity_id = $1",
    )
    .bind(skill_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        after, 1,
        "CURRENT (buggy) behavior: the hub_entities row is NOT cleaned up on \
         skill deletion. When skill-delete cleanup is wired, flip this to 0."
    );
    pool.close().await;
}

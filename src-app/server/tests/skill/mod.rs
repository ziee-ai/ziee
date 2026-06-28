//! Skill consumer integration tests (plan §7 — consumer integration tier).
//!
//! Covers the install-from-hub bundle pipeline, the Path B chat listing
//! (description-only, NO body injection), per-conversation hide, and the
//! `skill_mcp` `load_skill` / `read_skill_file` tools incl. path-safety.
//!
//! All tests run offline: hub catalog + bundles are served by the
//! in-test mock Pages server (`hub::mock_release_server`) via the
//! debug-only `ZIEE_HUB_PAGES_BASE` override, so the download → sha256
//! → extract path runs for real without touching GitHub.

mod access_and_security;
mod builtin;
mod bundle_security_http;
mod hide_in_conversation;
mod install_from_hub;
mod listing_in_chat;
mod real_llm;
mod skill_mcp_load;
mod sync_emit_test;

use serde_json::Value as Json;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use crate::hub::mock_release_server::{MockHub, MockItem, MockVersion, spawn_mock_hub};

/// The SKILL.md body the fixture skill ships. Path B contract: this
/// text must NOT appear in the chat listing — only via `load_skill`.
// Raw string literal (NOT "...\n\" continuation): a backslash-newline
// continuation strips the LEADING WHITESPACE of the next source line, which
// would silently de-indent the nested `metadata:` block and turn it into
// `metadata: null` + top-level author/license. Raw string preserves the
// 2-space indentation so the nested YAML survives.
pub const FIXTURE_SKILL_MD: &str = r#"---
name: configure-llm-providers
description: How to configure local + cloud LLM providers in ziee.
when_to_use: When the user mentions provider, API key, Ollama, or model registry.
allowed-tools: Read
metadata:
  author: ziee
  license: MIT
---

# Configuring LLM providers

THIS_IS_THE_SKILL_BODY_MARKER. Step 1: open settings. Step 2: add a key.
"#;

/// A supporting reference file the fixture skill ships under references/.
pub const FIXTURE_REFERENCE_MD: &str =
    "# Provider types\n\nREFERENCE_FILE_MARKER. Local vs cloud providers.\n";

/// Reverse-DNS name of the fixture skill the mock catalog serves.
pub const FIXTURE_SKILL_NAME: &str = "io.github.test/configure-llm-providers";

/// One mock catalog version carrying a single skill item that ships a
/// real SKILL.md + references/ bundle.
pub fn skill_catalog() -> Vec<MockVersion> {
    vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem::bundle(
            "skill",
            FIXTURE_SKILL_NAME,
            vec![
                ("SKILL.md", FIXTURE_SKILL_MD),
                ("references/provider-types.md", FIXTURE_REFERENCE_MD),
            ],
        )],
    }]
}

/// Boot a TestServer wired to a mock Pages server serving the skill
/// catalog + bundle. Returns `(server, mock)`. Keep the `MockHub`
/// alive for the test (its task is detached but the struct owns config).
pub async fn server_with_skill_catalog() -> (TestServer, MockHub) {
    let mock = spawn_mock_hub(skill_catalog()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    (server, mock)
}

/// Refresh the catalog from the mock so the seed catalog is replaced by
/// the mock's (skill-bearing) one. Mirrors `catalog_hermetic::apply_catalog`.
pub async fn refresh_catalog(server: &TestServer, admin_token: &str) {
    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .send()
        .await
        .expect("refresh");
    assert_eq!(
        resp.status(),
        200,
        "/hub/refresh against mock Pages must 200; got {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default(),
    );
}

/// Install the fixture skill from the mock hub on the user-scope
/// endpoint. Returns the install response body (`{skill, hub_tracking}`).
pub async fn install_fixture_skill(server: &TestServer, token: &str) -> Json {
    let resp = reqwest::Client::new()
        .post(server.api_url("/skills/install-from-hub"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "hub_id": FIXTURE_SKILL_NAME }))
        .send()
        .await
        .expect("install skill");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse install body");
    assert_eq!(
        status, 201,
        "skill install-from-hub should 201; got {status}: {body}"
    );
    body
}

/// Create an admin who can refresh the catalog + install user + system
/// skills, then refresh so the mock catalog is active.
pub async fn admin_and_refresh(server: &TestServer) -> crate::common::test_helpers::TestUser {
    let admin = create_user_with_permissions(
        server,
        "skill_admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "skills::read",
            "skills::install",
            "skills::manage",
            "skills::manage_system",
        ],
    )
    .await;
    refresh_catalog(server, &admin.token).await;
    admin
}

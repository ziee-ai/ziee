//! Tier 3 — the per-conversation full-text view bind-mounted READ-ONLY at
//! `/lit` inside the sandbox, exercised through the full production path:
//! fetch a paper (populates the view) → `execute_command` reads it at `/lit`
//! and a write to `/lit` is denied.
//!
//! Rootfs-gated: self-skips (NOT `#[ignore]`) when the host can't run the
//! sandbox (no bwrap / no published rootfs for this arch), via
//! `github_fetch_server_options`.

#![allow(unused_imports)]

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::code_sandbox::harness::{create_test_conversation, github_fetch_server_options, tool_call};
use crate::common::{test_helpers, TestServer};
use crate::lit_search::{configure, jsonrpc_conv, start_mock_epmc_fulltext};

// Linux-only: the per-conversation `/lit` view is bind-mounted into the guest
// only on the Linux bwrap-direct backend today (the macOS libkrun / WSL2
// backends don't yet plumb the lit-cache share). `#[cfg(target_os)]` (not
// `#[ignore]`) so it isn't even compiled on platforms where it can't pass.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn lit_view_is_readonly_mounted_in_sandbox() {
    // Mock Europe PMC fullTextXML first (its port goes into the server env).
    let (epmc, _hits) = start_mock_epmc_fulltext().await;
    let Some(opts) = github_fetch_server_options(vec![
        ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), epmc),
    ]) else {
        return; // no bwrap / no rootfs for this arch — skip cleanly
    };
    let server = TestServer::start_with_options(opts).await;

    // One user with sandbox-execute + lit_search use/admin; owns the conversation.
    let user = test_helpers::create_user_with_permissions(
        &server,
        "ls_sandbox",
        &[
            "code_sandbox::execute",
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc"]).await;

    let user_id = Uuid::parse_str(&user.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;

    // Fetch a paper into this conversation's /lit view.
    let res = jsonrpc_conv(
        &server,
        &user.token,
        &conv_id.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC123456"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sandbox_path = body["result"]["structuredContent"]["papers"][0]["sandbox_path"]
        .as_str()
        .expect("a /lit sandbox_path after fetch");
    assert!(sandbox_path.starts_with("/lit/"), "got: {sandbox_path}");

    // READ: the sandbox can cat the fetched paper from the read-only /lit mount.
    let read = tool_call(
        &server,
        &user.token,
        conv_id,
        "execute_command",
        json!({ "command": "cat /lit/*.txt", "flavor": "minimal" }),
    )
    .await;
    let stdout = read["result"]["structuredContent"]["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("CRISPR base editing off-target"),
        "sandbox must read the fetched paper at /lit; result: {}",
        read["result"]["structuredContent"]
    );

    // WRITE: /lit is read-only — a write must fail.
    let write = tool_call(
        &server,
        &user.token,
        conv_id,
        "execute_command",
        json!({
            "command": "sh -c 'echo x > /lit/blocked.txt 2>/dev/null && echo WROTE || echo READONLY'",
            "flavor": "minimal"
        }),
    )
    .await;
    let stdout = write["result"]["structuredContent"]["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("READONLY") && !stdout.contains("WROTE"),
        "writing to /lit must be denied (read-only bind); result: {}",
        write["result"]["structuredContent"]
    );
}

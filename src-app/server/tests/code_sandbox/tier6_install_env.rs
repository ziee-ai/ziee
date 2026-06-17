//! Tier 6 — Part A (on-demand install layer) through the FULL production path.
//!
//! Boots the sandbox and runs `printenv` inside it, asserting the install-prefix
//! environment that `build_bwrap_argv` injects via `--setenv` actually reaches
//! the sandbox process: the persistent micromamba / pip-user / npm prefixes that
//! make user-space installs discoverable across `execute_command` calls.
//!
//! Cheap + offline (no real package install / network): it only proves the env
//! plumbing, which is the one backend change Part A makes. The install *skills*
//! are content-only and need no runtime test. Self-skips when the host can't run
//! the sandbox / no published rootfs for this arch (same gate as the other
//! Tier-6 tests).

#![allow(unused_imports)]

use crate::code_sandbox::harness::{create_test_conversation, github_fetch_server_options, tool_call};
use crate::common::{test_helpers, TestServer};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn e2e_install_prefix_env_is_injected() {
    let Some(opts) = github_fetch_server_options(Vec::new()) else {
        return;
    };
    let server = TestServer::start_with_options(opts).await;

    let user = test_helpers::create_user_with_permissions(
        &server,
        "tier6_env",
        &["code_sandbox::execute"],
    )
    .await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;

    // Print the install-layer env in one shot. `printenv` is in coreutils (in
    // every rootfs flavor); we don't need any package installed.
    let body = tool_call(
        &server,
        &user.token,
        conv_id,
        "execute_command",
        json!({
            "command": "printenv MAMBA_ROOT_PREFIX PYTHONUSERBASE npm_config_prefix PIP_USER; echo \"PATH=$PATH\"",
            "flavor": "minimal",
        }),
    )
    .await;

    let structured = &body["result"]["structuredContent"];
    let stdout = structured["stdout"].as_str().unwrap_or("");

    // The persistent prefixes live under the per-conversation $HOME so installs
    // survive across calls (see build_bwrap_argv install-prefix block).
    assert!(
        stdout.contains("/home/sandboxuser/.ziee/micromamba"),
        "MAMBA_ROOT_PREFIX should be injected; stdout: {stdout:?}"
    );
    assert!(
        stdout.contains("/home/sandboxuser/.local"),
        "PYTHONUSERBASE should point at the pip-user prefix; stdout: {stdout:?}"
    );
    assert!(
        stdout.contains("/home/sandboxuser/.ziee/npm"),
        "npm_config_prefix should be injected; stdout: {stdout:?}"
    );
    assert!(
        stdout.contains("PIP_USER=1") || stdout.lines().any(|l| l.trim() == "1"),
        "PIP_USER=1 should be set; stdout: {stdout:?}"
    );
    // PATH must prepend the user-space bin dirs so a freshly-installed tool is
    // found without sourcing the (masked) ~/.bashrc.
    assert!(
        stdout.contains("/home/sandboxuser/.local/bin")
            && stdout.contains("/home/sandboxuser/.ziee/micromamba/bin"),
        "PATH should prepend the install-prefix bin dirs; stdout: {stdout:?}"
    );
}

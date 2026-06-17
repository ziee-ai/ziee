//! Tier 6 — host-folder mounting through the FULL production HTTP path.
//!
//! Boots the **desktop** binary (which registers the host-mount
//! `SandboxMountProvider`) with the sandbox enabled, inserts a `host_mounts`
//! row pointing at a real temp folder on this machine, then calls
//! `execute_command` and asserts the sandbox reads a file from the mounted
//! folder at `/mnt/<full host path>`.
//!
//! On macOS this exercises the libkrun virtio-fs share + the rebuilt
//! `sandbox-vm-launcher` / `sandbox-guest-agent`; on Linux the direct bwrap
//! `--ro-bind`. Self-skips when the host can't run the sandbox / no published
//! rootfs for this arch (same gate as the other Tier-6 tests).

#![allow(unused_imports)]

use crate::code_sandbox::harness::{create_test_conversation, github_fetch_server_options, tool_call};
use crate::common::{test_helpers, TestServer};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn e2e_host_folder_mount_reads_file() {
    let Some(mut opts) = github_fetch_server_options(Vec::new()) else {
        return;
    };
    // The host-mount provider lives in the desktop crate and is registered at
    // boot only by `ziee-desktop`. Use it so execute_command sees the mount.
    opts.use_desktop_binary = true;
    let server = TestServer::start_with_options(opts).await;

    // A real host folder with a known file (on this machine — the same host the
    // server runs on, so its mac_vm/bwrap can reach it).
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hostfile.txt"), b"HOST_MOUNT_OK").unwrap();
    let host_path = dir.path().to_string_lossy().to_string();

    // User (with execute perm) + conversation; attach the host folder to it.
    let user = test_helpers::create_user_with_permissions(
        &server,
        "tier6_hm",
        &["code_sandbox::execute", "host_mount::read", "host_mount::manage"],
    )
    .await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let conv_id = create_test_conversation(&pool, user_id).await;
    sqlx::query("INSERT INTO host_mounts (conversation_id, user_id, mounts) VALUES ($1, $2, $3)")
        .bind(conv_id)
        .bind(user_id)
        .bind(json!([{ "host_path": host_path, "read_only": true }]))
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    // The provider derives the in-sandbox path as `/mnt/<full host path>`.
    let sandbox_path = format!("/mnt/{}", host_path.trim_start_matches('/'));
    let body = tool_call(
        &server,
        &user.token,
        conv_id,
        "execute_command",
        json!({ "command": format!("cat '{sandbox_path}/hostfile.txt'"), "flavor": "minimal" }),
    )
    .await;

    let structured = &body["result"]["structuredContent"];
    let stdout = structured["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("HOST_MOUNT_OK"),
        "expected the mounted file's content in stdout; full result: {structured}"
    );
    // The active host mounts are reported back to the model.
    assert!(
        structured["mounts"].is_array() && !structured["mounts"].as_array().unwrap().is_empty(),
        "execute_command should report the active host mount; result: {structured}"
    );
}

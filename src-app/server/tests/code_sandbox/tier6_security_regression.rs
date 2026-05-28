//! Tier 6 — security regression suite.
//! Tests the exact bugs the audit caught actually stay fixed end-to-end.
//! All `#[ignore]`'d; require rootfs + bwrap.

#![allow(unused_imports)]

use crate::code_sandbox::harness::{
    create_test_conversation, enabled_test_server, post_jsonrpc, test_server_jwt, tool_call,
};
use crate::common::test_helpers;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

async fn setup_user_and_conv(server: &crate::common::TestServer) -> (Uuid, String, Uuid) {
    let test_user = test_helpers::create_user_with_permissions(
        server,
        "tier6sec_user",
        &["code_sandbox::execute"],
    )
    .await;
    let user_id = Uuid::parse_str(&test_user.user_id).expect("user uuid");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect");
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;
    (user_id, test_user.token, conv_id)
}

/// CRITICAL regression: a sandboxed shell plants a dangling symlink
/// at /home/sandboxuser/foo → /tmp/test-sandbox-pwn. Without the
/// `canonicalize_in_workspace` symlink-component rejection (commit
/// d28cc88), a follow-up write_file("foo", payload) would have
/// tokio::fs::write FOLLOW the symlink and write payload to the host's
/// /tmp/test-sandbox-pwn — host filesystem clobber.
#[tokio::test]
async fn e2e_dangling_symlink_does_not_clobber_host() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;

    // Target file: must NOT exist before the test, must NOT exist after.
    let target = format!("/tmp/test-sandbox-pwn-{}", Uuid::new_v4());
    assert!(
        !std::path::Path::new(&target).exists(),
        "test setup: target must not preexist"
    );

    // Step 1: plant the dangling symlink from inside the sandbox AND
    // positive-control that it was actually planted. Without this
    // confirmation, a silent `ln -s` failure (e.g. /home/sandboxuser
    // not writable) would make the "host target absent" assertion
    // below pass for the WRONG reason (no symlink → no follow → no
    // clobber → no detection of a broken defense).
    let plant = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": format!(
            "ln -s {} /home/sandboxuser/innocent.txt && \
             test -L /home/sandboxuser/innocent.txt && echo SYMLINK_PLANTED",
            target
        ) }),
    )
    .await;
    let plant_stdout = plant["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    assert!(
        plant_stdout.contains("SYMLINK_PLANTED"),
        "setup did not plant the symlink: stdout={plant_stdout:?}"
    );

    // Step 2: try to write through the symlink.
    let resp = post_jsonrpc(
        &server,
        &jwt,
        Some(conv_id),
        "tools/call",
        json!({
            "name": "write_file",
            "arguments": { "filename": "innocent.txt", "content": "host-clobber-payload" }
        }),
    )
    .await;
    let body: serde_json::Value = resp.json().await.expect("parse");
    // The call MUST have errored (defense in canonicalize_in_workspace
    // rejects the symlink component). We don't string-match the
    // message because the JSON-RPC envelope deliberately ships only a
    // generic "tool write_file failed" to avoid info leak — the
    // specific error code is logged server-side. The behavioral
    // assertion below (host target does NOT exist) is the real proof.
    assert!(
        body.get("error").is_some(),
        "expected JSON-RPC error envelope, got: {body}"
    );

    // The KEY security assertion: confirm the host target was NOT
    // created. If the symlink defense had failed, write_file would
    // have followed the symlink and clobbered /tmp/test-sandbox-pwn-*.
    assert!(
        !std::path::Path::new(&target).exists(),
        "SECURITY FAILURE: host target {target} was created via dangling symlink"
    );
}

/// CRITICAL regression: server env (DATABASE_URL, JWT secrets, *_API_KEY)
/// must NOT leak into the sandboxed bash. The `--clearenv` flag (commit
/// d28cc88) is the defense; this test confirms it actually wipes env.
/// We set a sentinel env var on the spawned server, then ask the
/// sandbox to print its env, then assert the sentinel isn't there.
#[tokio::test]
async fn e2e_clearenv_wipes_server_env_from_sandbox() {
    let Some(rootfs) = crate::code_sandbox::harness::rootfs_path() else {
        eprintln!("test skipped: no rootfs");
        return;
    };
    if !crate::code_sandbox::harness::bwrap_available() {
        eprintln!("test skipped: no bwrap");
        return;
    }
    // Spin up a server with our sentinel env var visible to it.
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        sandbox_enabled: true,
        sandbox_rootfs: Some(rootfs),
        sandbox_cgroup_parent: String::new(),
        extra_env: vec![(
            "ZIEE_TEST_SECRET_SENTINEL".into(),
            "this-must-not-leak-to-sandbox".into(),
        )],
        sandbox_cache_tempdir: None,
                use_desktop_binary: false,
    })
    .await;

    let test_user = test_helpers::create_user_with_permissions(
        &server,
        "tier6_env_user",
        &["code_sandbox::execute"],
    )
    .await;
    let user_id = Uuid::parse_str(&test_user.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;

    let body = tool_call(
        &server,
        &test_user.token,
        conv_id,
        "execute_command",
        json!({ "command": "env" }),
    )
    .await;
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .expect("stdout");
    assert!(
        !stdout.contains("ZIEE_TEST_SECRET_SENTINEL")
            && !stdout.contains("this-must-not-leak-to-sandbox"),
        "SECURITY FAILURE: server sentinel env leaked into sandbox: {stdout}"
    );
    // Confirm the env DID get the explicit safe defaults we set
    // (HOME, USER, PATH, LANG, LC_ALL, TERM).
    assert!(stdout.contains("USER=sandboxuser"), "USER must be set: {stdout}");
    assert!(stdout.contains("HOME=/home/sandboxuser"), "HOME must be set: {stdout}");
}

/// HIGH regression: write_file size cap (32 MiB). The
/// `WRITE_FILE_MAX_BYTES` constant (commit 8e02fb4) prevents host-disk
/// exhaustion via a single tool call.
#[tokio::test]
async fn e2e_write_file_rejects_oversized_content() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    // 33 MiB content (over the 32 MiB cap).
    let oversized = "x".repeat(33 * 1024 * 1024);
    let resp = post_jsonrpc(
        &server,
        &jwt,
        Some(conv_id),
        "tools/call",
        json!({
            "name": "write_file",
            "arguments": { "filename": "huge.bin", "content": oversized }
        }),
    )
    .await;
    let body: serde_json::Value = resp.json().await.expect("parse");
    assert!(
        body.get("error").is_some(),
        "expected JSON-RPC error envelope, got: {body}"
    );

    // Behavioral assertion: the file MUST NOT exist in the workspace
    // afterward. A follow-up read_file should fail with "not found"
    // (since neither workspace nor attachments contain the file). If
    // the size cap was BROKEN, write_file would have succeeded and
    // read_file would return the 33 MiB back (a much different shape).
    let follow_up = post_jsonrpc(
        &server,
        &jwt,
        Some(conv_id),
        "tools/call",
        json!({ "name": "read_file", "arguments": { "filename": "huge.bin" } }),
    )
    .await;
    let follow_body: serde_json::Value = follow_up.json().await.expect("parse");
    assert!(
        follow_body.get("error").is_some(),
        "SECURITY: huge.bin was written despite the cap. follow-up read_file \
         returned success: {follow_body}"
    );
}

/// HIGH regression: download endpoint must reject path traversal.
/// `canonicalize_in_workspace` is the defense (used since the original
/// port; reverified here through real HTTP).
#[tokio::test]
async fn e2e_download_endpoint_rejects_path_traversal() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let resp = reqwest::Client::new()
        .get(format!(
            "{}/api/code-sandbox/file/download?filename=../../../etc/passwd",
            server.base_url
        ))
        .header("Authorization", format!("Bearer {jwt}"))
        .header("x-conversation-id", conv_id.to_string())
        .send()
        .await
        .expect("send");
    let s = resp.status().as_u16();
    assert!(
        [400, 404].contains(&s),
        "expected 400 (bad filename) or 404 (not found), got {s}"
    );
}

/// CRITICAL regression: cross-tenant conversation_id spoofing.
/// User A has `code_sandbox::execute` (default Users group); they
/// pass User B's conversation_id in the header. Without the
/// `assert_owns_conversation` check (commit 8e02fb4), the call would
/// reach build_context → stage_attachments(other user's files) →
/// /home/sandboxuser/<filename> bind-mount → execute_command can read.
#[tokio::test]
async fn e2e_cross_tenant_conversation_id_blocked() {
    let Some(server) = enabled_test_server().await else { return };

    // User A — the attacker.
    let user_a = test_helpers::create_user_with_permissions(
        &server,
        "tier6_attacker",
        &["code_sandbox::execute"],
    )
    .await;
    // User B — the victim. Create a conversation owned by them.
    let user_b = test_helpers::create_user_with_permissions(
        &server,
        "tier6_victim",
        &["code_sandbox::execute"],
    )
    .await;
    let user_b_id = Uuid::parse_str(&user_b.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let victim_conv_id = create_test_conversation(&pool, user_b_id).await;
    pool.close().await;

    // Positive control: User A → POST with User A's OWN
    // conversation_id MUST succeed. Without this, a regression that
    // returned 404 for ALL conversations (e.g., assert_owns_conversation
    // bug that never finds anything) would silently make the negative
    // case pass for the wrong reason.
    let user_a_id = Uuid::parse_str(&user_a.user_id).unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .unwrap();
    let user_a_conv = create_test_conversation(&pool, user_a_id).await;
    pool.close().await;

    let own_resp = post_jsonrpc(
        &server,
        &user_a.token,
        Some(user_a_conv),
        "tools/list",
        json!({}),
    )
    .await;
    let own_status = own_resp.status().as_u16();
    assert!(
        own_status == 200,
        "POSITIVE CONTROL FAILED: user A calling tools/list on their OWN \
         conversation returned {own_status} (expected 200). This means the \
         negative-case 404 assertion below proves nothing — every call \
         is being rejected, not just cross-tenant."
    );

    // Negative case: User A → POST tools/call with User B's conv_id.
    let resp = post_jsonrpc(
        &server,
        &user_a.token,
        Some(victim_conv_id),
        "tools/call",
        json!({
            "name": "execute_command",
            "arguments": { "command": "cat /home/sandboxuser/* 2>/dev/null | head" }
        }),
    )
    .await;
    let s = resp.status().as_u16();
    let body = resp.text().await.expect("body");
    assert_eq!(
        s, 404,
        "expected 404 (canonical cross-tenant rejection), got {s}: {body}"
    );
    // 200 here would mean cross-tenant access succeeded — SECURITY FAILURE.
}

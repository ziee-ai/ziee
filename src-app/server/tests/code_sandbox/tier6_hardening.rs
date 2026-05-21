//! Tier 6 — hardening tests through the full HTTP path.
//! `#[ignore]`'d; need rootfs + bwrap.

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
        "tier6h_user",
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

/// rlimit NPROC=256 — try to spawn many processes, assert the
/// per-uid limit kicks in. We use a bounded loop (NOT a recursive
/// fork bomb) so the test is deterministic: with NPROC=256 the
/// `for i in $(seq 1 500); do sleep 60 & done` loop CAN'T spawn all
/// 500; we'll see the spawn errors in stderr and the loop completes.
///
/// (Recursive fork bombs are exhibit pathological host behavior
/// when bwrap is in PID-ns-disabled mode — signal propagation
/// through the user-ns mapping is fragile. Tier-4 hardening tests
/// cover the recursive shape directly via bwrap-direct.)
#[tokio::test]
#[ignore]
async fn e2e_nproc_rlimit_enforced_via_http() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        // Try to spawn 500 sleeping subshells. With NPROC=256 some
        // MUST fail to fork; bash reports those as "Resource
        // temporarily unavailable" or exit non-zero. We then kill
        // the sleepers we did manage to spawn.
        json!({ "command": "for i in $(seq 1 500); do (sleep 5) & done 2>&1 | head -c 4000; wait 2>/dev/null; echo DONE-$?" }),
    )
    .await;
    let structured = &body["result"]["structuredContent"];
    let stdout = structured["stdout"].as_str().unwrap_or("");
    let stderr = structured["stderr"].as_str().unwrap_or("");
    // Either the loop succeeded (NPROC didn't apply at all — would
    // be a bug) OR we see fork failures. We assert the FAIL case so
    // the test fails loudly if NPROC isn't enforced.
    let combined = format!("{stdout} {stderr}");
    assert!(
        combined.contains("fork")
            || combined.contains("Resource temporarily unavailable")
            || combined.contains("retry"),
        "expected fork-limit errors with NPROC=256, got stdout={stdout:?} stderr={stderr:?}"
    );
}

/// rlimit AS=4 GiB — allocate beyond should fail (or be killed),
/// not hang or OOM the host.
#[tokio::test]
#[ignore]
async fn e2e_memory_bomb_bounded_by_as_rlimit() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        // Try to allocate 10 GiB — must fail or get killed.
        json!({ "command": "python3 -c 'x=bytearray(10*1024**3)' || echo BOUNDED" }),
    )
    .await;
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    let stderr = body["result"]["structuredContent"]["stderr"]
        .as_str()
        .unwrap_or("");
    assert!(
        stdout.contains("BOUNDED")
            || stderr.contains("MemoryError")
            || body["result"]["structuredContent"]["exit_code"]
                .as_i64()
                .unwrap_or(0)
                != 0,
        "memory bomb must NOT silently succeed — got stdout={stdout} stderr={stderr}"
    );
}

// NOTE: wall-clock-timeout test deliberately omitted from Tier 6.
// The default sandbox timeout is 600s and there's currently no
// per-call timeout argument exposed via the tools/call schema, so a
// proper E2E timeout test would take ~10 minutes to run. The Tier 4
// hardening tests already exercise the timeout path bwrap-direct
// with a short timeout via the test driver. If a per-call timeout
// argument is added later, re-introduce this test with the short
// budget here.

/// Output cap 1 MiB. `yes` piped through `head -c 10MB` must yield
/// 1 MiB of captured stdout + truncation marker + `stdout_truncated: true`.
#[tokio::test]
#[ignore]
async fn e2e_output_cap_truncates_stdout_at_one_mib() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "yes | head -c 10485760" }),
    )
    .await;
    let structured = &body["result"]["structuredContent"];
    let stdout = structured["stdout"].as_str().unwrap();
    let truncated = structured["stdout_truncated"].as_bool().unwrap();
    // 1 MiB + truncation marker.
    assert!(
        stdout.len() <= 1024 * 1024 + 200,
        "stdout {} bytes exceeds cap+marker",
        stdout.len()
    );
    assert!(truncated, "stdout_truncated should be true");
}

/// Boot sanity: synthetic /etc/passwd shows only sandboxuser.
#[tokio::test]
#[ignore]
async fn e2e_etc_passwd_is_synthetic() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "cat /etc/passwd" }),
    )
    .await;
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap();
    assert!(
        stdout.contains("sandboxuser"),
        "expected sandboxuser in synthetic passwd: {stdout}"
    );
    // Must NOT contain host's root user or any host-specific account.
    assert!(
        !stdout.contains("pbya"),
        "host user pbya leaked into sandbox /etc/passwd: {stdout}"
    );
}

/// rootfs /usr is read-only — try to write must fail.
#[tokio::test]
#[ignore]
async fn e2e_usr_is_readonly() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "touch /usr/sandbox-write-test 2>&1; echo done=$?" }),
    )
    .await;
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap();
    // touch must have failed (exit != 0) — "done=0" would mean writable.
    assert!(
        !stdout.contains("done=0"),
        "/usr must be read-only inside sandbox, got: {stdout}"
    );
}

/// Sandbox uid is non-root (1001).
#[tokio::test]
#[ignore]
async fn e2e_sandbox_uid_is_1001() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "id -u && id -un" }),
    )
    .await;
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap();
    assert!(stdout.contains("1001"), "expected uid 1001: {stdout}");
    assert!(stdout.contains("sandboxuser"), "expected name sandboxuser: {stdout}");
}

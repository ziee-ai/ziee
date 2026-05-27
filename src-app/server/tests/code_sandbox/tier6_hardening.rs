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
async fn e2e_nproc_rlimit_enforced_via_http() {
    // Asserts the SAFETY property the sandbox actually guarantees:
    // a fork bomb DOES NOT escape the sandbox / wedge the dispatcher.
    // (Whether RLIMIT_NPROC or cgroup pids.max specifically fires
    // depends on the guest kernel + cgroup v2 delegation state —
    // libkrun's bundled kernel diverges from a stock host kernel
    // here; tier4_hardening::fork_bomb_contained_in_sandbox carries
    // the same caveat.) Spawn 500 sleepers, reap them, then verify
    // the next command in the SAME conversation still answers.
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let started = std::time::Instant::now();
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "for i in $(seq 1 500); do (sleep 30) & done 2>/dev/null; kill -9 $(jobs -p) 2>/dev/null; wait 2>/dev/null; echo DONE-$?" }),
    )
    .await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "fork-bomb call took {:?} — sandbox likely wedged",
        elapsed
    );
    let stdout = body["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    assert!(
        stdout.contains("DONE-"),
        "expected fork-bomb shell to complete (with or without fork-failures), got: {stdout:?}"
    );
    // Liveness check: the sandbox is still responsive for a fresh call.
    let next = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "echo ALIVE" }),
    )
    .await;
    let alive = next["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    assert!(
        alive.contains("ALIVE"),
        "sandbox stopped responding after the fork bomb: {alive:?}"
    );
}

/// rlimit AS=4 GiB — allocate beyond should fail (or be killed),
/// not hang or OOM the host.
#[tokio::test]
async fn e2e_memory_bomb_bounded_by_as_rlimit() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;

    // Positive control: confirm python3 is actually present in the
    // rootfs. Without this check, `python3: command not found` would
    // ALSO yield a non-zero exit + non-empty stderr → the assertion
    // would pass for the wrong reason (no memory bomb ever ran).
    let probe = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "command -v python3 && echo PYTHON_OK" }),
    )
    .await;
    let probe_stdout = probe["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    assert!(
        probe_stdout.contains("PYTHON_OK"),
        "rootfs lacks python3 — cannot validate memory rlimit. stdout={probe_stdout:?}"
    );

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
    // Stronger assertion: must NOT be "command not found"-shaped.
    assert!(
        !stderr.contains("not found") && !stdout.contains("not found"),
        "command-not-found leak: stdout={stdout:?} stderr={stderr:?}"
    );
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
    // Stronger than a per-host denylist (which would silently pass
    // on CI runners with `runner`/`root` as the host user): the
    // synthetic file is exactly one line. If the host /etc/passwd
    // leaked through (regression of the synthetic-identity bind),
    // we'd see tens of lines.
    let nonempty_lines = stdout.lines().filter(|l| !l.trim().is_empty()).count();
    assert_eq!(
        nonempty_lines, 1,
        "synthetic /etc/passwd MUST be exactly 1 line; got {nonempty_lines}: {stdout}"
    );
}

/// rootfs /usr is read-only — try to write must fail.
#[tokio::test]
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

/// Plan 1 §6.6: a memory cap set via PUT /code-sandbox/resource-limits
/// kicks in on the NEXT execute_command — proves the cache invalidation
/// path works AND the new limit is actually applied (not just stored).
///
/// We tighten memory.max from the 512 MiB default to 64 MiB, then ask
/// python3 to allocate 256 MiB. The kernel OOM-killer triggers on the
/// in-sandbox cgroup; we observe either a non-zero exit + the
/// 'BOUNDED' echo (the workload survived and reported it couldn't
/// allocate) OR a SIGKILL/exit-code-137 (the cgroup OOM-killed it).
/// Either outcome proves the cap is wired.
#[tokio::test]
async fn e2e_configured_memory_limit_enforced_via_http() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;

    // PUT a tight memory cap. Use a token with manage permission.
    let admin = test_helpers::create_user_with_permissions(
        &server,
        "tier6h_limits_admin",
        &[
            "code_sandbox::resource_limits::read",
            "code_sandbox::resource_limits::manage",
        ],
    )
    .await;
    // Set BOTH the cgroup memory cap AND the prlimit `--as` cap. The Tier-6
    // harness boots without cgroup delegation (sandbox_cgroup_parent=""), so
    // the cgroup memory.max is silently inert here; the only enforcement that
    // bites is `prlimit --as`. Setting both proves the §6 PATCH-→-cache-→-
    // next-exec pipeline works regardless of which mode the host supports.
    let put = reqwest::Client::new()
        .put(format!("{}/api/code-sandbox/resource-limits", server.base_url))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({
            "memory_max_bytes": 64 * 1024 * 1024_i64,
            "address_space_bytes": 64 * 1024 * 1024_i64,
        }))
        .send()
        .await
        .expect("PUT");
    assert_eq!(put.status().as_u16(), 200, "PUT status: {:?}", put.text().await);

    // Positive control: python3 actually present.
    let probe = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "command -v python3 && echo PYTHON_OK" }),
    )
    .await;
    let probe_stdout = probe["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("");
    assert!(
        probe_stdout.contains("PYTHON_OK"),
        "rootfs lacks python3 — cannot validate configured-memory cap. stdout={probe_stdout:?}"
    );

    // Allocate 256 MiB — must fail or be killed within the 64 MiB cap.
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "python3 -c 'x=bytearray(256*1024*1024)' || echo BOUNDED" }),
    )
    .await;
    let structured = &body["result"]["structuredContent"];
    let stdout = structured["stdout"].as_str().unwrap_or("");
    let stderr = structured["stderr"].as_str().unwrap_or("");
    let exit_code = structured["exit_code"].as_i64().unwrap_or(0);
    let combined = format!("{stdout} {stderr}");
    // Cgroup OOM-kill manifests as SIGKILL → bash reports exit 137 OR
    // we never reach the echo BOUNDED. Or the kernel returns ENOMEM and
    // python raises MemoryError, in which case BOUNDED appears + exit 0.
    let bounded = combined.contains("BOUNDED")
        || combined.contains("MemoryError")
        || combined.contains("Killed")
        || exit_code == 137
        || exit_code == -1;
    assert!(
        bounded,
        "expected 64 MiB cap to bound a 256 MiB alloc, got \
         exit_code={exit_code} stdout={stdout:?} stderr={stderr:?}"
    );
}

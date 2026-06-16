//! Tier 6 — end-to-end rootfs **version swap** behavior.
//!
//! Boots a real server (pinned to the test tag), runs a command to
//! populate the per-conversation workspace, POSTs the admin `set-pin`
//! endpoint to swap the rootfs version, then verifies the documented
//! swap policy:
//!
//!   * MAJOR bump (e.g. 0.0.x → 1.x) → the install-cache subdirs
//!     (`.cache`, `.npm`, `.cargo`, …) are WIPED across the workspace
//!     after drain, generated files are preserved, and the next tool
//!     call carries a `system_note` telling the LLM to reinstall.
//!   * MINOR/PATCH bump (same major) → caches are PRESERVED, no note.
//!
//! Swap targets are arch-resolved via `harness::{major_bump_target,
//! patch_bump_target}`: the patch bump uses `v0.0.6-alpha` (published
//! for x86_64 AND aarch64); the major bump uses `v1.0.0-alpha` on
//! x86_64 and skips on arches with no higher-major rootfs published
//! (aarch64 today). Needs network + a runnable sandbox backend. Run with:
//!   cargo test --test integration_tests -- --test-threads=1 \
//!     code_sandbox::tier6_version_swap

#![allow(unused_imports)]

use std::time::Duration;

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use crate::code_sandbox::harness::{
    create_test_conversation, enabled_test_server, tool_call,
};
use crate::common::{test_helpers, TestServer};

/// Register a user with both execute (run commands) and
/// environments::manage (set the pin), and create an owned
/// conversation. Returns (jwt, conversation_id).
async fn setup_user_and_conv(server: &TestServer) -> (String, Uuid) {
    let user = test_helpers::create_user_with_permissions(
        server,
        "swap_user",
        &[
            "code_sandbox::execute",
            "code_sandbox::environments::read",
            "code_sandbox::environments::manage",
        ],
    )
    .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("user uuid");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;
    (user.token, conv_id)
}

/// POST the admin set-pin endpoint. Returns the HTTP status + parsed body.
async fn set_pin(server: &TestServer, jwt: &str, version: &str) -> (u16, serde_json::Value) {
    let resp = reqwest::Client::new()
        .post(format!(
            "{}/api/code-sandbox/rootfs/versions/set-pin",
            server.base_url
        ))
        .header("Authorization", format!("Bearer {jwt}"))
        .json(&json!({ "version": version }))
        .send()
        .await
        .expect("set-pin request");
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or(json!(null));
    (status, body)
}

fn stdout_of(resp: &serde_json::Value) -> String {
    resp["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

// =====================================================================
// MAJOR bump → wipe install caches, preserve user files, system note
// =====================================================================

#[tokio::test]
async fn e2e_major_version_bump_wipes_caches_and_notes() {
    // A real major bump needs a published rootfs whose semver major
    // differs from the v0.0.x base. Only x86_64 ships one today
    // (v1.0.0-alpha); aarch64 tops out at v0.0.6-alpha, so this test
    // skips cleanly there (published-asset gap, not a backend defect).
    let Some(major_target) = crate::code_sandbox::harness::major_bump_target() else {
        eprintln!(
            "tier6 skipped: no major-version rootfs published for {} \
             (cannot exercise a real major-bump swap on this arch)",
            crate::code_sandbox::harness::test_arch_token()
        );
        return;
    };
    let Some(server) = enabled_test_server().await else { return };
    let (jwt, conv_id) = setup_user_and_conv(&server).await;

    // 1. Populate the workspace: a cache subdir that MUST be wiped on a
    //    major bump + a plain file that MUST survive. This first call
    //    also triggers the v0.0.3 download + cosign verify + mount.
    let plant = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "mkdir -p ~/.cache/marker ~/.npm && echo hi > ~/.cache/marker/f && echo KEEP > ~/keep.txt && ls ~/.cache && cat ~/keep.txt" }),
    )
    .await;
    assert!(
        stdout_of(&plant).contains("KEEP"),
        "setup failed to plant files: {plant:#}"
    );

    // 2. Swap to the major-bump release (0.x → 1.x). The semver major
    //    changes, so set_pin_with_drain schedules a drain-then-wipe.
    let (status, body) = set_pin(&server, &jwt, major_target).await;
    assert_eq!(status, 200, "set-pin to {major_target} failed: {body:#}");
    assert_eq!(
        body["swap"]["cache_wipe"].as_str(),
        Some("wipe_caches_on_drain"),
        "expected a major-bump wipe policy: {body:#}"
    );

    // 3. The wipe runs asynchronously in the drain task (after the prior
    //    exec's inflight guard dropped). Poll a read-only command until
    //    the wipe sentinel surfaces as a `system_note`, then assert.
    let mut last = json!(null);
    let mut wiped = false;
    for _ in 0..20 {
        let probe = tool_call(
            &server,
            &jwt,
            conv_id,
            "execute_command",
            json!({ "command": "ls ~/.cache 2>&1; echo ---; cat ~/keep.txt 2>&1" }),
        )
        .await;
        let note = probe["result"]["structuredContent"]["system_note"]
            .as_str()
            .unwrap_or("");
        if !note.is_empty() {
            last = probe;
            wiped = true;
            break;
        }
        last = probe;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    assert!(
        wiped,
        "no rootfs-upgrade system_note after major bump within 20s: {last:#}"
    );
    let note = last["result"]["structuredContent"]["system_note"]
        .as_str()
        .unwrap_or("");
    assert!(
        note.to_lowercase().contains("rootfs") || note.to_lowercase().contains("upgrad"),
        "system_note should mention the rootfs upgrade, got: {note:?}"
    );
    let out = stdout_of(&last);
    // The `.cache` install-cache subdir was wiped (the `marker` we
    // planted is gone)...
    assert!(
        !out.contains("marker"),
        "~/.cache/marker should have been wiped on major bump, got: {out:?}"
    );
    // ...but the user's generated file survived.
    assert!(
        out.contains("KEEP"),
        "~/keep.txt should be preserved across the bump, got: {out:?}"
    );
}

// =====================================================================
// MINOR/PATCH bump → caches preserved, no wipe, no note
// =====================================================================

#[tokio::test]
async fn e2e_patch_version_bump_preserves_caches() {
    let Some(server) = enabled_test_server().await else { return };
    let (jwt, conv_id) = setup_user_and_conv(&server).await;

    let plant = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "mkdir -p ~/.cache/marker && echo hi > ~/.cache/marker/f && ls ~/.cache" }),
    )
    .await;
    assert!(stdout_of(&plant).contains("marker"), "setup failed: {plant:#}");

    // Same-major bump (base → patch_bump_target, both major 0) → preserve
    // policy. Target is published for every supported test arch.
    let patch_target = crate::code_sandbox::harness::patch_bump_target();
    let (status, body) = set_pin(&server, &jwt, patch_target).await;
    assert_eq!(status, 200, "set-pin to {patch_target} failed: {body:#}");
    assert_eq!(
        body["swap"]["cache_wipe"].as_str(),
        Some("preserve"),
        "expected a preserve policy on a patch bump: {body:#}"
    );

    // Give any (non-wipe) drain a moment, then confirm the cache marker
    // is still there and no upgrade note fired.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let probe = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "ls ~/.cache 2>&1" }),
    )
    .await;
    assert!(
        stdout_of(&probe).contains("marker"),
        "~/.cache/marker must survive a patch bump: {probe:#}"
    );
    assert!(
        probe["result"]["structuredContent"]["system_note"].is_null(),
        "no system_note expected on a patch bump: {probe:#}"
    );
}

//! Tier 6 — full HTTP-E2E happy-path tests.
//!
//! Boots a real TestServer with `code_sandbox.enabled: true`, points
//! it at a mounted rootfs, posts real JSON-RPC requests over reqwest.
//! The handler runs `build_bwrap_argv` + spawns real bwrap; the
//! response carries the real command output. This is the ONLY tier
//! that exercises the full production code path; everything below it
//! is either mocked (Tier 3 doesn't enable sandbox) or parallel-harness
//! (Tier 4 has its own bwrap argv that drifts from production).
//!
//! All tests `#[ignore]`'d. Run with:
//!   cargo test --test integration_tests -- --test-threads=1 \
//!     --ignored code_sandbox::tier6_

#![allow(unused_imports)]

use crate::code_sandbox::harness::{
    create_test_conversation, enabled_test_server, needs_full_rootfs, post_jsonrpc,
    test_server_jwt, tool_call, tool_call_with_timeout,
};
use crate::common::test_helpers;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

/// Helper: register a user via API, create an owned conversation,
/// return (user_id, jwt, conversation_id). Each tier-6 test starts
/// with this.
async fn setup_user_and_conv(server: &crate::common::TestServer) -> (Uuid, String, Uuid) {
    // Use the existing common helper that registers a user via API
    // and grants the requested permissions through a test group.
    let test_user = test_helpers::create_user_with_permissions(
        server,
        "tier6_user",
        &["code_sandbox::execute"],
    )
    .await;
    let user_id = Uuid::parse_str(&test_user.user_id).expect("user uuid");

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");
    let conv_id = create_test_conversation(&pool, user_id).await;
    pool.close().await;

    (user_id, test_user.token, conv_id)
}

// ─────────────────────────────────────────────────────────────────────
// 6d — Boot / init E2E
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_initialize_returns_protocol_version_and_server_info() {
    let Some(server) = enabled_test_server().await else { return };
    // initialize is a STATELESS MCP method — must work for any
    // authenticated user without an ownership-checked conversation
    // context. Use a registered user so the result body has real
    // content to assert against (not just a 401).
    let (_user_id, jwt, _conv_id) = setup_user_and_conv(&server).await;

    let resp = post_jsonrpc(&server, &jwt, None, "initialize", json!({})).await;
    assert!(resp.status().is_success(), "got {}", resp.status());
    let body: serde_json::Value = resp.json().await.expect("parse");
    let result = body
        .get("result")
        .expect("initialize MUST return a result envelope");
    // Spec-required fields.
    assert_eq!(
        result["protocolVersion"].as_str(),
        Some("2025-11-25"),
        "protocolVersion changed unexpectedly: {result}"
    );
    let server_info = &result["serverInfo"];
    assert_eq!(
        server_info["name"].as_str(),
        Some("code_sandbox"),
        "serverInfo.name regressed: {server_info}"
    );
    assert!(
        server_info["version"].is_string(),
        "serverInfo.version must be a string: {server_info}"
    );
    assert!(
        result["capabilities"]["tools"].is_object(),
        "capabilities.tools must be an object: {result}"
    );
}

#[tokio::test]
async fn e2e_tools_list_returns_seven_tools_via_real_server() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, _conv_id) = setup_user_and_conv(&server).await;
    let resp = post_jsonrpc(&server, &jwt, None, "tools/list", json!({})).await;
    assert!(resp.status().is_success(), "got {}", resp.status());
    let body: serde_json::Value = resp.json().await.expect("parse");
    let tools = body
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .expect("result.tools array");
    assert_eq!(tools.len(), 7, "expected exactly 7 tools, got {tools:?}");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"execute_command"));
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"edit_file"));
    assert!(names.contains(&"list_files"));
    assert!(names.contains(&"get_resource_link"));
    assert!(names.contains(&"list_sandbox_environments"));
}

#[tokio::test]
async fn e2e_method_not_found_returns_minus_32601() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let resp = post_jsonrpc(&server, &jwt, Some(conv_id), "tools/nonexistent", json!({})).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = resp.json().await.expect("parse");
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_i64())
        .expect("error.code");
    assert_eq!(code, -32601, "JSON-RPC method-not-found = -32601");
}

// ─────────────────────────────────────────────────────────────────────
// 6a — Happy-path coverage (one test per tool, real bwrap)
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn e2e_execute_command_echo_hello_returns_stdout() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "echo hello-from-e2e" }),
    )
    .await;
    let structured = body
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .unwrap_or_else(|| panic!("result.structuredContent missing — full body: {body:#?}"));
    let stdout = structured["stdout"].as_str().expect("stdout str");
    let stderr = structured.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
    let exit_code = structured.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(-1);
    assert!(
        stdout.contains("hello-from-e2e"),
        "expected echo output. stdout={stdout:?} stderr={stderr:?} exit={exit_code}"
    );
    assert_eq!(structured["exit_code"].as_i64().unwrap(), 0);
    assert!(!structured["timed_out"].as_bool().unwrap());
}

/// Fix C regression: R must actually run in the `full` flavor. The reported bug
/// was `Rscript` failing with `libblas.so.3: cannot open shared object file`
/// (and a missing `/usr/lib/R/etc/ldpaths`) because `mmdebstrap --variant=minbase`
/// installs with Recommends disabled, so r-base-core's BLAS/LAPACK runtime was
/// never pulled in. The recipe now depends on `libopenblas0` explicitly.
///
/// Skipped unless a FULL rootfs is mounted (R isn't in the `minimal` test
/// rootfs): gated on `needs_full_rootfs()` (set ZIEE_SANDBOX_FLAVOR=minimal to
/// skip) plus the usual `enabled_test_server()` rootfs/bwrap check.
#[tokio::test]
async fn e2e_full_rootfs_rscript_runs_without_blas_error() {
    if !needs_full_rootfs() {
        return;
    }
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    // Test both: (1) R starts at all (the original libblas/ldpaths failure mode),
    // and (2) a BLAS-using package actually loads (ggplot2 is installed by the
    // recipe's `provision` step and pulls in matrix routines through its deps —
    // strictly stronger evidence that BLAS is wired in correctly, not just present).
    let body = tool_call_with_timeout(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        // flavor="full" is REQUIRED — R isn't in the minimal rootfs; without
        // this the runtime defaults to mounting minimal and Rscript isn't found.
        // Mirrors the original failing transcript which also passed flavor=full.
        json!({
            "command": "Rscript -e 'suppressMessages(library(ggplot2)); cat(1 + 1)'",
            "flavor": "full",
        }),
        // The full rootfs is ~900 MB; the FIRST (cold-cache) call downloads +
        // verifies + mounts it, which blows past the default 120 s. Give the
        // cold fetch room — once cached, later runs are fast.
        std::time::Duration::from_secs(900),
    )
    .await;
    let structured = body
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .unwrap_or_else(|| panic!("result.structuredContent missing — full body: {body:#?}"));
    let stdout = structured["stdout"].as_str().unwrap_or("");
    let stderr = structured.get("stderr").and_then(|s| s.as_str()).unwrap_or("");
    let exit_code = structured.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(-1);

    assert!(
        !stderr.contains("libblas") && !stderr.contains("ldpaths"),
        "R must find its BLAS/LAPACK runtime (Fix C regression). stderr={stderr:?}"
    );
    assert!(
        !stderr.contains("there is no package called"),
        "ggplot2 must be installed by the recipe's provision step. stderr={stderr:?}"
    );
    assert_eq!(
        exit_code, 0,
        "Rscript should exit 0. stdout={stdout:?} stderr={stderr:?}"
    );
    assert_eq!(
        stdout.trim(),
        "2",
        "R should compute 1+1=2 after loading ggplot2. stdout={stdout:?} stderr={stderr:?}"
    );
}

#[tokio::test]
async fn e2e_write_file_then_read_file_round_trip() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    // Write
    let write_body = tool_call(
        &server,
        &jwt,
        conv_id,
        "write_file",
        json!({ "filename": "hello.txt", "content": "line one\nline two\n" }),
    )
    .await;
    let write_structured = write_body["result"]["structuredContent"].clone();
    assert!(write_structured["success"].as_bool().unwrap());
    assert_eq!(write_structured["bytes_written"].as_u64().unwrap(), 18);

    // Read
    let read_body = tool_call(
        &server,
        &jwt,
        conv_id,
        "read_file",
        json!({ "filename": "hello.txt" }),
    )
    .await;
    let text = read_body["result"]["structuredContent"]["text"]
        .as_str()
        .expect("text");
    // read_file returns numbered lines: "1: line one\n2: line two\n"
    assert!(text.contains("1: line one"), "got: {text}");
    assert!(text.contains("2: line two"), "got: {text}");
    assert_eq!(
        read_body["result"]["structuredContent"]["total_lines"]
            .as_u64()
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn e2e_read_file_slice_returns_requested_range() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(
        &server,
        &jwt,
        conv_id,
        "write_file",
        json!({ "filename": "n.txt", "content": "a\nb\nc\nd\ne\n" }),
    )
    .await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "read_file",
        json!({ "filename": "n.txt", "start_line": 2, "end_line": 4 }),
    )
    .await;
    let text = body["result"]["structuredContent"]["text"].as_str().unwrap();
    assert!(text.contains("2: b") && text.contains("3: c") && text.contains("4: d"), "got: {text}");
    assert!(!text.contains("1: a") && !text.contains("5: e"), "got: {text}");
}

#[tokio::test]
async fn e2e_edit_file_replaces_inner_range() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(
        &server,
        &jwt,
        conv_id,
        "write_file",
        json!({ "filename": "e.txt", "content": "one\ntwo\nthree\nfour\nfive\n" }),
    )
    .await;
    tool_call(
        &server,
        &jwt,
        conv_id,
        "edit_file",
        json!({
            "filename": "e.txt",
            "start_line": 2,
            "end_line": 3,
            "new_content": "REPLACED-2\nREPLACED-3",
        }),
    )
    .await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "read_file",
        json!({ "filename": "e.txt" }),
    )
    .await;
    let text = body["result"]["structuredContent"]["text"].as_str().unwrap();
    assert!(text.contains("REPLACED-2"), "got: {text}");
    assert!(text.contains("REPLACED-3"), "got: {text}");
    assert!(text.contains("4: four"), "got: {text}");
}

#[tokio::test]
async fn e2e_edit_file_appends_at_len_plus_one() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(
        &server,
        &jwt,
        conv_id,
        "write_file",
        json!({ "filename": "a.txt", "content": "first\nsecond\n" }),
    )
    .await;
    // 2 lines, so start_line=3 (len+1) means append.
    tool_call(
        &server,
        &jwt,
        conv_id,
        "edit_file",
        json!({
            "filename": "a.txt",
            "start_line": 3,
            "end_line": 3,
            "new_content": "appended-third",
        }),
    )
    .await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "read_file",
        json!({ "filename": "a.txt" }),
    )
    .await;
    let text = body["result"]["structuredContent"]["text"].as_str().unwrap();
    assert!(text.contains("appended-third"), "got: {text}");
    assert!(text.contains("1: first"), "got: {text}");
}

#[tokio::test]
async fn e2e_list_files_shows_written_files_hides_dotfiles() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(&server, &jwt, conv_id, "write_file", json!({"filename":"visible1.txt","content":"a"})).await;
    tool_call(&server, &jwt, conv_id, "write_file", json!({"filename":"visible2.txt","content":"b"})).await;
    tool_call(&server, &jwt, conv_id, "write_file", json!({"filename":".env","content":"SECRET=x"})).await;
    let body = tool_call(&server, &jwt, conv_id, "list_files", json!({})).await;
    let files = body["result"]["structuredContent"]["files"]
        .as_array()
        .expect("files array");
    let names: Vec<&str> = files.iter().filter_map(|f| f["name"].as_str()).collect();
    assert!(names.contains(&"visible1.txt"), "{names:?}");
    assert!(names.contains(&"visible2.txt"), "{names:?}");
    assert!(!names.contains(&".env"), ".env (dotfile) MUST be hidden: {names:?}");
}

#[tokio::test]
async fn e2e_get_resource_link_for_workspace_artifact_returns_ziee_uri() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(&server, &jwt, conv_id, "write_file", json!({"filename":"art.txt","content":"x"})).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "get_resource_link",
        json!({ "filename": "art.txt" }),
    )
    .await;
    // The resource_link block is in content[0] (passes through
    // mcp_content_blocks) AND in structuredContent.
    let link = &body["result"]["structuredContent"];
    assert_eq!(link["type"].as_str().unwrap(), "resource_link");
    let uri = link["uri"].as_str().expect("uri");
    // Transient workspace artifacts now emit `ziee://<host_abs_path>` — a read-once,
    // in-process hint that the chat/workflow consumer reads off disk + ingests, then
    // strips (rewrites to /api/files/{id}). This raw MCP call sees it pre-stripping.
    assert!(uri.starts_with("ziee://"), "transient artifact uri must be ziee://: {uri}");
    assert!(uri.ends_with("art.txt"), "uri must point at the artifact file: {uri}");
    assert!(!link["is_saved"].as_bool().unwrap(), "workspace artifact is NOT saved");
}

/// Same flow with `code_sandbox.public_base_url` configured. Transient workspace
/// artifacts are now consumed in-process off disk via `ziee://<host_abs_path>`, so the
/// public origin is irrelevant to them: the URI must be `ziee://` and must NOT embed the
/// public host. (public_base_url still roots the download-with-token URL of is_saved:true
/// user attachments, which take the other branch of get_resource_link.)
#[tokio::test]
async fn e2e_get_resource_link_transient_artifact_ignores_public_base_url() {
    use crate::code_sandbox::harness::github_fetch_server_options;
    let Some(mut opts) = github_fetch_server_options(Vec::new()) else { return };
    opts.sandbox_public_base_url = Some("https://public.example.test".to_string());
    let server = crate::common::TestServer::start_with_options(opts).await;

    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(&server, &jwt, conv_id, "write_file", json!({"filename":"art.txt","content":"x"})).await;
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "get_resource_link",
        json!({ "filename": "art.txt" }),
    )
    .await;
    let link = &body["result"]["structuredContent"];
    assert_eq!(link["type"].as_str().unwrap(), "resource_link");
    let uri = link["uri"].as_str().expect("uri");
    assert!(uri.starts_with("ziee://"), "transient artifact uri must be ziee://: {uri}");
    assert!(
        !uri.contains("public.example.test"),
        "ziee:// uri must not embed the public origin: {uri}"
    );
    assert!(uri.ends_with("art.txt"), "uri: {uri}");
}

#[tokio::test]
async fn e2e_download_endpoint_returns_workspace_file_bytes() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;
    tool_call(
        &server,
        &jwt,
        conv_id,
        "write_file",
        json!({ "filename": "dl.txt", "content": "the-bytes-we-expect" }),
    )
    .await;
    let resp = reqwest::Client::new()
        .get(format!(
            "{}/api/code-sandbox/file/download?filename=dl.txt",
            server.base_url
        ))
        .header("Authorization", format!("Bearer {jwt}"))
        .header("x-conversation-id", conv_id.to_string())
        .send()
        .await
        .expect("send");
    assert!(resp.status().is_success(), "{}", resp.status());
    let bytes = resp.bytes().await.expect("body");
    assert_eq!(&bytes[..], b"the-bytes-we-expect");
}

// ─────────────────────────────────────────────────────────────────────
// 6e — Large output (streaming + OUTPUT_CAP_BYTES) + multi-user contention
// ─────────────────────────────────────────────────────────────────────

/// A command that emits more than the 1 MiB output cap must come back with
/// `stdout_truncated: true` and a bounded stdout — exercising the streaming
/// capture + cap path end-to-end rather than buffering unboundedly.
#[tokio::test]
async fn e2e_execute_command_large_output_is_capped_and_flagged() {
    let Some(server) = enabled_test_server().await else { return };
    let (_user_id, jwt, conv_id) = setup_user_and_conv(&server).await;

    // ~2 MiB of 'A' on stdout — well over the 1 MiB OUTPUT_CAP_BYTES.
    let body = tool_call(
        &server,
        &jwt,
        conv_id,
        "execute_command",
        json!({ "command": "head -c 2097152 /dev/zero | tr '\\0' 'A'" }),
    )
    .await;
    let structured = body
        .get("result")
        .and_then(|r| r.get("structuredContent"))
        .unwrap_or_else(|| panic!("structuredContent missing — body: {body:#?}"));

    assert_eq!(
        structured["stdout_truncated"].as_bool(),
        Some(true),
        "2 MiB of stdout must set stdout_truncated=true: {structured:#?}"
    );
    let stdout_len = structured["stdout"].as_str().map(|s| s.len()).unwrap_or(0);
    assert!(
        stdout_len > 0 && stdout_len <= 1_100_000,
        "captured stdout must be bounded near the 1 MiB cap, got {stdout_len} bytes"
    );
    assert!(
        !structured["timed_out"].as_bool().unwrap_or(true),
        "the large-output command should complete, not time out"
    );
}

/// Two DIFFERENT users running commands in their OWN conversations concurrently
/// must both succeed with their own correct, non-cross-contaminated output —
/// covering multi-user sandbox access under contention (each gets an isolated
/// workspace).
#[tokio::test]
async fn e2e_concurrent_multi_user_sandbox_isolated() {
    let Some(server) = enabled_test_server().await else { return };

    async fn setup_named(
        server: &crate::common::TestServer,
        name: &str,
    ) -> (String, Uuid) {
        let u = test_helpers::create_user_with_permissions(
            server,
            name,
            &["code_sandbox::execute"],
        )
        .await;
        let user_id = Uuid::parse_str(&u.user_id).expect("user uuid");
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&server.database_url)
            .await
            .expect("connect test db");
        let conv_id = create_test_conversation(&pool, user_id).await;
        pool.close().await;
        (u.token, conv_id)
    }

    let (jwt_a, conv_a) = setup_named(&server, "tier6_multiuser_a").await;
    let (jwt_b, conv_b) = setup_named(&server, "tier6_multiuser_b").await;

    // Fire both users' commands concurrently; each echoes a user-specific token.
    let fut_a = tool_call(
        &server,
        &jwt_a,
        conv_a,
        "execute_command",
        json!({ "command": "echo USER_A_MARKER_42" }),
    );
    let fut_b = tool_call(
        &server,
        &jwt_b,
        conv_b,
        "execute_command",
        json!({ "command": "echo USER_B_MARKER_99" }),
    );
    let (body_a, body_b) = tokio::join!(fut_a, fut_b);

    let out = |b: &serde_json::Value| -> String {
        b.get("result")
            .and_then(|r| r.get("structuredContent"))
            .and_then(|s| s.get("stdout"))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    let out_a = out(&body_a);
    let out_b = out(&body_b);

    assert!(out_a.contains("USER_A_MARKER_42"), "user A output wrong: {out_a:?}");
    assert!(out_b.contains("USER_B_MARKER_99"), "user B output wrong: {out_b:?}");
    // No cross-contamination between the two concurrent workspaces.
    assert!(!out_a.contains("USER_B_MARKER_99"), "A leaked B's output");
    assert!(!out_b.contains("USER_A_MARKER_42"), "B leaked A's output");
}

//! Tier 3 — the chat-path `execute_command` **flavor enum** is enforced.
//!
//! `execute_command`'s tool schema advertises
//! `"flavor": {"enum": ["minimal","full"]}` and the server enforced it nowhere:
//! any non-empty string was passed through to `execute_command_stream` →
//! `runtime_fetch::ensure_fetched` → `version_manager::install_version` →
//! `format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` → a live GitHub
//! Releases request that 404s. A model invented `"zee-workflow"` and it reached
//! the network.
//!
//! ## This test must never itself reach the network
//!
//! It boots a sandbox-ENABLED server whose configured rootfs dir is EMPTY —
//! `code_sandbox` init defers every rootfs-dependent probe to the first
//! `execute_command`, so the module registers anyway, which is the strictest
//! place to prove a refusal lands before anything touches a rootfs.
//!
//! The corollary is a trap this test fell into once and must not again: on that
//! server, a call with a VALID flavor is NOT harmless. `execute_command_stream`
//! `tokio::spawn`s its work before responding, the harness writes
//! `require_download_consent: false`, and the spawned task therefore proceeds to
//! `ensure_pin_initialized` (api.github.com) and `install_version` — a 57 MB or
//! 853 MB download that dropping the HTTP response does NOT cancel. So this file
//! **never invokes `execute_command` with an accepted flavor.** The accepted
//! case is proven exhaustively, and for free, by the pure-function unit test
//! `resolve_execute_flavor` in `code_sandbox/handlers.rs`; what belongs HERE is
//! the wiring — that the refusal really is reached over real HTTP — plus a
//! positive control that does not execute anything.

use crate::common::{TestServer, TestServerOptions};

/// A sandbox-ENABLED server whose configured rootfs dir is empty. Returns `None`
/// (skip) when the host cannot register the sandbox at all.
///
/// The bwrap probe is NOT `#[cfg]`-gated away on other platforms: compiling the
/// skip out of macOS/Windows (an earlier revision did) does not make the test
/// pass there — it makes it PANIC on the first `expect` instead of skipping.
async fn sandbox_server_without_rootfs() -> Option<(TestServer, tempfile::TempDir)> {
    if !crate::code_sandbox::harness::bwrap_available() {
        eprintln!("test skipped: bwrap not available on this host");
        return None;
    }
    let rootfs = tempfile::tempdir().expect("rootfs TempDir");
    let opts = TestServerOptions {
        sandbox_enabled: true,
        sandbox_rootfs: Some(rootfs.path().to_path_buf()),
        ..Default::default()
    };
    Some((TestServer::start_with_options(opts).await, rootfs))
}

/// TEST-15 — the advertised `["minimal","full"]` enum is enforced on the chat
/// path over real HTTP, and the advertisement itself is pinned alongside it.
#[tokio::test]
async fn execute_command_refuses_a_flavor_outside_the_advertised_enum() {
    let Some((server, _rootfs)) = sandbox_server_without_rootfs().await else {
        return;
    };
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "cs_flavor_contract",
        &["code_sandbox::execute", "chat::read", "chat::create"],
    )
    .await;
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &user.token,
        None,
        Some("flavor-contract conv"),
    )
    .await;
    let conv_id = conv["id"].as_str().expect("conversation id").to_string();
    let client = reqwest::Client::new();
    let url = format!("{}/api/code-sandbox", server.base_url);

    let rpc = |body: serde_json::Value| {
        client
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .header("x-conversation-id", &conv_id)
            .json(&body)
            .send()
    };

    // ---- (1) The ADVERTISEMENT — the enum the model is shown. Pinned in the
    // same test as the enforcement so the two cannot drift apart again.
    let list: serde_json::Value = rpc(serde_json::json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/list"
    }))
    .await
    .expect("tools/list")
    .json()
    .await
    .expect("tools/list json");
    let exec = list["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "execute_command")
        .expect("execute_command is advertised");
    let advertised: Vec<String> = exec["inputSchema"]["properties"]["flavor"]["enum"]
        .as_array()
        .expect("flavor enum is advertised")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert_eq!(
        advertised,
        vec!["minimal".to_string(), "full".to_string()],
        "the schema advertises exactly these flavors"
    );

    // ---- (2) POSITIVE CONTROL, executing nothing.
    //
    // A DIFFERENT sandbox tool on the same route, same auth, same conversation.
    // `read_file` goes through the identical chain the refusal must traverse —
    // JWT → permission → `x-conversation-id` → ownership → `build_context` →
    // `dispatch` — and comes back as a normal JSON-RPC result. Without this, a
    // "the bad flavor was refused" assertion is indistinguishable from "every
    // call on this route fails". It is deliberately NOT `execute_command`: on a
    // rootfs-less server an accepted flavor would spawn a real GitHub download.
    let control: serde_json::Value = rpc(serde_json::json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "read_file", "arguments": { "filename": "definitely-absent.txt" } }
    }))
    .await
    .expect("control call")
    .json()
    .await
    .expect("control json");
    assert!(
        control["result"].is_object() || control["error"]["message"].is_string(),
        "the control must produce a real JSON-RPC response: {control}"
    );
    let control_text = control.to_string();
    assert!(
        !control_text.contains("must be one of"),
        "the control call must NOT be refused for a flavor — it supplies none: {control}"
    );

    // ---- (3) The ENFORCEMENT: an invented flavor is refused, as a plain JSON
    // error, without entering the SSE branch that reaches the network.
    let resp = rpc(serde_json::json!({
        "jsonrpc": "2.0", "id": 3, "method": "tools/call",
        "params": {
            "name": "execute_command",
            "arguments": { "command": "echo hi", "flavor": "zee-workflow" }
        }
    }))
    .await
    .expect("send bogus flavor");
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        !ctype.contains("event-stream"),
        "a refused flavor must NOT enter the SSE execute path — that path spawns the \
         work (and the rootfs fetch) BEFORE responding; got content-type {ctype}"
    );
    let body: serde_json::Value = resp.json().await.expect("json error envelope");
    let msg = body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a JSON-RPC error refusal, got: {body}"))
        .to_string();
    assert!(
        msg.contains("flavor"),
        "the refusal names the argument: {msg}"
    );
    for name in &advertised {
        assert!(
            msg.contains(name.as_str()),
            "the refusal lists the advertised flavor `{name}`: {msg}"
        );
    }
    assert!(
        msg.contains("Example: {"),
        "the refusal carries a copyable literal-JSON example: {msg}"
    );
}

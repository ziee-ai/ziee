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
//! ROOTFS-FREE by construction. `code_sandbox` init defers every
//! rootfs-dependent probe (schema sentinel, PID-ns) to the first
//! `execute_command`, so the module registers with `rootfs_path` pointing at an
//! empty temp dir. That is exactly the state this test wants: the flavor check
//! must refuse BEFORE anything touches a rootfs or the network, so a server with
//! no rootfs at all is the strictest place to prove it. Needs `bwrap` only
//! because `probe_host` refuses to register without it — self-skips otherwise.

use crate::code_sandbox::harness::bwrap_available;
use crate::common::{TestServer, TestServerOptions};

/// A sandbox-ENABLED server whose configured rootfs dir is empty. Returns `None`
/// (skip) when the host can't register the sandbox at all.
async fn sandbox_server_without_rootfs() -> Option<(TestServer, tempfile::TempDir)> {
    #[cfg(target_os = "linux")]
    if !bwrap_available() {
        eprintln!("test skipped: bwrap not installed (apt install bubblewrap)");
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
/// path, and the advertisement itself is pinned alongside it.
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

    let call = |flavor: &str| {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "execute_command",
                "arguments": { "command": "echo hi", "flavor": flavor }
            }
        });
        client
            .post(&url)
            .header("Authorization", format!("Bearer {}", user.token))
            .header("x-conversation-id", &conv_id)
            .json(&body)
            .send()
    };

    // (1) The advertisement — the enum the model is shown. Pinned HERE, in the
    // same test as the enforcement, so the two can never drift apart again.
    let list: serde_json::Value = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
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

    // (2) The enforcement — an invented flavor must be refused, as a plain JSON
    // error, without entering the SSE streaming path that reaches the network.
    let resp = call("zee-workflow").await.expect("send bogus flavor");
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        !ctype.contains("event-stream"),
        "a refused flavor must NOT enter the SSE execute path (which is what \
         reaches install_version and the GitHub URL); got content-type {ctype}"
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

    // (3) HAPPY-PATH COUNTERPART: every advertised flavor is still accepted by
    // the flavor check. Without a rootfs the call cannot COMPLETE — it enters
    // the streaming path and fails on the missing rootfs — so the assertion is
    // precisely "it was not refused FOR ITS FLAVOR", which is what this test
    // owns. (Tier 6 covers a real end-to-end run.)
    for name in &advertised {
        let resp = call(name).await.expect("send advertised flavor");
        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if ctype.contains("event-stream") {
            continue; // took the streaming path → past the flavor check.
        }
        let body: serde_json::Value = resp.json().await.expect("json body");
        if let Some(msg) = body["error"]["message"].as_str() {
            assert!(
                !msg.contains("must be one of"),
                "the advertised flavor `{name}` must NOT be refused as unknown: {msg}"
            );
        }
    }
}

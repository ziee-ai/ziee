//! Extended conformance tests against `@modelcontextprotocol/server-everything`.
//!
//! Complements `conformance_test.rs` (basic happy paths) with deeper
//! exercises of the reference server: concurrent calls into one session,
//! resources list+read, prompts with arguments, repeated ping under load,
//! reconnect cycles, and call sequencing across tool kinds.
//!
//! These tests are skipped when `npx` is unavailable (`try_start_or_skip`).

use super::fixtures::everything_server::EverythingServer;
use std::sync::Arc;
use tokio::sync::Mutex;
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "everything-extended".to_string(),
        display_name: "Everything (extended fixture)".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        url: Some(url),
        headers: serde_json::json!({}),
        timeout_seconds: 30,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        run_in_sandbox: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn extended_concurrent_tool_calls_share_one_session() {
    // Stress the request-id allocator + session sharing: fire many tool
    // calls in parallel from one client and verify each completes
    // correctly (no id collisions, no response mixups).
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_concurrent_tool_calls_share_one_session"
    ).await else { return; };

    let client = Arc::new(Mutex::new(
        HttpMcpClient::new(server_config(server.base_url())).unwrap()
    ));
    client.lock().await.connect().await.expect("connect");

    // Note: HttpMcpClient's call_tool requires &mut self, so true
    // overlapped sends from one client would need an internal RWLock split.
    // What we *can* verify is that rapid back-to-back calls all succeed
    // with unique ids — the id allocator under concurrency.
    let mut tasks = Vec::new();
    for i in 0..10 {
        let c = client.clone();
        tasks.push(tokio::spawn(async move {
            let mut guard = c.lock().await;
            guard.call_tool(
                "echo",
                serde_json::json!({ "message": format!("seq-{}", i) }),
                None, None, None,
            ).await
        }));
    }

    for (i, task) in tasks.into_iter().enumerate() {
        let result = task.await.unwrap()
            .unwrap_or_else(|e| panic!("call {} failed: {}", i, e));
        assert!(!result.is_error, "call {} returned error", i);
        let combined: String = result.content.iter()
            .filter_map(|c| serde_json::to_string(&c.content).ok())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(combined.contains(&format!("seq-{}", i)),
                "call {} got wrong response (id mixup?): {}", i, combined);
    }

    client.lock().await.disconnect().await.ok();
}

#[tokio::test]
async fn extended_resources_list_and_read() {
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_resources_list_and_read"
    ).await else { return; };

    let mut client = HttpMcpClient::new(server_config(server.base_url())).unwrap();
    client.connect().await.expect("connect");

    let resources = client.list_resources().await.expect("list_resources");
    assert!(!resources.is_empty(),
            "server-everything exposes static resources; got empty list");

    // Read the first resource — verify the resource endpoint round-trips.
    let first = &resources[0];
    let value = client.read_resource(&first.uri).await
        .unwrap_or_else(|e| panic!("read_resource({}) failed: {}", first.uri, e));
    // Per MCP spec, read_resource returns `{ "contents": [...] }`
    assert!(value.get("contents").is_some(),
            "read_resource response must contain `contents`; got {}", value);

    client.disconnect().await.ok();
}

#[tokio::test]
async fn extended_get_prompt_with_arguments() {
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_get_prompt_with_arguments"
    ).await else { return; };

    let mut client = HttpMcpClient::new(server_config(server.base_url())).unwrap();
    client.connect().await.expect("connect");

    let prompts = client.list_prompts().await.expect("list_prompts");
    // Find a prompt that takes a required arg — server-everything's
    // `simple_prompt` takes none, `complex_prompt` takes one. Pick whatever
    // we find with a required arg, else fall back to first.
    let target = prompts.iter()
        .find(|p| p.arguments.iter().any(|a| a.required))
        .or_else(|| prompts.first())
        .expect("at least one prompt available");

    // Pass arbitrary string args for any declared params — server-everything
    // accepts arbitrary strings.
    let mut args = serde_json::Map::new();
    for arg in &target.arguments {
        args.insert(arg.name.clone(),
                    serde_json::Value::String("test-value".to_string()));
    }
    let result = client.get_prompt(&target.name, Some(serde_json::Value::Object(args))).await
        .unwrap_or_else(|e| panic!("get_prompt({}) failed: {}", target.name, e));

    assert!(!result.messages.is_empty(),
            "prompt rendering should produce at least one message");

    client.disconnect().await.ok();
}

#[tokio::test]
async fn extended_ping_repeatedly_under_load() {
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_ping_repeatedly_under_load"
    ).await else { return; };

    let mut client = HttpMcpClient::new(server_config(server.base_url())).unwrap();
    client.connect().await.expect("connect");

    for i in 0..20 {
        client.ping().await
            .unwrap_or_else(|e| panic!("ping iteration {} failed: {}", i, e));
    }

    client.disconnect().await.ok();
}

#[tokio::test]
async fn extended_reconnect_cycle_works() {
    // Verify connect → disconnect → connect works cleanly. Catches state
    // that should be reset on reconnect (session id, request id counter
    // shared across reconnects is OK; what matters is the new session
    // initializes correctly).
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_reconnect_cycle_works"
    ).await else { return; };

    let mut client = HttpMcpClient::new(server_config(server.base_url())).unwrap();

    for cycle in 0..3 {
        client.connect().await
            .unwrap_or_else(|e| panic!("connect cycle {} failed: {}", cycle, e));
        let tools = client.list_tools().await
            .unwrap_or_else(|e| panic!("list_tools after reconnect {} failed: {}", cycle, e));
        assert!(!tools.is_empty(), "tools should still be available after reconnect {}", cycle);
        client.disconnect().await
            .unwrap_or_else(|e| panic!("disconnect cycle {} failed: {}", cycle, e));
    }
}

#[tokio::test]
async fn extended_mixed_call_sequence() {
    // A realistic call pattern: list_tools → list_prompts → list_resources →
    // tool call → ping → tool call. Tests that interleaving of method kinds
    // doesn't desync request ids or session state.
    let Some(server) = EverythingServer::try_start_or_skip(
        "extended_mixed_call_sequence"
    ).await else { return; };

    let mut client = HttpMcpClient::new(server_config(server.base_url())).unwrap();
    client.connect().await.expect("connect");

    let tools = client.list_tools().await.expect("list_tools");
    assert!(!tools.is_empty());
    let _ = client.list_prompts().await.expect("list_prompts");
    let _ = client.list_resources().await.expect("list_resources");

    let r1 = client.call_tool("echo", serde_json::json!({"message": "first"}), None, None, None)
        .await.expect("first echo");
    assert!(!r1.is_error);

    client.ping().await.expect("ping in middle");

    let r2 = client.call_tool("echo", serde_json::json!({"message": "second"}), None, None, None)
        .await.expect("second echo");
    assert!(!r2.is_error);

    client.disconnect().await.ok();
}

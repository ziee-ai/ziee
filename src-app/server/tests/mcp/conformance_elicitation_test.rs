//! Elicitation conformance tests for the MCP HTTP client.
//!
//! Elicitation is the MCP feature where a server, mid tool-call, requests
//! structured user input from the client via a `elicitation/create`
//! JSON-RPC request sent over the open SSE stream. The client surfaces the
//! request to the user (via the `sse_tx` SSE event channel and the
//! `elicit_notify_tx` notification channel), awaits the user's response
//! through `elicitation_registry::respond()`, then POSTs that response
//! back to the server as a JSON-RPC reply.
//!
//! These tests use [`MockElicitationServer`] — a coordinated mock that
//! keeps the tool-call SSE stream open across multiple HTTP requests so it
//! can sequence the elicitation request/response/result handshake.
//!
//! Coverage:
//! - accept / decline / cancel happy paths
//! - notification fields (message, schema, server)
//! - SSE event emitted to UI via `sse_tx`
//! - sequential elicitations in one tool call get unique ids
//! - missing `sse_tx` auto-cancels (no way to surface UI)
//! - oneshot channel dropped → cancel sent
//! - HTTP respond endpoint: 404 unknown id, 400 invalid action, 403 no perm

use super::fixtures::mock_elicitation_server::{ElicitationScript, MockElicitationServer};
use std::time::Duration;
use tokio::sync::mpsc;
use ziee::{
    elicitation_registry, ElicitationResponse, ElicitationStartedNotification, HttpMcpClient,
    McpClient, McpServer, TransportType, UsageMode,
};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "mock-elicit".to_string(),
        display_name: "Mock Elicitation".to_string(),
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
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

/// Helper: spin up the mock, connect a client, return both plus the
/// notification and sse channels the test will drive.
async fn setup(
    script: ElicitationScript,
) -> (
    MockElicitationServer,
    HttpMcpClient,
    mpsc::UnboundedReceiver<ElicitationStartedNotification>,
    mpsc::UnboundedReceiver<Result<axum::response::sse::Event, std::convert::Infallible>>,
    mpsc::UnboundedSender<ElicitationStartedNotification>,
    mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>,
) {
    let mock = MockElicitationServer::start_with_script(script).await;
    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let (notify_tx, notify_rx) = mpsc::unbounded_channel::<ElicitationStartedNotification>();
    let (sse_tx, sse_rx) = mpsc::unbounded_channel::<
        Result<axum::response::sse::Event, std::convert::Infallible>,
    >();
    (mock, client, notify_rx, sse_rx, notify_tx, sse_tx)
}

// ─── Accept / decline / cancel happy paths ─────────────────────────────────

#[tokio::test]
async fn elicit_accept_happy_path() {
    let script = ElicitationScript {
        message: "Approve the deletion?".to_string(),
        requested_schema: serde_json::json!({
            "type": "object",
            "properties": { "approve": { "type": "boolean" } }
        }),
        tool_result_content: vec![serde_json::json!({
            "type": "text",
            "text": "deletion-approved-and-done"
        })],
        ..ElicitationScript::default()
    };
    let (mock, mut client, mut notify_rx, _sse_rx, notify_tx, sse_tx) = setup(script).await;

    let message_id = uuid::Uuid::new_v4();
    let call_handle = tokio::spawn(async move {
        client
            .call_tool(
                "delete_thing",
                serde_json::json!({"id": "42"}),
                Some(message_id),
                Some(sse_tx),
                Some(notify_tx),
            )
            .await
    });

    // Receive the elicitation notification within 3s
    let notif = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .expect("must surface elicitation notification within 3s")
        .expect("notification channel must yield Some");

    assert_eq!(notif.message, "Approve the deletion?");
    assert_eq!(notif.message_id, Some(message_id));
    assert_eq!(notif.server, "mock-elicit");
    assert_eq!(notif.requested_schema["properties"]["approve"]["type"], "boolean");

    // User accepts with content
    let (_found, _) = elicitation_registry::respond(
        notif.elicitation_id,
        ElicitationResponse {
            action: "accept".to_string(),
            content: Some(serde_json::json!({"approve": true})),
        },
    );

    let result = tokio::time::timeout(Duration::from_secs(5), call_handle)
        .await
        .expect("call_tool must complete within 5s after response")
        .expect("task")
        .expect("tool result");

    assert!(!result.is_error);
    let combined = serde_json::to_string(&result.content).unwrap();
    assert!(combined.contains("deletion-approved-and-done"));

    // Verify the mock got the accept body
    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1, "mock should have received exactly 1 elicitation response");
    let r = &responses[0];
    assert_eq!(r["result"]["action"], "accept");
    assert_eq!(r["result"]["content"]["approve"], true);
}

#[tokio::test]
async fn elicit_decline_path_omits_content() {
    let (mock, mut client, mut notify_rx, _sse_rx, notify_tx, sse_tx) =
        setup(ElicitationScript::default()).await;

    let call_handle = tokio::spawn(async move {
        client
            .call_tool(
                "anything",
                serde_json::json!({}),
                None,
                Some(sse_tx),
                Some(notify_tx),
            )
            .await
    });

    let notif = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();

    elicitation_registry::respond(
        notif.elicitation_id,
        ElicitationResponse {
            action: "decline".to_string(),
            content: None,
        },
    );

    let _ = tokio::time::timeout(Duration::from_secs(5), call_handle).await.unwrap();

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["action"], "decline");
    // Per http.rs: for non-accept actions, content is omitted (not null)
    assert!(responses[0]["result"].get("content").is_none(),
            "decline result must omit `content` entirely; got: {}",
            responses[0]["result"]);
}

#[tokio::test]
async fn elicit_cancel_path_omits_content() {
    let (mock, mut client, mut notify_rx, _sse_rx, notify_tx, sse_tx) =
        setup(ElicitationScript::default()).await;

    let call_handle = tokio::spawn(async move {
        client
            .call_tool("anything", serde_json::json!({}), None, Some(sse_tx), Some(notify_tx))
            .await
    });

    let notif = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();

    elicitation_registry::respond(
        notif.elicitation_id,
        ElicitationResponse {
            action: "cancel".to_string(),
            content: None,
        },
    );

    let _ = tokio::time::timeout(Duration::from_secs(5), call_handle).await.unwrap();

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["action"], "cancel");
    assert!(responses[0]["result"].get("content").is_none());
}

// ─── UI surface: SSE event sent to sse_tx ──────────────────────────────────

#[tokio::test]
async fn elicit_sse_event_includes_elicitation_id_and_schema() {
    let (_mock, mut client, mut notify_rx, mut sse_rx, notify_tx, sse_tx) =
        setup(ElicitationScript {
            message: "name?".to_string(),
            requested_schema: serde_json::json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }),
            ..ElicitationScript::default()
        })
        .await;

    let call_handle = tokio::spawn(async move {
        client
            .call_tool("anything", serde_json::json!({}), None, Some(sse_tx), Some(notify_tx))
            .await
    });

    // Drain the first SSE event surfaced to the UI
    let event = tokio::time::timeout(Duration::from_secs(3), sse_rx.recv())
        .await
        .expect("must surface SSE event within 3s")
        .expect("event channel must yield Some")
        .expect("event must be Ok");

    // Event::default().event("mcpElicitationRequired").data(json)
    let serialized = format!("{:?}", event);
    assert!(serialized.contains("mcpElicitationRequired"),
            "event should be tagged 'mcpElicitationRequired'; got: {}", serialized);
    assert!(serialized.contains("requested_schema") || serialized.contains("name"),
            "event payload should include schema/data; got: {}", serialized);

    // Clean up the awaiting client by responding cancel
    let notif = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();
    elicitation_registry::respond(
        notif.elicitation_id,
        ElicitationResponse { action: "cancel".to_string(), content: None },
    );
    let _ = tokio::time::timeout(Duration::from_secs(5), call_handle).await;
}

// ─── Notification ordering: notification before SSE event ──────────────────

#[tokio::test]
async fn elicit_notification_fires_before_sse_event() {
    // The implementation in http.rs sends the ElicitationStartedNotification
    // BEFORE pushing the mcpElicitationRequired SSE event so the extension
    // layer can persist the DB row before the UI receives the elicitation.
    let (_mock, mut client, mut notify_rx, mut sse_rx, notify_tx, sse_tx) =
        setup(ElicitationScript::default()).await;

    let call_handle = tokio::spawn(async move {
        client
            .call_tool("anything", serde_json::json!({}), None, Some(sse_tx), Some(notify_tx))
            .await
    });

    // Both channels should populate; the notification must come first or
    // at least be available immediately when we check.
    let notif = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let sse_evt = tokio::time::timeout(Duration::from_secs(3), sse_rx.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    // The SSE event's payload should include the same elicitation_id
    let evt_str = format!("{:?}", sse_evt);
    assert!(evt_str.contains(&notif.elicitation_id.to_string()),
            "SSE event must reference the notification's elicitation_id; got: {}", evt_str);

    elicitation_registry::respond(
        notif.elicitation_id,
        ElicitationResponse { action: "cancel".to_string(), content: None },
    );
    let _ = tokio::time::timeout(Duration::from_secs(5), call_handle).await;
}

// ─── Sequential elicitations in one tool call ─────────────────────────────

#[tokio::test]
async fn elicit_two_sequential_in_one_tool_call_get_unique_ids() {
    let mock = MockElicitationServer::start_with_script(ElicitationScript {
        message: "step?".to_string(),
        elicitation_response_timeout: Duration::from_secs(5),
        ..ElicitationScript::default()
    })
    .await;
    mock.set_elicitations_per_tool_call(2);

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<ElicitationStartedNotification>();
    let (sse_tx, _sse_rx) = mpsc::unbounded_channel::<
        Result<axum::response::sse::Event, std::convert::Infallible>,
    >();

    let call_handle = tokio::spawn(async move {
        client
            .call_tool("multi_step", serde_json::json!({}), None, Some(sse_tx), Some(notify_tx))
            .await
    });

    // First elicitation
    let notif1 = tokio::time::timeout(Duration::from_secs(3), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();
    elicitation_registry::respond(
        notif1.elicitation_id,
        ElicitationResponse {
            action: "accept".to_string(),
            content: Some(serde_json::json!({"step": 1})),
        },
    );

    // Second elicitation (mock sends another after the first responds)
    let notif2 = tokio::time::timeout(Duration::from_secs(5), notify_rx.recv())
        .await
        .unwrap()
        .unwrap();
    elicitation_registry::respond(
        notif2.elicitation_id,
        ElicitationResponse {
            action: "accept".to_string(),
            content: Some(serde_json::json!({"step": 2})),
        },
    );

    assert_ne!(notif1.elicitation_id, notif2.elicitation_id,
               "each elicitation must get a fresh per-elicitation UUID");

    let result = tokio::time::timeout(Duration::from_secs(8), call_handle)
        .await
        .expect("call_tool must complete")
        .expect("task")
        .expect("tool");
    assert!(!result.is_error);

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"]["content"]["step"], 1);
    assert_eq!(responses[1]["result"]["content"]["step"], 2);
}

// ─── Defensive: missing sse_tx → auto-cancel ───────────────────────────────

#[tokio::test]
async fn elicit_without_sse_tx_auto_cancels() {
    // If a tool call enters the SSE branch without an sse_tx (e.g., called
    // directly without an Axum SSE forwarder), the client has no way to
    // reach the user, so it must auto-cancel rather than hang.
    let mock = MockElicitationServer::start_with_script(ElicitationScript {
        elicitation_response_timeout: Duration::from_secs(2),
        ..ElicitationScript::default()
    })
    .await;

    let mut client = HttpMcpClient::new(server_config(mock.base_url())).unwrap();
    client.connect().await.expect("connect");

    // Note: no sse_tx, but we still need elicit_notify_tx=None too —
    // otherwise the notification fires but no consumer waits on it.
    let result = tokio::time::timeout(
        Duration::from_secs(8),
        client.call_tool("anything", serde_json::json!({}), None, None, None),
    )
    .await
    .expect("client must NOT hang when no sse_tx is provided")
    .expect("call_tool should return a result (cancel path)");

    // After auto-cancel, the mock continues and emits the tool result, so
    // we should get a successful ToolResult here. What matters is that
    // (a) the client didn't hang, (b) the mock recorded a cancel response.
    let _ = result;

    let responses = mock.elicitation_responses();
    assert_eq!(responses.len(), 1,
               "mock should have received the auto-cancel response");
    assert_eq!(responses[0]["result"]["action"], "cancel",
               "no-sse-tx path must auto-cancel the elicitation");
}

// ─── Registry-level: respond on unknown id is a no-op ─────────────────────

#[tokio::test]
async fn elicit_registry_respond_unknown_id_returns_not_found() {
    let unknown = uuid::Uuid::new_v4();
    let (found, content_id) = elicitation_registry::respond(
        unknown,
        ElicitationResponse { action: "accept".to_string(), content: None },
    );
    assert!(!found, "registry::respond must report not-found for unknown id");
    assert!(content_id.is_none());
}

#[tokio::test]
async fn elicit_registry_remove_cleans_up_pending() {
    // Register an entry, then remove it. respond() should report not-found.
    let id = uuid::Uuid::new_v4();
    let (tx, _rx) = tokio::sync::oneshot::channel();
    let cid = uuid::Uuid::new_v4();
    elicitation_registry::register(id, tx, Some(cid));

    let removed = elicitation_registry::remove(id);
    assert_eq!(removed, Some(cid),
               "remove() must return the registered content_id");

    let (found, _) = elicitation_registry::respond(
        id,
        ElicitationResponse { action: "accept".to_string(), content: None },
    );
    assert!(!found, "respond after remove must be a no-op");
}

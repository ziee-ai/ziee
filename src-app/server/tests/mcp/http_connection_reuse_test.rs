//! Regression test for the stale keep-alive connection hang.
//!
//! A proxy/tunnel in front of an MCP server (e.g. a Coder workspace-app tunnel,
//! nginx, a cloud LB) can silently drop an *idle* keep-alive connection
//! half-open (no FIN/RST). reqwest would then hand that dead connection to the
//! NEXT request — e.g. `notifications/initialized` right after a successful
//! `initialize` — and the write blackholes until the timeout ("error sending
//! request for url"). The fix is `.pool_max_idle_per_host(0)` on the MCP HTTP
//! clients (fresh connection per request).
//!
//! This test models a "reaping proxy": a raw TCP server that answers EXACTLY ONE
//! HTTP request per connection, then holds the socket open and goes silent
//! (never answers a 2nd request on the same connection). The existing
//! `MockMcpServer` keeps connections alive normally, which is why the suite
//! never caught this — hence a dedicated mock here.
//!
//! - WITH the fix: every request opens a fresh connection the mock answers → the
//!   test passes fast (no timing dependence).
//! - WITHOUT the fix: `notifications/initialized` reuses the held-silent
//!   connection → hangs → `timeout_seconds` fires → `connect()` errors → this
//!   test fails. That is the exact production bug, now guarded.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use ziee::{HttpMcpClient, McpClient, McpServer, TransportType, UsageMode};

fn server_config(url: String) -> McpServer {
    McpServer {
        id: uuid::Uuid::new_v4(),
        user_id: None,
        name: "reaping-proxy-mock".to_string(),
        display_name: "Reaping Proxy Mock".to_string(),
        description: None,
        enabled: true,
        is_system: false,
        transport_type: TransportType::Http,
        command: None,
        args: serde_json::json!([]),
        environment_variables: serde_json::json!({}),
        url: Some(url),
        headers: serde_json::json!({}),
        // Short: if a regression reuses a dead connection, the hang fails the
        // test in ~5s instead of the 30s default. The passing path never waits.
        timeout_seconds: 5,
        supports_sampling: false,
        usage_mode: UsageMode::Auto,
        max_concurrent_sessions: None,
        is_built_in: false,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A TCP server that answers exactly ONE HTTP request per connection, then holds
/// the socket open and never responds again (models a proxy that black-holes a
/// reused keep-alive connection). Returns the base URL (`http://host:port/mcp`).
async fn start_reaping_mock() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let mut data: Vec<u8> = Vec::new();

                // Read until the end of the request headers.
                let (head_end, content_len, is_get) = loop {
                    let n = match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    data.extend_from_slice(&buf[..n]);
                    if let Some(pos) = find_subslice(&data, b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&data[..pos]);
                        let is_get = head.starts_with("GET ");
                        let content_len = head
                            .lines()
                            .find_map(|l| {
                                let lower = l.to_ascii_lowercase();
                                lower
                                    .strip_prefix("content-length:")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        break (pos + 4, content_len, is_get);
                    }
                };

                // Read the rest of the body (per Content-Length).
                while data.len() < head_end + content_len {
                    match sock.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => data.extend_from_slice(&buf[..n]),
                    }
                }

                let body = &data[head_end..(head_end + content_len).min(data.len())];
                let body_json: serde_json::Value =
                    serde_json::from_slice(body).unwrap_or(serde_json::Value::Null);
                let method = body_json.get("method").and_then(|m| m.as_str()).unwrap_or("");
                let id = body_json.get("id").cloned().unwrap_or(serde_json::Value::Null);

                let response = if is_get {
                    // Standalone GET-SSE → 405 so the client's GET-SSE task exits quietly.
                    "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if method == "initialize" {
                    let result = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "protocolVersion": "2025-11-25",
                            "capabilities": { "tools": {} },
                            "serverInfo": { "name": "reaping-mock", "version": "0.0.1" }
                        }
                    })
                    .to_string();
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nmcp-session-id: reap-sess-1\r\nContent-Length: {}\r\n\r\n{}",
                        result.len(),
                        result
                    )
                } else if method == "tools/list" {
                    let result = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": { "tools": [
                            { "name": "t", "description": "d", "inputSchema": { "type": "object" } }
                        ] }
                    })
                    .to_string();
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                        result.len(),
                        result
                    )
                } else {
                    // notifications/initialized (and any other notification/DELETE) → 202.
                    "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string()
                };

                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.flush().await;

                // Go SILENT: hold the socket open and never respond again. A client
                // that REUSES this idle keep-alive connection for a 2nd request will
                // blackhole (reproducing the proxy reap). When the client instead
                // opens a fresh connection (the fix), it closes this one → EOF here.
                let mut sink = vec![0u8; 1024];
                loop {
                    match sock.read(&mut sink).await {
                        Ok(0) | Err(_) => break, // client closed the connection — done
                        Ok(_) => { /* drain any reused-request bytes; never respond */ }
                    }
                }
            });
        }
    });
    format!("http://{}/mcp", addr)
}

#[tokio::test]
async fn survives_proxy_that_serves_one_request_per_connection() {
    let url = start_reaping_mock().await;
    let mut client = HttpMcpClient::new(server_config(url)).expect("client construction");

    // connect() runs initialize + notifications/initialized (then spawns the
    // standalone GET-SSE). With idle-pool reuse, the notification would reuse the
    // held-silent connection and hang to timeout; with pool_max_idle_per_host(0)
    // each request gets a fresh connection the mock answers.
    client
        .connect()
        .await
        .expect("connect() must succeed without reusing a dead idle connection");

    // A further discrete request must also work (fresh connection again).
    let tools = client
        .list_tools()
        .await
        .expect("list_tools() must succeed on a fresh connection");
    assert_eq!(tools.len(), 1, "mock advertises exactly one tool");

    client.disconnect().await.ok();
}

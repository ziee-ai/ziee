//! TEST-6/7/8/9/12 — the daemon↔pane JSON-RPC round-trip (ITEM-9 pane path).
//!
//! Hermetic + cross-platform: an ephemeral bridge (`server::start(0)`, temp-dir
//! cert) plus a **mock pane** — a `tokio-tungstenite` WSS client that connects with
//! a valid token+Origin, sends the `register` hello, and answers daemon→pane
//! requests. The Office.js execution is the ONLY thing mocked; the broker, socket
//! loop, and `dispatch_tool` run for real. This proves the exact wire contract the
//! real `taskpane.js` implements, identically on macOS and Windows (WKWebView /
//! WebView2 only host the same JS + WSS).
//!
//! Run single-threaded — the broker's pane/pending registries are process-global:
//!   cargo test -p ziee-desktop --test <target> -- --test-threads=1 office_bridge::pane_rpc

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use ziee_desktop::modules::office_bridge::bridge::{broker, cert, server};
use ziee_desktop::modules::office_bridge::handlers::dispatch_tool;
use ziee_desktop::modules::office_bridge::platform;

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

// ─────────────────────────── TLS + WSS client helpers ───────────────────────────
// (Mirror `bridge_test.rs` — a client that trusts the minted bridge cert.)

fn client_config_trusting(cert_der: &[u8]) -> rustls::ClientConfig {
    use rustls::pki_types::CertificateDer;
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(cert_der.to_vec()))
        .expect("add minted cert as trust root");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

fn https_client(cert_der: &[u8]) -> reqwest::Client {
    let cert = reqwest::Certificate::from_der(cert_der).expect("der → reqwest cert");
    reqwest::Client::builder()
        .add_root_certificate(cert)
        .build()
        .expect("build https client")
}

fn extract_injected_token(html: &str) -> String {
    let marker = "window.__ZIEE_BRIDGE_TOKEN__ = \"";
    let start = html.find(marker).expect("token in served page") + marker.len();
    let end = html[start..].find('"').expect("closing quote") + start;
    html[start..end].to_string()
}

fn ws_request(
    port: u16,
    origin: &str,
    token: &str,
) -> tokio_tungstenite::tungstenite::handshake::client::Request {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("wss://127.0.0.1:{port}/bridge")
        .into_client_request()
        .expect("client request");
    req.headers_mut()
        .insert(http::header::ORIGIN, http::HeaderValue::from_str(origin).unwrap());
    req.headers_mut().insert(
        http::header::SEC_WEBSOCKET_PROTOCOL,
        http::HeaderValue::from_str(&format!("ziee-bridge, {token}")).unwrap(),
    );
    req
}

async fn ws_connect(port: u16, token: &str, cert_der: &[u8]) -> WsStream {
    let origin = format!("https://127.0.0.1:{port}");
    let connector = tokio_tungstenite::Connector::Rustls(Arc::new(client_config_trusting(cert_der)));
    let (ws, _resp) = tokio_tungstenite::connect_async_tls_with_config(
        ws_request(port, &origin, token),
        None,
        false,
        Some(connector),
    )
    .await
    .expect("valid token + origin upgrades");
    ws
}

/// Bring up an ephemeral bridge and connect a WSS pane client (valid token+Origin).
async fn bring_up() -> (server::BridgeHandle, WsStream) {
    let dir = tempfile::tempdir().expect("tempdir");
    let minted = cert::load_or_mint(dir.path()).expect("mint bridge cert");
    let cert_der = minted.ca_der.clone();
    let handle = server::start(0, dir.path().to_path_buf())
        .await
        .expect("bridge starts on an ephemeral port");
    let port = handle.port;

    let html = https_client(&cert_der)
        .get(format!("https://127.0.0.1:{port}/taskpane.html"))
        .send()
        .await
        .expect("GET taskpane.html")
        .text()
        .await
        .expect("body");
    let token = extract_injected_token(&html);
    let ws = ws_connect(port, &token, &cert_der).await;
    // Keep the tempdir alive for the listener's lifetime via the handle's process;
    // the OS cleans the temp cert when the process exits. Leak the dir handle so the
    // cert file survives the rest of the test.
    std::mem::forget(dir);
    (handle, ws)
}

// ─────────────────────────── mock pane driver ───────────────────────────

#[derive(Clone, Copy)]
enum Mode {
    Ok,
    Err,
}

/// Drive a mock task pane over `ws`: send the `register` hello, then answer each
/// daemon→pane request (a frame with `method` + a non-null `id`). In `Mode::Ok` it
/// replies with a result echoing the method + params (and a `text`); in `Mode::Err`
/// it replies with a JSON-RPC error. `junk_first` sends an unparsable frame and a
/// stale-id response before entering the loop (TEST-8).
async fn run_mock_pane(mut ws: WsStream, host: &'static str, doc_key: String, mode: Mode, junk_first: bool) {
    let register = json!({
        "jsonrpc": "2.0", "id": 1, "method": "register",
        "params": { "host": host, "doc_key": doc_key }
    });
    ws.send(Message::Text(register.to_string().into())).await.ok();

    if junk_first {
        ws.send(Message::Text("this is not json".into())).await.ok();
        let stale = json!({ "jsonrpc": "2.0", "id": 999_999, "result": { "text": "stale" } });
        ws.send(Message::Text(stale.to_string().into())).await.ok();
    }

    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Text(t) => {
                let v: Value = match serde_json::from_str(t.as_str()) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let method = v.get("method").and_then(|m| m.as_str());
                let id = v.get("id").cloned();
                if let (Some(method), Some(id)) = (method, id) {
                    if id.is_null() {
                        continue;
                    }
                    let resp = match mode {
                        Mode::Ok => json!({
                            "jsonrpc": "2.0", "id": id,
                            "result": {
                                "got_method": method,
                                "got_params": v.get("params").cloned().unwrap_or(Value::Null),
                                "text": "MOCK BODY",
                            }
                        }),
                        Mode::Err => json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": { "code": -32002, "message": "mock host-unsupported" }
                        }),
                    };
                    ws.send(Message::Text(resp.to_string().into())).await.ok();
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

/// Retry `call_pane` while it reports `OFFICE_PANE_NOT_CONNECTED` (the pane's
/// `register` may not be processed yet). Returns the result Value or the final
/// error CODE (as a String, to avoid naming `AppError` across the test crate).
async fn retry_call_pane(doc: &str, method: &str, params: Value) -> Result<Value, String> {
    for _ in 0..150 {
        match broker::call_pane(doc, method, params.clone()).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let code = e.error_code().to_string();
                if code == broker::OFFICE_PANE_NOT_CONNECTED {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
                return Err(code);
            }
        }
    }
    Err(broker::OFFICE_PANE_NOT_CONNECTED.to_string())
}

/// `retry_call_pane` for the full `dispatch_tool` path (TEST-9/12).
async fn retry_dispatch(name: &str, args: Value) -> Result<Value, String> {
    for _ in 0..150 {
        match dispatch_tool(platform::active(), name, &args).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let code = e.error_code().to_string();
                if code == broker::OFFICE_PANE_NOT_CONNECTED {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    continue;
                }
                return Err(code);
            }
        }
    }
    Err(broker::OFFICE_PANE_NOT_CONNECTED.to_string())
}

// ─────────────────────────── tests ───────────────────────────

/// TEST-6 — register + a daemon→pane request reaches the pane with the right
/// envelope and its reply routes back through the broker.
#[tokio::test]
async fn test6_pane_register_and_round_trip() {
    let doc = "/Users/x/RptSix.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Ok, false));

    let out = retry_call_pane(&doc, "read_document", json!({ "doc_full_name": doc }))
        .await
        .expect("round-trip result");
    assert_eq!(out.get("got_method").and_then(|m| m.as_str()), Some("read_document"));
    assert_eq!(
        out.get("got_params").and_then(|p| p.get("doc_full_name")).and_then(|d| d.as_str()),
        Some(doc.as_str()),
        "the pane received the doc_full_name param"
    );
    assert_eq!(out.get("text").and_then(|t| t.as_str()), Some("MOCK BODY"));

    handle.shutdown();
    pane.abort();
}

/// TEST-7 — closing the pane socket unregisters it: a later `call_pane` for that
/// doc is `OFFICE_PANE_NOT_CONNECTED`. Two decoy panes make the negative
/// deterministic (≥2 panes + no key match → not-connected, defeating sole-pane).
#[tokio::test]
async fn test7_close_unregisters_pane() {
    let doc = "/Users/x/RptSeven.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Ok, false));
    // Confirm it is connected first.
    retry_call_pane(&doc, "get_selection", json!({ "doc_full_name": doc }))
        .await
        .expect("connected before close");

    // Close the pane socket.
    pane.abort();

    // Decoys so the post-close resolution can't fall back to a foreign sole pane.
    let (tx1, _r1) = mpsc::unbounded_channel::<axum::extract::ws::Message>();
    let (tx2, _r2) = mpsc::unbounded_channel::<axum::extract::ws::Message>();
    let d1 = broker::next_pane_id();
    let d2 = broker::next_pane_id();
    broker::register_pane(d1, "word".into(), format!("/decoy/x-{d1}.docx"), tx1);
    broker::register_pane(d2, "excel".into(), format!("/decoy/y-{d2}.xlsx"), tx2);

    // Poll until the server has processed the close and unregistered the pane. Use a
    // SHORT per-call timeout: while the aborted pane is still (briefly) registered,
    // call_pane routes to it and times out fast; once the socket close is processed
    // and the pane unregistered, it flips to NOT_CONNECTED. Ok / TIMEOUT → keep
    // waiting; NOT_CONNECTED → done.
    let mut code = String::new();
    for _ in 0..80 {
        match broker::call_pane_with_timeout(
            &doc,
            "get_selection",
            json!({}),
            Duration::from_millis(100),
        )
        .await
        {
            Ok(_) => tokio::time::sleep(Duration::from_millis(20)).await,
            Err(e) => {
                let c = e.error_code().to_string();
                if c == broker::OFFICE_PANE_NOT_CONNECTED {
                    code = c;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    assert_eq!(code, broker::OFFICE_PANE_NOT_CONNECTED, "closed pane is unregistered");

    broker::unregister_pane(d1);
    broker::unregister_pane(d2);
    handle.shutdown();
}

/// TEST-8 — an unparsable frame and a stale-id response are ignored; the socket
/// stays up and a subsequent real round-trip still succeeds.
#[tokio::test]
async fn test8_junk_frames_are_ignored() {
    let doc = "/Users/x/RptEight.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Ok, true));

    let out = retry_call_pane(&doc, "get_selection", json!({ "doc_full_name": doc }))
        .await
        .expect("round-trip still works after junk frames");
    assert_eq!(out.get("text").and_then(|t| t.as_str()), Some("MOCK BODY"));

    handle.shutdown();
    pane.abort();
}

/// TEST-9 — the full `dispatch_tool` path maps the pane's result into the MCP
/// `tool_result` shape (`content` + `structuredContent`).
#[tokio::test]
async fn test9_dispatch_tool_read_document_round_trip() {
    let doc = "/Users/x/RptNine.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Ok, false));

    let out = retry_dispatch("read_document", json!({ "doc_full_name": doc }))
        .await
        .expect("dispatch_tool round-trip");
    assert_eq!(
        out.get("content").and_then(|c| c.get(0)).and_then(|c| c.get("text")).and_then(|t| t.as_str()),
        Some("MOCK BODY"),
        "readable content channel carries the pane text"
    );
    assert_eq!(
        out.get("structuredContent").and_then(|s| s.get("text")).and_then(|t| t.as_str()),
        Some("MOCK BODY"),
        "structuredContent carries the raw pane result"
    );

    handle.shutdown();
    pane.abort();
}

/// TEST-12 — a pane that replies with a JSON-RPC error propagates as the typed
/// `OFFICE_PANE_ERROR` through `dispatch_tool` (not a panic, not a success).
#[tokio::test]
async fn test12_pane_error_propagates() {
    let doc = "/Users/x/RptTwelve.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Err, false));

    let code = retry_dispatch("read_document", json!({ "doc_full_name": doc }))
        .await
        .expect_err("pane error surfaces as an error");
    assert_eq!(code, broker::OFFICE_PANE_ERROR);

    handle.shutdown();
    pane.abort();
}

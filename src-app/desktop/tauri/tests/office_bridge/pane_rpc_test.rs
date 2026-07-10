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
    /// Reply with a generic pane error (→ OFFICE_PANE_ERROR).
    Err,
    /// Reply with the pane's "unsupported on this host" code -32002
    /// (→ OFFICE_UNSUPPORTED_ON_HOST).
    Unsupported,
}

/// Drive a mock task pane over `ws`: send the `register` hello, then answer each
/// daemon→pane request (a frame with `method` + a non-null `id`). In `Mode::Ok` it
/// replies with a result echoing the method + params (and a `text`); in `Mode::Err`
/// it replies with a JSON-RPC error. `junk_first` sends an unparsable frame and a
/// stale-id response before entering the loop (TEST-8).
async fn run_mock_pane(mut ws: WsStream, host: &'static str, doc_key: String, mode: Mode, junk_first: bool) {
    let self_doc = doc_key.clone(); // included in Ok replies so a test can tell panes apart
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
                                "pane_doc": self_doc,
                                "text": "MOCK BODY",
                            }
                        }),
                        Mode::Err => json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": { "code": -32000, "message": "mock pane failure" }
                        }),
                        Mode::Unsupported => json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": { "code": -32002, "message": "only supported in Word" }
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

    let out = retry_call_pane(&doc, "run_office_js", json!({ "doc_full_name": doc }))
        .await
        .expect("round-trip result");
    assert_eq!(out.get("got_method").and_then(|m| m.as_str()), Some("run_office_js"));
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
    retry_call_pane(&doc, "run_office_js", json!({ "doc_full_name": doc }))
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
            "run_office_js",
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

    let out = retry_call_pane(&doc, "run_office_js", json!({ "doc_full_name": doc }))
        .await
        .expect("round-trip still works after junk frames");
    assert_eq!(out.get("text").and_then(|t| t.as_str()), Some("MOCK BODY"));

    handle.shutdown();
    pane.abort();
}

/// TEST-9 — the full `dispatch_tool` path maps the pane's result into the MCP
/// `tool_result` shape (`content` + `structuredContent`).
#[tokio::test]
async fn test9_dispatch_tool_run_office_js_round_trip() {
    let doc = "/Users/x/RptNine.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "word", doc.clone(), Mode::Ok, false));

    let out = retry_dispatch("run_office_js", json!({ "doc_full_name": doc, "script": "return 1;", "mode": "read" }))
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

    let code = retry_dispatch("run_office_js", json!({ "doc_full_name": doc, "script": "return 1;", "mode": "read" }))
        .await
        .expect_err("pane error surfaces as an error");
    assert_eq!(code, broker::OFFICE_PANE_ERROR);

    handle.shutdown();
    pane.abort();
}

/// TEST-16 — a pane reply with the "unsupported on this host" code (-32002) maps to
/// OFFICE_UNSUPPORTED_ON_HOST (same code as the native PPT pre-gate), not the generic
/// OFFICE_PANE_ERROR.
#[tokio::test]
async fn test16_pane_unsupported_maps_to_unsupported_on_host() {
    let doc = "/Users/x/RptSixteen.docx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "excel", doc.clone(), Mode::Unsupported, false));

    let code = retry_dispatch("run_office_js", json!({ "doc_full_name": doc, "script": "return 1;", "mode": "read" }))
        .await
        .expect_err("unsupported-on-host surfaces as an error");
    assert_eq!(code, "OFFICE_UNSUPPORTED_ON_HOST");

    handle.shutdown();
    pane.abort();
}

/// TEST-15 — POSITIVE multi-pane routing: two live panes for two distinct documents
/// each answer; a call for doc A must resolve to A's pane and a call for doc B to B's
/// (exact-match resolution + per-pane response binding, not cross-routed).
#[tokio::test]
async fn test15_two_panes_route_to_correct_document() {
    let doc_a = "/Users/x/Alpha.docx".to_string();
    let doc_b = "/Users/x/Beta.docx".to_string();

    let (handle_a, ws_a) = bring_up().await;
    let (handle_b, ws_b) = bring_up().await;
    let pane_a = tokio::spawn(run_mock_pane(ws_a, "word", doc_a.clone(), Mode::Ok, false));
    let pane_b = tokio::spawn(run_mock_pane(ws_b, "word", doc_b.clone(), Mode::Ok, false));

    // Both panes register against the SAME process-global broker (two separate
    // bridge listeners, one shared broker), so exact-key resolution must pick right.
    let out_a = retry_call_pane(&doc_a, "run_office_js", json!({ "doc_full_name": doc_a }))
        .await
        .expect("doc A round-trip");
    let out_b = retry_call_pane(&doc_b, "run_office_js", json!({ "doc_full_name": doc_b }))
        .await
        .expect("doc B round-trip");

    assert_eq!(
        out_a.get("pane_doc").and_then(|d| d.as_str()),
        Some(doc_a.as_str()),
        "doc A's call must be answered by doc A's pane"
    );
    assert_eq!(
        out_b.get("pane_doc").and_then(|d| d.as_str()),
        Some(doc_b.as_str()),
        "doc B's call must be answered by doc B's pane"
    );

    handle_a.shutdown();
    handle_b.shutdown();
    pane_a.abort();
    pane_b.abort();
}

// ───────────────── office-run-office-js: run_office_js pane tool ─────────────────

/// TEST-7 (office-run-office-js) — the full `dispatch_tool` path routes
/// `run_office_js` to the connected pane carrying `{doc_full_name, script}` and maps
/// the pane's reply into the MCP `tool_result` shape (`content` + `structuredContent`).
#[tokio::test]
async fn run_office_js_dispatch_round_trip() {
    let doc = "/Users/x/RunBook.xlsx".to_string();
    let (handle, ws) = bring_up().await;
    let pane = tokio::spawn(run_mock_pane(ws, "excel", doc.clone(), Mode::Ok, false));

    // TEST-8 — `mode` is delivered in `params` but does NOT change execution: read
    // and write round-trip identically through the daemon → broker → pane.
    for mode in ["read", "write"] {
        let out = retry_dispatch(
            "run_office_js",
            json!({ "doc_full_name": doc, "script": "return 42;", "mode": mode }),
        )
        .await
        .expect("run_office_js dispatch round-trip");
        let sc = out.get("structuredContent").cloned().unwrap_or(Value::Null);
        assert_eq!(sc.get("got_method").and_then(|m| m.as_str()), Some("run_office_js"));
        assert_eq!(
            sc.get("got_params").and_then(|p| p.get("script")).and_then(|s| s.as_str()),
            Some("return 42;"),
            "pane received the script param (mode {mode})"
        );
        assert_eq!(
            sc.get("got_params").and_then(|p| p.get("mode")).and_then(|m| m.as_str()),
            Some(mode),
            "pane received the mode param unchanged (mode {mode})"
        );
    }

    let out = retry_dispatch(
        "run_office_js",
        json!({ "doc_full_name": doc, "script": "return 42;", "mode": "read" }),
    )
    .await
    .expect("run_office_js dispatch round-trip");
    // The pane reply's `text` is surfaced into the readable content channel via the
    // shared `pane_tool_result` mapping (the same path that carries the real
    // `{result,truncated,text}` reply — here the mock's `text` is "MOCK BODY").
    assert_eq!(
        out.get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str()),
        Some("MOCK BODY"),
        "pane text surfaced in the readable content channel"
    );

    handle.shutdown();
    pane.abort();
}

/// Shared live-Excel bring-up for the `#[ignore]` run_office_js live tests (macOS
/// only): bind the fixed 44300 against the app's trusted cert, open Excel with a
/// selected cell, and wait for a real task pane to register (the one manual step —
/// clicking the ribbon button — an Office add-in pane can't be opened by automation).
/// Returns the bridge handle + the connected pane's target key.
///
/// Run these live tests ONE AT A TIME (they all bind the
/// fixed port 44300 and each needs a manual ribbon click, so they cannot run
/// concurrently): name a single test on the `--ignored` cargo invocation, and quit the
/// desktop app first so nothing else holds 44300.
#[cfg(target_os = "macos")]
async fn open_excel_and_wait_for_pane() -> (server::BridgeHandle, String) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info,ziee_desktop::modules::office_bridge=debug")
        .with_test_writer()
        .try_init();
    let home = std::env::var("HOME").expect("HOME");
    let data_dir =
        std::path::PathBuf::from(home).join("Library/Application Support/com.ziee.chat");
    let handle = server::start(44300, data_dir)
        .await
        .expect("bridge binds 44300 (quit the desktop app first if this fails)");

    let _ = std::process::Command::new("open")
        .args(["-a", "Microsoft Excel"])
        .status();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(
            r#"tell application "Microsoft Excel"
                activate
                if (count of workbooks) = 0 then make new workbook
                select range "A1" of active sheet of active workbook
            end tell"#,
        )
        .status();

    eprintln!("\n>>> LIVE run_office_js: Excel is open. NOW click the ribbon: Home -> Ziee -> 'Show Ziee Bridge'.");
    eprintln!(">>> Waiting up to 600s (10 min) for the pane to connect — no rush...\n");
    let mut target = String::new();
    for i in 0..600 {
        if let Some(key) = broker::connected_pane_keys().into_iter().next() {
            target = if key.is_empty() {
                "Untitled".to_string()
            } else {
                key
            };
            eprintln!(">>> pane connected (target = {target:?})");
            break;
        }
        if i % 10 == 0 && i > 0 {
            eprintln!(">>> still waiting for a pane... ({i}s)");
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert!(!target.is_empty(), "a live task pane must connect");
    (handle, target)
}

/// TEST-8 (office-run-office-js, live `#[ignore]`) — a hardcoded real Office.js script
/// passed to `run_office_js` executes in a live Excel task pane and its `return` value
/// round-trips. Same one manual ribbon-click setup as the other live run_office_js tests.
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "live: runs real Office.js in an Excel task pane on this macOS session"]
async fn run_office_js_live_mac_executes_script() {
    let (handle, target) = open_excel_and_wait_for_pane().await;
    let script = "const s = context.workbook.worksheets.getActiveWorksheet();\n\
                  const r = s.getRange('A1');\n\
                  r.values = [['ziee-run']];\n\
                  r.load('address');\n\
                  await context.sync();\n\
                  return r.address;";
    let out = broker::call_pane_with_timeout(
        &target,
        "run_office_js",
        json!({ "doc_full_name": target, "script": script }),
        Duration::from_secs(20),
    )
    .await
    .expect("run_office_js round-trips through the live pane");
    eprintln!(">>> run_office_js returned: {out}");
    let result_str = out.get("result").map(|r| r.to_string()).unwrap_or_default();
    assert!(
        result_str.contains("A1"),
        "the returned range address should contain A1: {out}"
    );
    handle.shutdown();
}

/// TEST-11 (office-run-office-js, real-LLM + live `#[ignore]`) — a REAL
/// OpenAI-compatible model, given the SHIPPED `run_office_js` tool schema, emits a
/// tool call whose Office.js `script` then executes in the live Excel pane and returns
/// a value. Proves the end-to-end: real model writes valid Office.js against the real
/// schema → the real pane runs it. Soft-skips when `ZIEE_OFFICE_REAL_LLM_URL` is unset
/// (DEC-6: point it at the coder.ziee LiteLLM `:4000` via an SSH tunnel).
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "live+real-LLM: needs ZIEE_OFFICE_REAL_LLM_URL + an Excel task pane"]
async fn run_office_js_real_llm_live() {
    let url = match std::env::var("ZIEE_OFFICE_REAL_LLM_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            eprintln!("SKIP run_office_js_real_llm_live: ZIEE_OFFICE_REAL_LLM_URL unset");
            return;
        }
    };
    let model =
        std::env::var("ZIEE_OFFICE_REAL_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".to_string());
    let key = std::env::var("ZIEE_OFFICE_REAL_LLM_KEY").ok();

    // Advertise the ACTUAL run_office_js descriptor we ship to the real model.
    let list = ziee_desktop::modules::office_bridge::tools::tool_list();
    let tool = list["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "run_office_js")
        .expect("run_office_js in tool_list")
        .clone();
    let body = json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": "Use the run_office_js tool on the open workbook to set cell A1 of the active worksheet to the text 'hello' and return A1's address."
        }],
        "tools": [{ "type": "function", "function": {
            "name": tool["name"],
            "description": tool["description"],
            "parameters": tool["inputSchema"],
        }}],
        "tool_choice": "auto",
        "max_tokens": 500
    });
    let mut req = reqwest::Client::new()
        .post(&url)
        .header("content-type", "application/json")
        .body(body.to_string());
    if let Some(k) = key {
        req = req.header("authorization", format!("Bearer {k}"));
    }
    let resp = req
        .send()
        .await
        .expect("LLM request sent")
        .text()
        .await
        .expect("LLM response body");
    let v: Value = serde_json::from_str(&resp).unwrap_or_else(|e| panic!("LLM json ({e}): {resp}"));
    let func = v["choices"][0]["message"]["tool_calls"][0]["function"].clone();
    assert_eq!(
        func["name"], "run_office_js",
        "the model must call run_office_js: {resp}"
    );
    let args: Value = serde_json::from_str(
        func["arguments"]
            .as_str()
            .expect("tool-call arguments is a JSON string"),
    )
    .expect("tool-call arguments parse");
    let script = args["script"]
        .as_str()
        .expect("model produced a `script` argument")
        .to_string();
    eprintln!(">>> model-written run_office_js script:\n{script}\n");

    // Execute the model's own script in the live pane.
    let (handle, target) = open_excel_and_wait_for_pane().await;
    let out = broker::call_pane_with_timeout(
        &target,
        "run_office_js",
        json!({ "doc_full_name": target, "script": script }),
        Duration::from_secs(30),
    )
    .await
    .expect("the model's run_office_js script executes in the live pane");
    eprintln!(">>> run_office_js returned: {out}");

    // Verify the model's script actually had an EFFECT (not just that a reply came
    // back): read A1 back with a deterministic (non-LLM) script and assert it is
    // non-empty — the model was asked to set A1 to 'hello'.
    let verify = broker::call_pane_with_timeout(
        &target,
        "run_office_js",
        json!({
            "doc_full_name": target,
            "script": "const r = context.workbook.worksheets.getActiveWorksheet().getRange('A1'); r.load('values'); await context.sync(); return r.values[0][0];",
        }),
        Duration::from_secs(20),
    )
    .await
    .expect("A1 read-back executes");
    eprintln!(">>> A1 read-back: {verify}");
    let a1 = verify.get("result").map(|r| r.to_string()).unwrap_or_default();
    // Assert A1 holds the requested value ('hello'), not merely non-empty — the helper
    // reuses an existing workbook and never clears A1, so a stale value from another
    // live test could mask a silently-failed model write. Case-insensitive for model
    // phrasing variance.
    assert!(
        a1.to_lowercase().contains("hello"),
        "the model's script set A1 to the requested 'hello' (got {a1}): {verify}"
    );
    handle.shutdown();
}

/// TEST-14 (office-mode-gated-approval, real-LLM) — validates the trust-based model's
/// load-bearing assumption: given the SHIPPED `run_office_js` schema, a real model
/// reliably self-classifies `mode` — `"read"` for a pure-read task and `"write"` for a
/// mutating task. That self-classification is exactly what the server approval loop
/// gates on (read → auto-run, write → prompt). Soft-skips when `ZIEE_OFFICE_REAL_LLM_URL`
/// is unset. No Excel pane needed — this exercises only the model + the tool schema.
///
/// Gated by a runtime SOFT-SKIP (not `#[ignore]`): when `ZIEE_OFFICE_REAL_LLM_URL` is
/// unset it returns immediately, so a default `cargo test` never fires a live call. This
/// is the lifecycle-preferred gate (A3 forbids `#[ignore]`); it's why this test — unlike
/// the live-*Excel-pane* tests, which physically need a `#[ignore]` opt-in — stays live.
#[tokio::test]
async fn run_office_js_real_llm_declares_mode() {
    let url = match std::env::var("ZIEE_OFFICE_REAL_LLM_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            eprintln!("SKIP run_office_js_real_llm_declares_mode: ZIEE_OFFICE_REAL_LLM_URL unset");
            return;
        }
    };
    let model =
        std::env::var("ZIEE_OFFICE_REAL_LLM_MODEL").unwrap_or_else(|_| "qwen3.6-35b-a3b".to_string());
    let key = std::env::var("ZIEE_OFFICE_REAL_LLM_KEY").ok();

    let list = ziee_desktop::modules::office_bridge::tools::tool_list();
    let tool = list["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "run_office_js")
        .expect("run_office_js in tool_list")
        .clone();

    // (task prompt, expected mode)
    let cases = [
        ("Read cell A1 of the active worksheet and tell me its value. Use the run_office_js tool.", "read"),
        ("Set cell A1 of the active worksheet to the text 'hello'. Use the run_office_js tool.", "write"),
    ];
    for (task, expected) in cases {
        let body = json!({
            "model": model,
            "messages": [{ "role": "user", "content": task }],
            "tools": [{ "type": "function", "function": {
                "name": tool["name"], "description": tool["description"], "parameters": tool["inputSchema"] }}],
            "tool_choice": "auto",
            "max_tokens": 500
        });
        let mut req = reqwest::Client::new()
            .post(&url)
            .header("content-type", "application/json")
            .body(body.to_string());
        if let Some(k) = &key {
            req = req.header("authorization", format!("Bearer {k}"));
        }
        let resp = req.send().await.expect("LLM request").text().await.expect("LLM body");
        let v: Value = serde_json::from_str(&resp).unwrap_or_else(|e| panic!("LLM json ({e}): {resp}"));
        let func = v["choices"][0]["message"]["tool_calls"][0]["function"].clone();
        assert_eq!(func["name"], "run_office_js", "model must call run_office_js for: {task}\n{resp}");
        let args: Value = serde_json::from_str(func["arguments"].as_str().expect("arguments string"))
            .expect("tool-call arguments parse");
        let mode = args["mode"].as_str().unwrap_or("<missing>");
        eprintln!(">>> task={task:?} → mode={mode:?}");
        assert_eq!(
            mode, expected,
            "model must declare mode={expected:?} for task {task:?} (got {mode:?})"
        );
    }
}

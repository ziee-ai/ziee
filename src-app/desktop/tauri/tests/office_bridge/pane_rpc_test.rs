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

/// Shared live-Excel bring-up for the opt-in (env-gated) run_office_js live tests
/// (macOS only): bind the fixed 44300 against the app's trusted cert, open Excel with a
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
    open_office_app_and_wait(
        "Microsoft Excel",
        r#"tell application "Microsoft Excel"
                activate
                if (count of workbooks) = 0 then make new workbook
                select range "A1" of active sheet of active workbook
            end tell"#,
    )
    .await
}

/// Live Word bring-up: open Word with a short starter document that contains the
/// word "budget", so the human "review pass" task has a real comment target.
#[cfg(target_os = "macos")]
async fn open_word_and_wait_for_pane() -> (server::BridgeHandle, String) {
    open_office_app_and_wait(
        "Microsoft Word",
        "tell application \"Microsoft Word\"\n\
                activate\n\
                if (count of documents) = 0 then make new document\n\
                set content of text object of active document to \"Q3 Draft Report\" & return & \"The marketing budget needs review before the board meeting on Friday.\" & return & \"Please add your notes below.\"\n\
            end tell",
    )
    .await
}

/// Live PowerPoint bring-up: open PowerPoint with an empty presentation. Requires
/// macOS Automation (TCC) consent for PowerPoint (System Settings → Privacy →
/// Automation), otherwise the osascript seed is denied (error -10003).
#[cfg(target_os = "macos")]
async fn open_powerpoint_and_wait_for_pane() -> (server::BridgeHandle, String) {
    open_office_app_and_wait(
        "Microsoft PowerPoint",
        "tell application \"Microsoft PowerPoint\"\n\
                activate\n\
                if (count of presentations) = 0 then make new presentation\n\
            end tell",
    )
    .await
}

/// Shared live bring-up (macOS): bind the fixed 44300 against the app's trusted
/// cert, open `app`, run `setup_osascript` to seed a document, then wait for the
/// one manual step — clicking the ribbon button — to connect a task pane (an
/// Office add-in pane can't be opened by automation). Returns the bridge handle +
/// the connected pane's target key.
///
/// Run these live tests ONE AT A TIME (they all bind the fixed port 44300 and each
/// needs a manual ribbon click, so they cannot run concurrently): name a single
/// test on the cargo invocation, and quit the desktop app first so nothing else
/// holds 44300.
#[cfg(target_os = "macos")]
async fn open_office_app_and_wait(
    app: &str,
    setup_osascript: &str,
) -> (server::BridgeHandle, String) {
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

    let _ = std::process::Command::new("open").args(["-a", app]).status();
    tokio::time::sleep(Duration::from_secs(5)).await;
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(setup_osascript)
        .status();

    eprintln!("\n>>> LIVE run_office_js: {app} is open. NOW click the ribbon: Home -> Ziee -> 'Show Ziee Bridge'.");
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

/// TEST-8 (office-run-office-js, live, env-gated) — a hardcoded real Office.js script
/// passed to `run_office_js` executes in a live Excel task pane and its `return` value
/// round-trips. Same one manual ribbon-click setup as the other live run_office_js tests.
///
/// Runtime SOFT-SKIP (not an ignore attribute — A3): returns immediately unless
/// `ZIEE_OFFICE_LIVE=1` (with an Excel task pane open), so a default `cargo test` never
/// binds 44300 or opens Excel.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn run_office_js_live_mac_executes_script() {
    if std::env::var("ZIEE_OFFICE_LIVE").is_err() {
        eprintln!("SKIP run_office_js_live_mac_executes_script: set ZIEE_OFFICE_LIVE=1 with an Excel task pane open");
        return;
    }
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

/// TEST-11 (office-run-office-js, real-LLM + live, env-gated) — a REAL
/// OpenAI-compatible model, given the SHIPPED `run_office_js` tool schema, emits a
/// tool call whose Office.js `script` then executes in the live Excel pane and returns
/// a value. Proves the end-to-end: real model writes valid Office.js against the real
/// schema → the real pane runs it. Runtime SOFT-SKIP (not an ignore attribute — A3):
/// returns immediately when `ZIEE_OFFICE_REAL_LLM_URL` is unset
/// (DEC-6: point it at the coder.ziee LiteLLM `:4000` via an SSH tunnel).
#[cfg(target_os = "macos")]
#[tokio::test]
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
/// Gated by a runtime SOFT-SKIP (no ignore attribute): when `ZIEE_OFFICE_REAL_LLM_URL`
/// is unset it returns immediately, so a default `cargo test` never fires a live call.
/// This is the lifecycle-preferred gate (A3 forbids ignore-attributes); every live test
/// in this module uses the same env-gated soft-skip instead.
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

// ============================================================================
// Human-like real-LLM live tests — realistic, research-backed tasks a person
// actually does, across Excel / Word / PowerPoint. Each drives the REAL model
// (coder.ziee) to write ONE multi-step `run_office_js` Office.js script, runs the
// model's OWN script in the live pane, then VERIFIES the document actually changed
// via a deterministic (non-LLM) read-back — not a toy "set A1 to hello".
//
// Scenarios grounded in common tutorials/showcases:
//   - Excel: a monthly budget — Category/Planned/Actual headers (bold), expense
//     rows, a Total row with SUM, currency-formatted amounts.
//   - Word: a document review pass — a Heading-1 heading, a tracked-changes edit,
//     and a comment on the word "budget".
//   - PowerPoint: an agenda slide — a new slide with a title + agenda items.
//
// SOFT-SKIP (A3): return unless BOTH `ZIEE_OFFICE_LIVE=1` (a live pane) and
// `ZIEE_OFFICE_REAL_LLM_URL` (the real model) are set. Run ONE AT A TIME (each
// binds 44300 + needs a manual ribbon click). PowerPoint also needs macOS
// Automation (TCC) consent.
// ============================================================================

/// Both the live-pane and real-LLM gates set? Else print SKIP and return false.
#[cfg(target_os = "macos")]
fn live_llm_ready(test: &str) -> bool {
    if std::env::var("ZIEE_OFFICE_LIVE").is_err() {
        eprintln!("SKIP {test}: set ZIEE_OFFICE_LIVE=1 (needs a live Office task pane)");
        return false;
    }
    if std::env::var("ZIEE_OFFICE_REAL_LLM_URL")
        .map(|u| u.is_empty())
        .unwrap_or(true)
    {
        eprintln!("SKIP {test}: set ZIEE_OFFICE_REAL_LLM_URL (real model via coder.ziee)");
        return false;
    }
    true
}

/// Ask the REAL model to write ONE `run_office_js` script for `prompt` against the
/// SHIPPED tool schema; return the model's `script` argument. Assumes the LLM env
/// is set (guarded by `live_llm_ready`).
#[cfg(target_os = "macos")]
async fn model_writes_run_office_js(prompt: &str) -> String {
    let url = std::env::var("ZIEE_OFFICE_REAL_LLM_URL").expect("ZIEE_OFFICE_REAL_LLM_URL");
    let model = std::env::var("ZIEE_OFFICE_REAL_LLM_MODEL")
        .unwrap_or_else(|_| "qwen3.6-35b-a3b".to_string());
    let key = std::env::var("ZIEE_OFFICE_REAL_LLM_KEY").ok();
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
        "messages": [{ "role": "user", "content": prompt }],
        "tools": [{ "type": "function", "function": {
            "name": tool["name"],
            "description": tool["description"],
            "parameters": tool["inputSchema"],
        }}],
        "tool_choice": "auto",
        "max_tokens": 1500
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
        .expect("model produced a `script`")
        .to_string();
    eprintln!(">>> model-written run_office_js script:\n{script}\n");
    script
}

/// Generate the model's `run_office_js` script for `prompt`, run it in the live
/// pane, then run the deterministic `verify` script and check `ok(&result)`.
/// REGENERATE and retry (up to 3 attempts — real models are non-deterministic)
/// when EITHER the model's script errors in the pane (e.g. an Office.js
/// array-dimension mismatch) OR it "runs" but doesn't actually do the task (a
/// no-op / dummy script — `ok` returns false). Returns the last verified result
/// (the caller's assertions then report specifics). The pane stays connected
/// across attempts, so a retry needs NO extra ribbon click.
#[cfg(target_os = "macos")]
async fn model_task_and_verify(
    target: &str,
    prompt: &str,
    verify: &str,
    ok: impl Fn(&Value) -> bool,
) -> Value {
    let mut last = Value::Null;
    for attempt in 1..=3 {
        let script = model_writes_run_office_js(prompt).await;
        match broker::call_pane_with_timeout(
            target,
            "run_office_js",
            json!({ "doc_full_name": target, "script": script }),
            Duration::from_secs(45),
        )
        .await
        {
            Ok(out) => {
                eprintln!(">>> attempt {attempt}: model script ran OK: {out}");
                let v = broker::call_pane_with_timeout(
                    target,
                    "run_office_js",
                    json!({ "doc_full_name": target, "script": verify }),
                    Duration::from_secs(30),
                )
                .await
                .expect("verify read-back executes");
                eprintln!(">>> verify read-back: {v}");
                last = v.get("result").cloned().unwrap_or(Value::Null);
                if ok(&last) {
                    return last;
                }
                eprintln!(
                    ">>> attempt {attempt}: the task did NOT complete (no-op / incomplete script), regenerating..."
                );
            }
            Err(e) => {
                eprintln!(
                    ">>> attempt {attempt}: the model's script FAILED in the pane, regenerating — {e}"
                );
            }
        }
    }
    last
}

/// Human-like Excel task — a real monthly budget. The model builds it; we verify
/// the structure, the SUM totals (Planned 2550 / Actual 2495), the bold header,
/// and currency formatting.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn human_excel_monthly_budget() {
    if !live_llm_ready("human_excel_monthly_budget") {
        return;
    }
    let prompt = "Use the run_office_js tool on the open Excel workbook to build a simple monthly \
        budget on the active worksheet, all in ONE script. Put the headers Category, Planned, and \
        Actual in A1, B1, C1, and make the header cells A1:C1 bold. Starting in row 2 add these four \
        rows exactly as (Category, Planned, Actual): Rent 1500 1450; Groceries 600 640; \
        Transport 200 175; Utilities 250 230. In the next row put the word Total in column A and use \
        SUM formulas in columns B and C to total the Planned and Actual amounts. Format the amount \
        cells B2:C6 as US-dollar currency.";
    let (handle, target) = open_excel_and_wait_for_pane().await;
    let verify = "const s = context.workbook.worksheets.getActiveWorksheet(); \
        const r = s.getRange('A1:C7'); r.load('values, text'); \
        const h = s.getRange('A1'); h.load('format/font/bold'); \
        const amt = s.getRange('B2'); amt.load('numberFormat'); \
        await context.sync(); \
        return { values: r.values, text: r.text, a1Bold: h.format.font.bold, amtFmt: amt.numberFormat[0][0] };";
    let res = model_task_and_verify(&target, prompt, verify, |r| {
        let f = r.to_string();
        f.contains("2550") || f.contains("2,550")
    })
    .await;
    handle.shutdown();

    let flat = res.to_string().to_lowercase();
    assert!(
        flat.contains("category") && flat.contains("planned") && flat.contains("actual"),
        "budget headers present: {res}"
    );
    assert!(flat.contains("total"), "a Total row is present: {res}");
    // SUM correctness — Planned 1500+600+200+250 = 2550; Actual 1450+640+175+230 = 2495.
    // Tolerate the currency thousands separator ("2,550") as well as the raw value.
    assert!(
        flat.contains("2550") || flat.contains("2,550"),
        "the Planned SUM total (2550) is present: {res}"
    );
    assert!(
        flat.contains("2495") || flat.contains("2,495"),
        "the Actual SUM total (2495) is present: {res}"
    );
    assert_eq!(res["a1Bold"].as_bool(), Some(true), "the header cell is bold: {res}");
    assert!(
        res["amtFmt"].as_str().unwrap_or("").contains('$'),
        "the amounts are currency-formatted: {res}"
    );
}

/// Human-like Word task — a real review pass. The model adds a Heading-1 heading,
/// turns on tracked changes, appends a paragraph, and comments on "budget"; we
/// verify each landed.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn human_word_review_pass() {
    if !live_llm_ready("human_word_review_pass") {
        return;
    }
    let prompt = "Use the run_office_js tool on the open Word document to do a quick review pass, \
        all in ONE script: (1) at the very START of the document body, insert a new paragraph with \
        the text 'Executive Summary' and set that paragraph's style to the built-in Heading 1 style. \
        (2) Turn ON tracked changes for the document. (3) At the END of the document body, insert a \
        new paragraph with the text 'Reviewed and approved for Q3.' (4) Search the body for the word \
        'budget' (case-insensitive) and on the first match insert a comment with the text \
        'Please confirm the Q3 figure.'";
    let (handle, target) = open_word_and_wait_for_pane().await;
    let verify = "const body = context.document.body; body.load('text'); \
        context.document.load('changeTrackingMode'); \
        const paras = body.paragraphs; paras.load('items/text, items/styleBuiltIn'); \
        let commentCount = -1; \
        try { const cs = body.getComments(); cs.load('items'); await context.sync(); commentCount = cs.items.length; } catch (e) { commentCount = -1; } \
        await context.sync(); \
        return { text: body.text, tracking: context.document.changeTrackingMode, \
            firstStyle: paras.items.length ? paras.items[0].styleBuiltIn : '', commentCount };";
    let res = model_task_and_verify(&target, prompt, verify, |r| {
        r["text"].as_str().unwrap_or("").to_lowercase().contains("executive summary")
            && r["tracking"].as_str() != Some("Off")
    })
    .await;
    handle.shutdown();

    let text = res["text"].as_str().unwrap_or("").to_lowercase();
    assert!(
        text.contains("executive summary"),
        "the 'Executive Summary' heading was inserted: {res}"
    );
    assert!(
        text.contains("reviewed and approved for q3"),
        "the tracked paragraph was appended: {res}"
    );
    assert!(
        res["firstStyle"].as_str().unwrap_or("").to_lowercase().contains("heading"),
        "the first paragraph is Heading-1 styled: {res}"
    );
    assert_ne!(
        res["tracking"].as_str(),
        Some("Off"),
        "tracked changes is ON: {res}"
    );
    let comments = res["commentCount"].as_i64().unwrap_or(-1);
    assert!(
        comments >= 1,
        "a comment was inserted (getComments returned {comments}): {res}"
    );
}

/// Human-like PowerPoint task — a real agenda slide. The model adds a slide with a
/// title + agenda items; we verify the text appears on a slide.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn human_powerpoint_agenda_slide() {
    if !live_llm_ready("human_powerpoint_agenda_slide") {
        return;
    }
    let prompt = "Use the run_office_js tool to add an agenda slide to the open PowerPoint \
        presentation. Write ONE Office.js script (it runs inside PowerPoint.run with `context` in \
        scope) that: (1) adds a new slide with `context.presentation.slides.add();` (2) loads the \
        slide collection (`context.presentation.slides.load('items'); await context.sync();`) and \
        takes the LAST slide as the new one: `const slide = context.presentation.slides.getItemAt(context.presentation.slides.items.length - 1);` \
        (3) on that slide adds a title text box: `const t = slide.shapes.addTextBox(\"Today's Agenda\"); t.left = 50; t.top = 30; t.width = 600; t.height = 60;` \
        (4) adds a second text box with the three agenda items on separate lines (use a newline \\n \
        between them): `const b = slide.shapes.addTextBox(\"Q3 results\\nProduct roadmap\\nHiring plan\"); b.left = 50; b.top = 130; b.width = 600; b.height = 220;` \
        (5) `await context.sync();` and returns a short confirmation string. Use exactly those texts.";
    let (handle, target) = open_powerpoint_and_wait_for_pane().await;
    let verify = "const pres = context.presentation; pres.slides.load('items'); await context.sync(); \
        const texts = []; \
        for (const slide of pres.slides.items) { \
            slide.shapes.load('items'); await context.sync(); \
            for (const sh of slide.shapes.items) { try { sh.textFrame.textRange.load('text'); } catch (e) {} } \
            await context.sync(); \
            for (const sh of slide.shapes.items) { try { texts.push(sh.textFrame.textRange.text); } catch (e) {} } \
        } \
        return { slideCount: pres.slides.items.length, texts };";
    let res = model_task_and_verify(&target, prompt, verify, |r| {
        r["texts"].to_string().to_lowercase().contains("agenda")
    })
    .await;
    handle.shutdown();

    let flat = res["texts"].to_string().to_lowercase();
    assert!(
        flat.contains("agenda"),
        "the agenda title is present on a slide: {res}"
    );
    assert!(flat.contains("q3 results"), "agenda item 'Q3 results' present: {res}");
    assert!(flat.contains("product roadmap"), "agenda item 'Product roadmap' present: {res}");
    assert!(flat.contains("hiring plan"), "agenda item 'Hiring plan' present: {res}");
}

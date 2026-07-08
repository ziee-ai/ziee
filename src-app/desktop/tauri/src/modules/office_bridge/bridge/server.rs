//! The standalone dual-stack HTTPS + WSS bridge listener (ITEM-5).
//!
//! Serves the embedded Office task-pane bundle over rustls on **both**
//! `127.0.0.1:port` and `[::1]:port` (DEC-5 — WebView2 resolves `localhost` to
//! `::1`), with:
//!
//! - `GET /taskpane.html` (and `/`) — the task pane, with a fresh per-session
//!   token minted + registered in [`auth`] and injected in place of the
//!   `__ZIEE_BRIDGE_TOKEN__` placeholder (DEC-6).
//! - `GET /taskpane.js`, `GET /icon.png` — the remaining embedded assets.
//! - `GET /bridge` — a `WebSocketUpgrade` guarded by the **Origin allowlist**
//!   AND a valid **per-session token** (carried as a `Sec-WebSocket-Protocol`
//!   value, DEC-6); on accept it runs the JSON-RPC duplex ([`handle_socket`])
//!   that the daemon↔pane broker ([`broker`]) drives — ITEM-9.
//! - token-guarded `POST /report|/caps|/selection|/comment` — dev/diagnostic
//!   sinks that accept JSON and 200.
//!
//! The rustls `ServerConfig` is built from the cached/minted bridge cert
//! ([`cert`]) with the explicit ring provider; `axum-server`'s
//! `from_tcp_rustls` serves our pre-bound TCP sockets (bound synchronously so
//! the listener is up before [`start`] returns, and so an ephemeral `port == 0`
//! yields a concrete port reused for both address families). A failed `[::1]`
//! bind is logged and skipped — the bridge still runs on v4.

use std::net::{Ipv4Addr, Ipv6Addr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_server::tls_rustls::RustlsConfig;

use tokio::sync::mpsc;

use ziee::AppError;

use super::protocol::BridgeResponse;
use super::{assets, auth, broker, cert};

/// The WSS subprotocol the task pane offers (alongside the token) and the
/// bridge selects on accept. Kept in sync with `resources/office-bridge/taskpane.js`.
const SUBPROTOCOL: &str = "ziee-bridge";

/// Error code [`start`] returns when the requested TCP port is already bound by
/// another process. Distinct from a generic bind failure so the caller can apply
/// the "auto-migrate the port if not yet sideloaded, else surface it" policy
/// (see `office_bridge::register_office_bridge`).
pub const PORT_IN_USE_CODE: &str = "OFFICE_BRIDGE_PORT_IN_USE";

/// Bind an ephemeral loopback port, read the number the OS assigned, and release
/// it — yielding a port that was free a moment ago. There is a small TOCTOU
/// window (another process could claim it before the caller re-binds); the caller
/// binds immediately after and treats a second failure as fatal.
pub fn find_free_loopback_port() -> Option<u16> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

/// A running bridge listener. Holds the shared `axum-server` shutdown handle so
/// callers can stop both address-family servers at once.
pub struct BridgeHandle {
    /// The port both listeners bound (the concrete port even when `start` was
    /// called with `0` for an ephemeral bind).
    pub port: u16,
    /// The canonical served origin (`https://localhost:<port>`).
    pub origin: String,
    /// Shared shutdown handle for the v4 (+ v6) servers.
    handle: axum_server::Handle,
}

impl BridgeHandle {
    /// Immediately stop both listeners.
    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}

/// State shared into the bridge handlers — currently just the Origin allowlist.
#[derive(Clone)]
struct BridgeState {
    allowed_origins: Arc<Vec<String>>,
}

/// The set of Origins the `/bridge` upgrade accepts: the served loopback origins
/// on the bound port, plus the fixed production origin (`https://localhost:44300`,
/// the manifest `SourceLocation` WebView2 loads from).
fn allowed_origins(port: u16) -> Vec<String> {
    let mut v = vec![
        format!("https://localhost:{port}"),
        format!("https://127.0.0.1:{port}"),
        format!("https://[::1]:{port}"),
    ];
    let prod = "https://localhost:44300".to_string();
    if !v.contains(&prod) {
        v.push(prod);
    }
    v
}

/// Start the dual-stack bridge listener. Mints/loads the bridge cert under
/// `data_dir`, builds the rustls `ServerConfig`, and serves the [`build_router`]
/// Router on `127.0.0.1:port` and (best-effort) `[::1]:port`. Pass `port == 0`
/// for an ephemeral port (tests); the returned [`BridgeHandle`] carries the
/// concrete port.
pub async fn start(port: u16, data_dir: PathBuf) -> Result<BridgeHandle, AppError> {
    let minted = cert::load_or_mint(&data_dir)?;
    // Serve the full chain (leaf + CA) so a client trusting the CA validates the
    // presented leaf; the leaf's private key is the server key.
    let server_config = cert::build_server_config(&minted.chain_pem, &minted.leaf_key_pem)?;
    let tls = RustlsConfig::from_config(Arc::new(server_config));

    // Bind v4 synchronously so the socket is listening before we return, and so
    // an ephemeral port==0 resolves to a concrete port we can reuse for v6. An
    // `AddrInUse` failure is reported with a distinct code so the caller can
    // migrate the port (when the add-in has not been sideloaded yet) instead of
    // silently leaving the bridge down.
    let l4 = match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            return Err(AppError::new(
                StatusCode::CONFLICT,
                PORT_IN_USE_CODE,
                format!("office_bridge: TCP port {port} is already in use"),
            ));
        }
        Err(e) => return Err(bind_err(format!("bind 127.0.0.1:{port}: {e}"))),
    };
    let actual_port = l4
        .local_addr()
        .map_err(|e| bind_err(format!("read bound v4 addr: {e}")))?
        .port();
    l4.set_nonblocking(true)
        .map_err(|e| bind_err(format!("set v4 nonblocking: {e}")))?;

    // v6 is best-effort — a host without IPv6 loopback simply runs v4 (DEC-5).
    let l6 = match TcpListener::bind((Ipv6Addr::LOCALHOST, actual_port)) {
        Ok(l) => match l.set_nonblocking(true) {
            Ok(()) => Some(l),
            Err(e) => {
                tracing::warn!("office_bridge: set [::1] nonblocking failed: {e}; v4 only");
                None
            }
        },
        Err(e) => {
            tracing::warn!(
                "office_bridge: bind [::1]:{actual_port} failed ({e}); continuing v4-only"
            );
            None
        }
    };

    let state = BridgeState {
        allowed_origins: Arc::new(allowed_origins(actual_port)),
    };
    let router = build_router(state);
    let handle = axum_server::Handle::new();

    // v4 server.
    {
        let h = handle.clone();
        let tls = tls.clone();
        let make = router.clone().into_make_service();
        tokio::spawn(async move {
            if let Err(e) = axum_server::from_tcp_rustls(l4, tls)
                .handle(h)
                .serve(make)
                .await
            {
                tracing::error!("office_bridge: v4 bridge listener exited: {e}");
            }
        });
    }

    // v6 server (only if the bind succeeded).
    if let Some(l6) = l6 {
        let h = handle.clone();
        let tls = tls.clone();
        let make = router.into_make_service();
        tokio::spawn(async move {
            if let Err(e) = axum_server::from_tcp_rustls(l6, tls)
                .handle(h)
                .serve(make)
                .await
            {
                tracing::warn!("office_bridge: [::1] bridge listener exited: {e}");
            }
        });
    }

    let origin = format!("https://localhost:{actual_port}");
    tracing::info!(
        "office_bridge: bridge listening on {origin} (dual-stack; cert fp {})",
        minted.fingerprint
    );
    Ok(BridgeHandle {
        port: actual_port,
        origin,
        handle,
    })
}

/// Build the bridge Router (assets + WSS upgrade + POST sinks).
fn build_router(state: BridgeState) -> Router {
    Router::new()
        .route("/", get(serve_taskpane))
        .route("/taskpane.html", get(serve_taskpane))
        .route("/taskpane.js", get(serve_taskpane_js))
        .route("/icon.png", get(serve_icon))
        .route("/bridge", get(bridge_ws))
        .route("/report", post(post_sink))
        .route("/caps", post(post_sink))
        .route("/selection", post(post_sink))
        .route("/comment", post(post_sink))
        .with_state(state)
}

/// Serve `taskpane.html` with a freshly-minted per-session token injected in
/// place of the quoted `"__ZIEE_BRIDGE_TOKEN__"` placeholder (DEC-6). Only the
/// quoted value is replaced — never the JS variable name
/// `window.__ZIEE_BRIDGE_TOKEN__`, which contains the placeholder as a substring.
async fn serve_taskpane() -> Response {
    let bytes = match assets::get("taskpane.html") {
        Some(b) => b,
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "taskpane.html missing").into_response()
        }
    };
    let token = auth::new_session_token();
    let html = String::from_utf8_lossy(bytes);
    let injected = html.replace("\"__ZIEE_BRIDGE_TOKEN__\"", &format!("\"{token}\""));
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        injected,
    )
        .into_response()
}

async fn serve_taskpane_js() -> Response {
    serve_static("taskpane.js")
}

async fn serve_icon() -> Response {
    serve_static("icon.png")
}

/// Serve an embedded asset verbatim with its content-type.
fn serve_static(name: &str) -> Response {
    match assets::get(name) {
        Some(b) => (
            [(header::CONTENT_TYPE, assets::content_type(name))],
            b.to_vec(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Whether the request's `Origin` header is on the allowlist. Shared by the
/// `/bridge` WSS upgrade and the POST sinks so both enforce the SAME allowlist
/// (a token-bearing but cross-origin caller is rejected at either surface).
fn origin_allowed(state: &BridgeState, headers: &HeaderMap) -> bool {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    state.allowed_origins.iter().any(|o| o == origin)
}

/// `GET /bridge` — enforce Origin + token, then upgrade to a WSS echo socket.
async fn bridge_ws(
    State(state): State<BridgeState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // Origin allowlist (DEC-6) — reject BEFORE the upgrade.
    if !origin_allowed(&state, &headers) {
        let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
        tracing::warn!("office_bridge: /bridge rejected — disallowed origin {origin:?}");
        return (StatusCode::FORBIDDEN, "forbidden origin").into_response();
    }

    // Per-session token: carried as a Sec-WebSocket-Protocol value alongside the
    // `ziee-bridge` subprotocol name (DEC-6 — keeps it out of the URL/query).
    let proto = headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token_ok = proto
        .split(',')
        .map(|s| s.trim())
        .any(|cand| cand != SUBPROTOCOL && !cand.is_empty() && auth::verify(cand));
    if !token_ok {
        tracing::warn!("office_bridge: /bridge rejected — missing/invalid session token");
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }

    // Select the subprotocol back (Chromium/WebView2 abort the socket if the client
    // offered subprotocols and the server selects none), then run the JSON-RPC duplex.
    ws.protocols([SUBPROTOCOL]).on_upgrade(handle_socket)
}

/// Service one task-pane socket as a JSON-RPC duplex (ITEM-9). A single task owns
/// the socket and `tokio::select!`s between:
///   - outbound frames pushed by [`broker::call_pane`] via an mpsc sink, and
///   - inbound frames from the pane.
///
/// Inbound frames are classified by JSON-RPC shape: a frame with `method` is a
/// pane→daemon request/notification (`register` hello → [`broker::register_pane`];
/// `ping`/`selection_changed` → debug-logged), a frame with `result`/`error` is a
/// reply to a daemon→pane request → [`broker::route_response`]. Junk / unclassifiable
/// frames are ignored (the loop keeps running). The pane is unregistered on close.
async fn handle_socket(mut socket: WebSocket) {
    let pane_id = broker::next_pane_id();
    // The sink the broker pushes daemon→pane requests down. A clone is handed to the
    // registry on the pane's `register` hello; this end holds `tx` so `rx` stays open
    // for the socket's lifetime regardless of registration timing.
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    loop {
        tokio::select! {
            // Outbound: a daemon→pane request from `call_pane`.
            out = rx.recv() => {
                match out {
                    Some(msg) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    // All senders dropped (registry entry gone AND local tx gone) —
                    // cannot happen while we hold `tx`, but treat as end-of-life.
                    None => break,
                }
            }
            // Inbound: a frame from the pane.
            inbound = socket.recv() => {
                match inbound {
                    Some(Ok(Message::Text(t))) => classify_pane_frame(pane_id, &tx, t.as_str()),
                    // Binary is unused by the pane protocol; ignore.
                    Some(Ok(Message::Binary(_))) => {}
                    // Ping/Pong are handled by axum's keep-alive.
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    // Clean close or stream end.
                    Some(Ok(Message::Close(_))) | None => break,
                    // Transport error.
                    Some(Err(_)) => break,
                }
            }
        }
    }

    broker::unregister_pane(pane_id);
}

/// Length bounds on the UNTRUSTED `register` identity fields.
const MAX_HOST_LEN: usize = 32;
const MAX_DOC_KEY_LEN: usize = 4096;

/// Truncate untrusted pane input to `max` chars (char-boundary safe).
fn capped(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Classify + handle one inbound text frame from the pane (see [`handle_socket`]).
fn classify_pane_frame(pane_id: u64, tx: &mpsc::UnboundedSender<Message>, text: &str) {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        // Not JSON → ignore (a malformed/junk frame must not kill the socket).
        Err(_) => return,
    };

    // A frame with `method` is a pane→daemon request/notification.
    if let Some(method) = v.get("method").and_then(|m| m.as_str()) {
        match method {
            "register" => {
                // `host`/`doc_key` are UNTRUSTED pane input — bound their length so a
                // compromised pane can't register a pathological identity string.
                let params = v.get("params");
                let host = capped(
                    params.and_then(|p| p.get("host")).and_then(|h| h.as_str()).unwrap_or("unknown"),
                    MAX_HOST_LEN,
                );
                let doc_key = capped(
                    params.and_then(|p| p.get("doc_key")).and_then(|d| d.as_str()).unwrap_or(""),
                    MAX_DOC_KEY_LEN,
                );
                tracing::debug!(
                    "office_bridge: pane {pane_id} registered (host={host}, doc_key_set={})",
                    !doc_key.is_empty()
                );
                broker::register_pane(pane_id, host, doc_key, tx.clone());
            }
            "ping" | "selection_changed" => {
                tracing::trace!("office_bridge: pane {pane_id} {method}");
            }
            other => {
                tracing::debug!("office_bridge: pane {pane_id} unhandled method `{other}`");
            }
        }
        return;
    }

    // Otherwise a frame with `result`/`error` is a reply to a daemon→pane request.
    // Route it bound to THIS pane (broker rejects a corr id routed to another pane).
    if v.get("result").is_some() || v.get("error").is_some() {
        if let Ok(resp) = serde_json::from_value::<BridgeResponse>(v) {
            broker::route_response(pane_id, resp);
        }
    }
    // Neither method nor result/error → ignore.
}

/// Whether a POST carries a valid session token, via `Authorization: Bearer
/// <token>` or the `X-Ziee-Bridge-Token` header.
fn post_token_ok(headers: &HeaderMap) -> bool {
    if let Some(bearer) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        if auth::verify(bearer.trim()) {
            return true;
        }
    }
    if let Some(t) = headers
        .get("x-ziee-bridge-token")
        .and_then(|v| v.to_str().ok())
    {
        if auth::verify(t.trim()) {
            return true;
        }
    }
    false
}

/// Token-guarded dev/diagnostic sink: accept a JSON body and 200. Shared by
/// `/report`, `/caps`, `/selection`, `/comment`.
///
/// Enforces the SAME Origin allowlist as the `/bridge` WSS upgrade (in addition
/// to the session token) so a token-bearing but cross-origin caller is rejected
/// with 403 here too — the token alone is not sufficient.
async fn post_sink(State(state): State<BridgeState>, headers: HeaderMap, body: Bytes) -> Response {
    if !origin_allowed(&state, &headers) {
        let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
        tracing::warn!("office_bridge: POST sink rejected — disallowed origin {origin:?}");
        return (StatusCode::FORBIDDEN, "forbidden origin").into_response();
    }
    if !post_token_ok(&headers) {
        return (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    }
    if serde_json::from_slice::<serde_json::Value>(&body).is_err() {
        return (StatusCode::BAD_REQUEST, "invalid json").into_response();
    }
    (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
}

/// Build the internal error the bind path returns.
fn bind_err(msg: impl Into<String>) -> AppError {
    AppError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "OFFICE_BRIDGE_LISTEN_ERROR",
        msg,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_free_loopback_port_returns_a_bindable_port() {
        let p = find_free_loopback_port().expect("a free loopback port");
        assert_ne!(p, 0);
        // It was free a moment ago, so we can bind it right now.
        let l = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, p))
            .expect("the reported port is bindable");
        drop(l);
    }

    #[tokio::test]
    async fn start_reports_port_in_use_with_distinct_code() {
        // Occupy a port, then ask the bridge to bind the SAME port.
        let squatter = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let taken = squatter.local_addr().unwrap().port();
        // Unique data dir so concurrent tests don't race on the minted cert files.
        let data_dir = std::env::temp_dir().join(format!("ziee-ob-test-{taken}"));
        let _ = std::fs::create_dir_all(&data_dir);
        let err = start(taken, data_dir.clone())
            .await
            .err()
            .expect("binding an occupied port must fail");
        assert_eq!(err.error_code(), PORT_IN_USE_CODE);
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn allowed_origins_includes_loopback_and_prod() {
        let o = allowed_origins(44300);
        assert!(o.contains(&"https://localhost:44300".to_string()));
        assert!(o.contains(&"https://127.0.0.1:44300".to_string()));
        assert!(o.contains(&"https://[::1]:44300".to_string()));
        // Ephemeral port still carries the fixed production origin.
        let e = allowed_origins(1234);
        assert!(e.contains(&"https://localhost:1234".to_string()));
        assert!(e.contains(&"https://localhost:44300".to_string()));
    }
}

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
//!   value, DEC-6); on accept it echoes each text frame back. Real JSON-RPC
//!   method dispatch is ITEM-9 — this is the transport skeleton, matching the
//!   proven spike's `/bridge` echo.
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

use ziee::AppError;

use super::{assets, auth, cert};

/// The WSS subprotocol the task pane offers (alongside the token) and the
/// bridge selects on accept. Kept in sync with `resources/office-bridge/taskpane.js`.
const SUBPROTOCOL: &str = "ziee-bridge";

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
    // an ephemeral port==0 resolves to a concrete port we can reuse for v6.
    let l4 = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| bind_err(format!("bind 127.0.0.1:{port}: {e}")))?;
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

    // Echo the selected subprotocol back (Chromium/WebView2 abort the socket if
    // the client offered subprotocols and the server selects none).
    ws.protocols([SUBPROTOCOL]).on_upgrade(handle_socket)
}

/// Echo each received text/binary frame back to the sender. RPC dispatch is
/// ITEM-9; this mirrors the spike's `/bridge` echo.
async fn handle_socket(mut socket: WebSocket) {
    while let Some(Ok(msg)) = socket.recv().await {
        match msg {
            Message::Text(t) => {
                if socket.send(Message::Text(t)).await.is_err() {
                    break;
                }
            }
            Message::Binary(b) => {
                if socket.send(Message::Binary(b)).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            // Ping/Pong are handled by axum's keep-alive; ignore here.
            _ => {}
        }
    }
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

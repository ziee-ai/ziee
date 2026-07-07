//! TEST-7 — the standalone dual-stack HTTPS + WSS bridge listener (ITEM-5).
//!
//! Hermetic: the bridge binds an EPHEMERAL port on `127.0.0.1` (and best-effort
//! `[::1]`), with its cert minted into a `tempdir` — no fixed 44300, no external
//! network. A TLS client that TRUSTS the minted cert (added as a rustls root)
//! then asserts:
//!   (a) `GET /taskpane.html` returns 200 with a per-session token injected,
//!   (b) `wss://.../bridge` with that token + an allowed Origin echoes a frame,
//!   (c) a bad Origin and a missing/invalid token are BOTH rejected.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use ziee::office_bridge_bridge::{cert, server};

/// Build a rustls `ClientConfig` that trusts ONLY the minted bridge cert
/// (added as a root), using the ring provider explicitly to match `cert.rs`.
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

/// A reqwest client that trusts the minted bridge cert (added as a root, per the
/// TEST-7 spec) for the HTTPS `GET`.
fn https_client(cert_der: &[u8]) -> reqwest::Client {
    let cert = reqwest::Certificate::from_der(cert_der).expect("der → reqwest cert");
    reqwest::Client::builder()
        .add_root_certificate(cert)
        .build()
        .expect("build https client")
}

/// Extract the injected per-session token from the served `taskpane.html`.
fn extract_injected_token(html: &str) -> &str {
    let marker = "window.__ZIEE_BRIDGE_TOKEN__ = \"";
    let start = html
        .find(marker)
        .expect("token assignment present in served page")
        + marker.len();
    let end = html[start..].find('"').expect("closing quote") + start;
    &html[start..end]
}

/// Build a `wss://127.0.0.1:<port>/bridge` client request with the given Origin
/// and (optional) session token carried in the `ziee-bridge` subprotocol list.
fn ws_request(
    port: u16,
    origin: &str,
    token: Option<&str>,
) -> tokio_tungstenite::tungstenite::handshake::client::Request {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("wss://127.0.0.1:{port}/bridge")
        .into_client_request()
        .expect("client request");
    req.headers_mut().insert(
        http::header::ORIGIN,
        http::HeaderValue::from_str(origin).unwrap(),
    );
    let proto = match token {
        Some(t) => format!("ziee-bridge, {t}"),
        None => "ziee-bridge".to_string(),
    };
    req.headers_mut().insert(
        http::header::SEC_WEBSOCKET_PROTOCOL,
        http::HeaderValue::from_str(&proto).unwrap(),
    );
    req
}

/// Connect the WSS client, trusting the minted cert.
async fn ws_connect(
    req: tokio_tungstenite::tungstenite::handshake::client::Request,
    cert_der: &[u8],
) -> Result<
    tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    tokio_tungstenite::tungstenite::Error,
> {
    let connector =
        tokio_tungstenite::Connector::Rustls(Arc::new(client_config_trusting(cert_der)));
    let (ws, _resp) =
        tokio_tungstenite::connect_async_tls_with_config(req, None, false, Some(connector)).await?;
    Ok(ws)
}

#[tokio::test]
async fn test7_bridge_https_and_wss_end_to_end() {
    use tokio_tungstenite::tungstenite::Message;

    // Ephemeral, temp-dir cert → fully hermetic. Mint first so the test holds
    // the exact cert bytes the listener will load from the cache.
    let dir = tempfile::tempdir().expect("tempdir");
    let minted = cert::load_or_mint(dir.path()).expect("mint bridge cert");
    let cert_der = minted.cert_der.clone();

    let handle = server::start(0, dir.path().to_path_buf())
        .await
        .expect("bridge starts on an ephemeral port");
    let port = handle.port;
    assert_ne!(port, 0, "an ephemeral port was assigned");

    let client = https_client(&cert_der);

    // ---- (a) GET /taskpane.html over TLS → 200 with a token injected --------
    let resp = client
        .get(format!("https://127.0.0.1:{port}/taskpane.html"))
        .send()
        .await
        .expect("GET taskpane.html over TLS");
    assert_eq!(resp.status(), 200, "taskpane served 200 over TLS");
    assert!(
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .starts_with("text/html"),
        "taskpane content-type is html"
    );
    let html = resp.text().await.expect("body");
    let token = extract_injected_token(&html).to_string();
    assert_ne!(
        token, "__ZIEE_BRIDGE_TOKEN__",
        "the placeholder was replaced with a real token"
    );
    assert_eq!(token.len(), 43, "32-byte base64url session token");
    assert!(
        !html.contains("= \"__ZIEE_BRIDGE_TOKEN__\""),
        "the quoted placeholder is gone from the served page"
    );

    // ---- (b) WSS /bridge with a valid token + allowed Origin → echo ---------
    let origin = format!("https://127.0.0.1:{port}");
    let mut ws = ws_connect(ws_request(port, &origin, Some(&token)), &cert_der)
        .await
        .expect("valid token + origin upgrades");
    ws.send(Message::Text("round-trip".into()))
        .await
        .expect("send text frame");
    let echoed = tokio::time::timeout(std::time::Duration::from_secs(5), ws.next())
        .await
        .expect("echo arrives before timeout")
        .expect("stream item")
        .expect("ws message");
    assert_eq!(
        echoed.into_text().expect("text frame").as_str(),
        "round-trip",
        "the bridge echoes the frame back"
    );
    ws.close(None).await.ok();

    // ---- (c1) WSS with a BAD Origin → rejected before upgrade ---------------
    let bad_origin = ws_connect(
        ws_request(port, "https://evil.example", Some(&token)),
        &cert_der,
    )
    .await;
    assert!(
        bad_origin.is_err(),
        "a disallowed Origin must be rejected (no upgrade)"
    );

    // ---- (c2) WSS with a MISSING token → rejected --------------------------
    let no_token = ws_connect(ws_request(port, &origin, None), &cert_der).await;
    assert!(no_token.is_err(), "a missing token must be rejected");

    // ---- (c3) WSS with an INVALID token → rejected -------------------------
    let bad_token = ws_connect(
        ws_request(port, &origin, Some("not-a-real-token")),
        &cert_der,
    )
    .await;
    assert!(bad_token.is_err(), "an invalid token must be rejected");

    handle.shutdown();
}

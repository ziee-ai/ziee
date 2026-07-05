//! Transparent reverse-proxy for the built-in BioMCP server.
//!
//! `/api/bio/mcp` holds the JWT boundary (the MCP client injects a
//! short-lived JWT, validated by `RequirePermissions<(BioQuery,)>`),
//! ensures the managed sidecar is healthy, then byte-pipes the MCP
//! streamable-HTTP request through to the sidecar's `/mcp`. Only the MCP
//! protocol headers are forwarded — `Authorization` and the admin-config
//! key headers (which the client attaches from the row's `headers`) are
//! stripped; biomcp gets the keys via its process env, not over HTTP.

use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::modules::bio_mcp::permissions::BioQuery;
use crate::modules::bio_mcp::supervisor;
use crate::modules::permissions::RequirePermissions;

/// MCP streamable-HTTP request headers we forward to the sidecar. Anything
/// else (Authorization, the upstream key headers, hop-by-hop) is dropped.
const FORWARD_REQUEST_HEADERS: &[&str] = &[
    "content-type",
    "accept",
    "mcp-session-id",
    "mcp-protocol-version",
    "last-event-id",
];

static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();

/// The shared reqwest client is built lazily on the FIRST request (not at boot),
/// so a build failure must map to a 503 rather than panic the worker. reqwest's
/// build is near-infallible (only the TLS backend init can fail), and the
/// failure is deterministic, so caching the `None` is fine.
///
/// No request timeout — MCP streamable-HTTP responses may be long-lived SSE
/// streams. (See the llm_local_runtime proxy client for the same reasoning;
/// `.timeout(ZERO)` would break every request.)
fn shared_client() -> Option<&'static reqwest::Client> {
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .pool_max_idle_per_host(8)
                .no_proxy()
                .build()
                .map_err(|e| {
                    tracing::error!(error = %e, "bio_mcp: failed to build shared reqwest client");
                })
                .ok()
        })
        .as_ref()
}

pub async fn proxy_handler(
    _auth: RequirePermissions<(BioQuery,)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Ensure the (admin-enabled) sidecar is up; surface a clear error
    // otherwise (disabled / offline / failed to start).
    let base_url = match supervisor::ensure_healthy().await {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let upstream_url = format!("{}/mcp", base_url);
    let client = match shared_client() {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "biomcp proxy client unavailable" })),
            )
                .into_response();
        }
    };
    // axum's `Method` is `http::Method`, which reqwest 0.12 re-exports — pass
    // it through faithfully (no silent coercion; the router only mounts
    // POST/GET/DELETE here anyway).
    let mut req = client.request(method, &upstream_url);
    for name in FORWARD_REQUEST_HEADERS {
        if let Some(v) = headers.get(*name) {
            req = req.header(*name, v);
        }
    }
    if !body.is_empty() {
        req = req.body(body.to_vec());
    }

    match req.send().await {
        Ok(resp) => stream_back(resp).await,
        Err(e) => {
            // Don't leak the reqwest error's Display (it can include the upstream
            // host/port, TLS/DNS internals) to the client. Log it under a UUID
            // trace_id and return a generic body, mirroring
            // `AppError::internal_with_id`.
            let trace_id = uuid::Uuid::new_v4();
            tracing::error!(%trace_id, error = %e, "bio_mcp: sidecar request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "upstream biomcp request failed",
                    "trace_id": trace_id.to_string(),
                })),
            )
                .into_response()
        }
    }
}

/// Forward the sidecar response to the client, preserving SSE streaming
/// and the MCP response headers (e.g. `Mcp-Session-Id`). Copy of the
/// `llm_local_runtime::proxy_handlers::stream_back` shape.
async fn stream_back(upstream: reqwest::Response) -> Response {
    let status = upstream.status();
    let mut headers_out = HeaderMap::new();
    for (k, v) in upstream.headers().iter() {
        if k == reqwest::header::CONTENT_LENGTH || k == reqwest::header::TRANSFER_ENCODING {
            continue;
        }
        if let Ok(hv) = HeaderValue::from_bytes(v.as_bytes()) {
            if let Ok(name) = axum::http::HeaderName::from_bytes(k.as_str().as_bytes()) {
                headers_out.insert(name, hv);
            }
        }
    }

    let out_status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let body = Body::from_stream(upstream.bytes_stream());
    let mut resp = Response::new(body);
    *resp.status_mut() = out_status;
    *resp.headers_mut() = headers_out;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `stream_back` must preserve the upstream status + MCP protocol headers
    /// (e.g. `mcp-session-id`), STRIP `content-length` / `transfer-encoding`
    /// (re-derived by the streaming body), and forward the body bytes verbatim.
    /// Mocks ONLY the external boundary: a real loopback HTTP "sidecar" whose
    /// response is fetched with reqwest and handed to `stream_back`.
    #[tokio::test]
    async fn stream_back_preserves_status_headers_strips_len_and_streams_body() {
        // One-shot loopback "sidecar".
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/mcp",
                axum::routing::get(|| async {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/event-stream")
                        .header("mcp-session-id", "sess-xyz")
                        .header("content-length", "13") // must be stripped by stream_back
                        .body(Body::from("hello-sidecar"))
                        .unwrap()
                }),
            );
            let _ = axum::serve(listener, app).await;
        });

        let upstream = reqwest::Client::new()
            .get(format!("http://{addr}/mcp"))
            .send()
            .await
            .expect("fetch upstream");

        let resp = stream_back(upstream).await;

        assert_eq!(resp.status(), StatusCode::OK, "status must be preserved");
        assert_eq!(
            resp.headers().get("mcp-session-id").unwrap(),
            "sess-xyz",
            "MCP session header must be forwarded"
        );
        assert!(
            resp.headers().get(reqwest::header::CONTENT_LENGTH).is_none(),
            "content-length must be stripped"
        );
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "text/event-stream",
        );

        let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .expect("collect body");
        assert_eq!(&bytes[..], b"hello-sidecar", "body bytes must pass through");
    }
}

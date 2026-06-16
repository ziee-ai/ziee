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

static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn shared_client() -> &'static reqwest::Client {
    // No request timeout — MCP streamable-HTTP responses may be long-lived
    // SSE streams. (See the llm_local_runtime proxy client for the same
    // reasoning; `.timeout(ZERO)` would break every request.)
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .no_proxy()
            .build()
            .expect("bio_mcp shared reqwest client init")
    })
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
    // axum's `Method` is `http::Method`, which reqwest 0.12 re-exports — pass
    // it through faithfully (no silent coercion; the router only mounts
    // POST/GET/DELETE here anyway).
    let mut req = shared_client().request(method, &upstream_url);
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
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": format!("biomcp sidecar request failed: {}", e) })),
        )
            .into_response(),
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

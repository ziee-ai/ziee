// Integration + HTTP-handler tests for the web_search module.
//
// Tier 2 (settings CRUD + permission gating + secret round-trip) and Tier 3
// (JSON-RPC MCP handler: initialize / tools/list / tools/call, with a mock
// SearXNG provider + a loopback page-fetch fixture).

mod mcp_test;
mod real_llm_test;
mod settings_test;

use serde_json::{Value, json};

/// Build a JSON-RPC request to the web_search MCP endpoint.
pub fn jsonrpc(
    server: &crate::common::TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/web-search/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

/// Spawn a loopback mock SearXNG that returns canned JSON results from
/// `/search`. The SearXNG provider's policy allows private/loopback (it's an
/// admin-trusted endpoint), so this needs no debug env seam.
pub async fn start_mock_searxng() -> String {
    use axum::{
        Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/search",
        get(|q: Query<HashMap<String, String>>| async move {
            // Lock SearXNG's request contract: a regression dropping `q` or
            // sending `format != json` returns 400 (→ the search test fails)
            // instead of silently passing on canned data.
            let ok = q.get("q").map(|s| !s.is_empty()).unwrap_or(false)
                && q.get("format").map(|s| s == "json").unwrap_or(false);
            if !ok {
                return StatusCode::BAD_REQUEST.into_response();
            }
            Json(json!({
                "results": [
                    { "title": "Example Result", "url": "https://example.com/a", "content": "a snippet about the query" },
                    { "title": "Second Result", "url": "https://example.com/b", "content": "more context" }
                ]
            }))
            .into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// Spawn a loopback SearXNG that always returns HTTP 500 — used to exercise the
/// chain's error path (a configured provider that fails). No env seam needed:
/// the SearXNG provider's trusted policy allows loopback.
pub async fn start_failing_searxng() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::{Router, http::StatusCode, routing::get};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let h = hits.clone();
    let app = Router::new().route(
        "/search",
        get(move || {
            let h = h.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

/// Loopback mock SearXNG that always replies HTTP 429 (rate-limited) and counts
/// hits — to exercise the fallback-on-rate-limit path (distinct from the 500
/// case `start_failing_searxng` covers).
pub async fn start_rate_limited_searxng() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::{Router, http::StatusCode, routing::get};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let h = hits.clone();
    let app = Router::new().route(
        "/search",
        get(move || {
            let h = h.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::TOO_MANY_REQUESTS
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

/// Loopback mock SearXNG that returns a result containing `marker` and counts
/// hits — for the real-LLM test (proves a real model invoked the tool AND used
/// the result). Returns (base_url, hit_counter).
pub async fn start_marker_searxng(
    marker: &str,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::{Json, Router, routing::get};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let h = hits.clone();
    let marker = marker.to_string();
    let app = Router::new().route(
        "/search",
        get(move || {
            let h = h.clone();
            let content = format!("The unique status code is {marker}.");
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                Json(json!({
                    "results": [
                        { "title": "Ziee Status", "url": "https://example.com/status", "content": content }
                    ]
                }))
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

/// Spawn a loopback mock Brave endpoint returning canned Brave-shaped JSON.
/// Returns the FULL endpoint URL (incl. `/search`), to be set as
/// `WEB_SEARCH_BRAVE_ENDPOINT` (the debug-only Brave endpoint override).
pub async fn start_mock_brave() -> String {
    use axum::{
        Json, Router,
        extract::Query,
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/search",
        get(|headers: HeaderMap, q: Query<HashMap<String, String>>| async move {
            // Lock Brave's auth contract: 401 unless the X-Subscription-Token
            // header is present, so a regression that drops/renames it (which
            // would 401 against real Brave) fails the fallback test.
            let token = headers
                .get("X-Subscription-Token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if token.is_empty() {
                return StatusCode::UNAUTHORIZED.into_response();
            }
            // Lock the query contract too: the `q` param must be forwarded.
            if !q.get("q").map(|s| !s.is_empty()).unwrap_or(false) {
                return StatusCode::BAD_REQUEST.into_response();
            }
            Json(json!({
                "web": {
                    "results": [
                        { "title": "Brave Hit", "url": "https://brave.example/x", "description": "brave snippet" }
                    ]
                }
            }))
            .into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}/search")
}

/// Spawn a loopback HTML page for the page-fetch fixture. Page-fetch uses the
/// public-only SSRF policy, so the TestServer must be started with
/// `WEB_SEARCH_FETCH_ALLOW_LOOPBACK=1` for a 127.0.0.1 fixture to be reachable.
pub async fn start_mock_html() -> String {
    use axum::{Router, response::Html, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/page",
        get(|| async {
            Html(
                "<html><head><title>Fixture Title</title></head><body>\
                 <nav>home about contact</nav>\
                 <article><h1>Main Heading</h1>\
                 <p>This is the substantive body paragraph that readability keeps so the model can read the page.</p>\
                 <p>A second meaningful paragraph with enough words to be retained by the extractor.</p>\
                 </article><footer>copyright boilerplate</footer></body></html>",
            )
        }),
    )
    .route(
        // Redirect (302) to a private/IMDS address — the SSRF guard must block
        // the redirect HOP even though the initial loopback URL was allowed.
        "/redirect-to-imds",
        get(|| async {
            (
                axum::http::StatusCode::FOUND,
                [(axum::http::header::LOCATION, "http://169.254.169.254/latest/meta-data/")],
            )
        }),
    )
    .route(
        // Oversized page (~81 KB body) for the char-truncation + byte-cap tests.
        "/big",
        get(|| async {
            let para = "lorem ipsum dolor sit amet ".repeat(3000); // ~81 KB
            Html(format!(
                "<html><head><title>Big Page</title></head><body>\
                 <article><h1>Big</h1><p>{para}</p></article></body></html>"
            ))
        }),
    )
    // Non-HTML content types: the readability extractor is HTML-oriented, so
    // these exercise the best-effort fallback (fetch_url must still 200 and
    // surface the body text rather than choking on the wrong content type).
    .route(
        "/data.json",
        get(|| async {
            (
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                "{\"marker\":\"JSONBODYMARKER\",\"items\":[1,2,3]}",
            )
        }),
    )
    .route(
        "/data.csv",
        get(|| async {
            (
                [(axum::http::header::CONTENT_TYPE, "text/csv")],
                "col_a,col_b\nCSVBODYMARKER,2\nrow,3\n",
            )
        }),
    )
    .route(
        "/data.xml",
        get(|| async {
            (
                [(axum::http::header::CONTENT_TYPE, "application/xml")],
                "<?xml version=\"1.0\"?><root><item>XMLBODYMARKER</item></root>",
            )
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

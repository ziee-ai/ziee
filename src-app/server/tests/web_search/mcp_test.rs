//! Tier 3 — web_search JSON-RPC MCP handler: discovery, permission gating,
//! the no-provider error path, a real search via a mock SearXNG, and a real
//! page fetch via a loopback fixture (debug loopback seam).

use serde_json::json;

use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};
use crate::common::{TestServer, TestServerOptions};
use crate::web_search::{
    jsonrpc, start_failing_searxng, start_mock_brave, start_mock_html, start_mock_searxng,
    start_rate_limited_searxng,
};

fn admin_perms() -> &'static [&'static str] {
    &["web_search::admin::read", "web_search::admin::manage"]
}

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_init", &["web_search::use"]).await;
    let res = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "web_search");
}

#[tokio::test]
async fn test_tools_list_has_search_and_fetch() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_list", &["web_search::use"]).await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"web_search"));
    assert!(names.contains(&"fetch_url"));
}

#[tokio::test]
async fn test_tools_call_requires_use_permission() {
    let server = TestServer::start().await;
    // Stripped from all groups → no web_search::use.
    let user = create_user_with_no_permissions(&server, "ws_noperm").await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_default_users_group_grants_web_search_use() {
    // A user whose ONLY source of web_search::use is default-Users membership
    // (migration 098) must be able to call the tools. `create_user_with_permissions`
    // with an empty perm list registers the user (auto-joined to the default
    // Users group) and adds NO custom-group perms — so a 403 here would mean
    // migration 098's grant is missing/broken. We expect 200 with an in-band
    // "no provider configured" JSON-RPC error, NOT a 403.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_default_only", &[]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        res.status(),
        200,
        "default-Users member must pass the web_search::use gate (migration 098)"
    );
    let body: serde_json::Value = res.json().await.unwrap();
    // Permission passed → in-band no-provider error, not a transport 403.
    assert!(body["error"].is_object(), "expected in-band no-provider error: {body}");
}

#[tokio::test]
async fn test_web_search_403_after_users_group_permission_revoked() {
    // Inverse of the migration-098 grant test: if an admin strips
    // web_search::use from the default Users group, a member whose ONLY source
    // of the permission was that group must immediately be gated out (403) on
    // the next call — the gate re-resolves group perms per request.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_revoked", &[]).await;

    // Sanity: the grant is in place → passes the gate (200 in-band error).
    let before = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(before.status(), 200, "grant present → passes gate");

    // Admin action: remove web_search::use from the default Users group.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let affected = sqlx::query(
        "UPDATE groups SET permissions = array_remove(permissions, 'web_search::use') \
         WHERE is_default = TRUE",
    )
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();
    assert!(affected >= 1, "default Users group must exist to strip the perm");
    pool.close().await;

    // Next call → 403 (no other source of the permission).
    let after = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(after.status(), 403, "revoking the group perm must gate the user out");
}

#[tokio::test]
async fn test_search_with_no_provider_configured_returns_error() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_noprov", &["web_search::use"]).await;
    // Default settings: enabled, chain [searxng, brave], neither configured.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "anything" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200); // JSON-RPC carries the error in-band
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["result"].is_null());
    assert!(body["error"].is_object(), "expected a JSON-RPC error: {body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or("")
            .contains("no web search provider"),
        "expected the no-provider error specifically: {body}"
    );
}

#[tokio::test]
async fn test_search_via_mock_searxng() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_search_admin", admin_perms()).await;
    let searxng = start_mock_searxng().await;
    let client = reqwest::Client::new();

    // Configure SearXNG + make it the only chain entry.
    let r = client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "base_url": searxng } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "provider_chain": ["searxng"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // The admin is also a Users-group member → has web_search::use.
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "rust" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["provider"], "searxng", "body: {body}");
    assert!(
        sc["results"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "expected non-empty results: {body}"
    );
    assert_eq!(sc["results"][0]["url"], "https://example.com/a");
    // Lock the SearXNG `content` → SearchHit `snippet` remap + title mapping
    // through the live deserialize → map → serialize path.
    assert_eq!(sc["results"][0]["snippet"], "a snippet about the query", "body: {body}");
    assert_eq!(sc["results"][0]["title"], "Example Result", "body: {body}");

    // The model-facing TEXT channel is a readable digest, NOT stringified JSON
    // (the retrofit's whole point). Lock it so a regression to `v.to_string()`
    // fails here.
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        !text.trim_start().starts_with('{') && !text.trim_start().starts_with('['),
        "search text channel must be a readable digest, not stringified JSON: {text}"
    );
    assert!(
        text.contains("Example Result") && text.contains("example.com"),
        "search digest should name the hits: {text}"
    );

    // max_results is plumbed through end-to-end: the mock returns 2 rows, so
    // capping to 1 must yield exactly 1 result on the wire.
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "rust", "max_results": 1 } }),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["results"].as_array().unwrap().len(),
        1,
        "max_results=1 must cap results to 1: {body}"
    );
}

#[tokio::test]
async fn test_search_chain_all_providers_error_returns_in_band_error() {
    // Exercise the DB-backed resolve → build → walk path end-to-end past where
    // the run_chain unit tests (fake providers) stop: a configured provider that
    // errors (HTTP 500) and no fallback succeeds → in-band JSON-RPC error, not 500.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_allerr_admin", admin_perms()).await;
    let (failing, _hits) = start_failing_searxng().await;
    let client = reqwest::Client::new();

    let r = client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "base_url": failing } }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "provider_chain": ["searxng"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "rust" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["result"].is_null(), "expected no result: {body}");
    assert!(body["error"].is_object(), "expected in-band error: {body}");
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("searxng"),
        "error should name the failing provider: {body}"
    );
}

#[tokio::test]
async fn test_search_chain_falls_back_to_second_provider() {
    // Live, DB-backed fallback: chain [searxng, brave] where searxng errors
    // (HTTP 500) and brave serves. Uses the debug-only WEB_SEARCH_BRAVE_ENDPOINT
    // seam to point brave at a loopback mock. Asserts structuredContent.provider
    // names the engine that actually served (brave) — the module's headline feature.
    let brave_mock = start_mock_brave().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_BRAVE_ENDPOINT".to_string(), brave_mock)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ws_fallback_admin", admin_perms()).await;
    let (failing, searxng_hits) = start_failing_searxng().await;
    let client = reqwest::Client::new();

    client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "base_url": failing } }))
        .send()
        .await
        .unwrap();
    client
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "BSA-test-key" }))
        .send()
        .await
        .unwrap();
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "provider_chain": ["searxng", "brave"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["provider"], "brave", "should fall back to brave; body: {body}");
    assert_eq!(sc["results"][0]["url"], "https://brave.example/x");
    assert_eq!(sc["results"][0]["snippet"], "brave snippet");
    // Prove searxng was actually attempted (and errored) before brave served —
    // distinguishes real fallback from searxng being silently skipped.
    assert_eq!(
        searxng_hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "searxng must be attempted exactly once before falling back to brave"
    );
}

#[tokio::test]
async fn test_search_chain_falls_back_on_rate_limit_429() {
    // A rate-limited (HTTP 429) first provider must be treated like an error and
    // trigger fallback to the next provider — not a fatal stop. Chain
    // [searxng, brave]: searxng 429s, brave serves.
    let brave_mock = start_mock_brave().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_BRAVE_ENDPOINT".to_string(), brave_mock)],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ws_429_admin", admin_perms()).await;
    let (limited, searxng_hits) = start_rate_limited_searxng().await;
    let client = reqwest::Client::new();

    client
        .put(server.api_url("/web-search/providers/searxng"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "config": { "base_url": limited } }))
        .send()
        .await
        .unwrap();
    client
        .put(server.api_url("/web-search/providers/brave"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "api_key": "BSA-test-key" }))
        .send()
        .await
        .unwrap();
    let r = client
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": true, "provider_chain": ["searxng", "brave"] }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "y" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["provider"], "brave", "429 must fall back to brave; body: {body}");
    assert_eq!(
        searxng_hits.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "searxng must be attempted exactly once before the 429 fallback"
    );
}

/// fetch_url's Accept header requests HTML, but a server may still return
/// JSON / CSV / XML. The readability extractor is HTML-oriented; this asserts
/// the best-effort path still 200s and surfaces the body text (rather than
/// erroring or returning an empty result) for non-HTML content types.
#[tokio::test]
async fn test_fetch_url_handles_non_html_content_types() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_FETCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "ws_fetch_nonhtml", &["web_search::use"]).await;
    let base = start_mock_html().await;

    for (path, marker) in [
        ("/data.json", "JSONBODYMARKER"),
        ("/data.csv", "CSVBODYMARKER"),
        ("/data.xml", "XMLBODYMARKER"),
    ] {
        let res = jsonrpc(
            &server,
            &user.token,
            "tools/call",
            json!({ "name": "fetch_url", "arguments": { "url": format!("{base}{path}") } }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(res.status(), 200, "{path}: fetch_url should 200");
        let body: serde_json::Value = res.json().await.unwrap();
        assert!(
            body["error"].is_null(),
            "{path}: fetch_url must not error on non-HTML: {body}"
        );
        let content = body["result"]["structuredContent"]["content"]
            .as_str()
            .unwrap_or("");
        assert!(
            content.contains(marker),
            "{path}: non-HTML body must still surface its text ({marker}); got: {body}"
        );
    }
}

#[tokio::test]
async fn test_fetch_url_via_loopback_fixture() {
    // Page-fetch is locked to public IPs; the debug seam relaxes it to DEV_LOCAL
    // so a 127.0.0.1 fixture is reachable in tests.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_FETCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "ws_fetch", &["web_search::use"]).await;
    let html = start_mock_html().await;

    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "fetch_url", "arguments": { "url": format!("{html}/page") } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    let content = sc["content"].as_str().unwrap_or("");
    assert!(
        content.contains("substantive body"),
        "extracted markdown should keep the body; got: {body}"
    );
    assert_eq!(sc["title"], "Fixture Title");

    // The model-facing TEXT channel is the page markdown (text-as-text), NOT a
    // JSON-wrapped blob — lock the retrofit.
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("substantive body") && !text.trim_start().starts_with('{'),
        "fetch text channel must be readable markdown, not JSON: {text}"
    );
}

#[tokio::test]
async fn test_fetch_url_blocks_redirect_to_private_ip() {
    // Redirect-based SSRF: even with the loopback fixture reachable
    // (WEB_SEARCH_FETCH_ALLOW_LOOPBACK), a 302 to an IMDS/private address must
    // be blocked on the redirect HOP (the validated client re-validates every
    // hop under the same SSRF policy), so the fetch fails in-band.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_FETCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(&server, "ws_redir", &["web_search::use"]).await;
    let html = start_mock_html().await;

    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "fetch_url", "arguments": { "url": format!("{html}/redirect-to-imds") } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200, "JSON-RPC carries the error in-band");
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["result"].is_null(), "redirect to IMDS must not yield a page: {body}");
    assert!(body["error"].is_object(), "must be an in-band fetch error: {body}");
}

#[tokio::test]
async fn test_fetch_url_truncates_at_char_cap() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_FETCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ws_trunc_admin", admin_perms()).await;
    // Min char cap, large byte cap so the big body isn't rejected first.
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "fetch_max_chars": 1000, "fetch_max_bytes": 104_857_600 }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let html = start_mock_html().await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "fetch_url", "arguments": { "url": format!("{html}/big") } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["truncated"], true, "oversized page must be truncated: {body}");
    assert!(
        sc["content"].as_str().unwrap_or("").chars().count() <= 1000,
        "content must be capped at fetch_max_chars: {body}"
    );
}

#[tokio::test]
async fn test_fetch_url_rejects_oversized_body() {
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("WEB_SEARCH_FETCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ws_big_admin", admin_perms()).await;
    // Minimum byte cap; the ~81 KB /big body exceeds it.
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "fetch_max_bytes": 65536 }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let html = start_mock_html().await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "fetch_url", "arguments": { "url": format!("{html}/big") } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["result"].is_null(), "expected no result: {body}");
    assert!(body["error"].is_object(), "expected WEB_FETCH_TOO_LARGE in-band error: {body}");
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("exceeds cap"),
        "expected the too-large error specifically: {body}"
    );
}

#[tokio::test]
async fn test_search_when_disabled_returns_in_band_error() {
    // The admin kill-switch: enabled=false → web_search returns an in-band
    // WEB_SEARCH_DISABLED error (not 403, not a crash).
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_disabled_admin", admin_perms()).await;
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "web_search", "arguments": { "query": "rust" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["result"].is_null(), "{body}");
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("disabled"),
        "expected WEB_SEARCH_DISABLED: {body}"
    );
}

#[tokio::test]
async fn test_fetch_when_disabled_returns_in_band_error() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ws_fdisabled_admin", admin_perms()).await;
    let r = reqwest::Client::new()
        .put(server.api_url("/web-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    // url is non-empty so the empty-url guard passes; the enabled gate fires first.
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "fetch_url", "arguments": { "url": "https://example.com/" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("disabled"),
        "expected WEB_SEARCH_DISABLED: {body}"
    );
}

#[tokio::test]
async fn test_empty_query_and_url_are_rejected() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ws_empty_args", &["web_search::use"]).await;

    for (tool, arg_key) in [("web_search", "query"), ("fetch_url", "url")] {
        let res = jsonrpc(
            &server,
            &user.token,
            "tools/call",
            json!({ "name": tool, "arguments": { arg_key: "  " } }),
        )
        .send()
        .await
        .unwrap();
        assert_eq!(res.status(), 200);
        let body: serde_json::Value = res.json().await.unwrap();
        assert!(
            body["error"]["message"].as_str().unwrap_or("").contains("must not be empty"),
            "{tool} with blank {arg_key} should be rejected: {body}"
        );
    }
}

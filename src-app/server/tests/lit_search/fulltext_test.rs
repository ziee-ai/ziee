//! Tier 2 — fetch_paper_fulltext: open-access resolution via a mock Europe PMC
//! fullTextXML server, the content-addressed cache (a second fetch is a hit, no
//! re-request), the per-conversation `/lit` view symlink, and the
//! paywalled → not_open_access path.

use std::sync::atomic::Ordering;

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};
use crate::lit_search::{configure, jsonrpc_conv, start_mock_epmc_fulltext};

fn admin_perms() -> &'static [&'static str] {
    &["lit_search::admin::read", "lit_search::admin::manage"]
}

/// Insert a minimal conversation owned by `user_id` (enough for
/// `get_conversation_user_id` → view linking). Returns the conversation id.
async fn seed_conversation(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let conv_id = Uuid::new_v4();
    let uid = Uuid::parse_str(user_id).unwrap();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'lit fulltext', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    conv_id
}

#[tokio::test]
async fn test_fetch_fulltext_caches_and_links_view() {
    let (epmc, hits) = start_mock_epmc_fulltext().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), epmc),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_ft_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // First fetch: PMCID → Europe PMC fullTextXML → full_text, view linked.
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC123456"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["lit_dir"], "/lit");
    let paper = &sc["papers"][0];
    assert_eq!(paper["status"], "full_text", "body: {body}");
    assert_eq!(paper["source"], "europepmc");
    assert!(paper["chars"].as_u64().unwrap_or(0) > 0);
    assert!(
        paper["sandbox_path"].as_str().unwrap_or("").starts_with("/lit/"),
        "owned conversation must get a /lit view path: {paper}"
    );
    // The model-facing text carries the extracted body.
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("CRISPR base editing off-target"), "inline full text: {text}");
    assert_eq!(hits.load(Ordering::SeqCst), 1, "first fetch hits upstream once");

    // Second fetch of the same id → cache hit, NO second upstream request.
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC123456"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"]["structuredContent"]["papers"][0]["status"], "full_text");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "second fetch must be served from cache (no re-request)"
    );
}

#[tokio::test]
async fn test_verify_quote_verified_after_fetch() {
    // Fetch full text (populating the content-addressed cache), then verify_quote
    // exercises the POSITIVE (verified) + absent (not_found) deterministic paths.
    let (epmc, _hits) = start_mock_epmc_fulltext().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), epmc),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_vq_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // Populate the cache for PMC123456.
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC123456"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);

    // A verbatim span of the cached full text → verified.
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "verify_quote", "arguments": {
            "id": "PMC123456",
            "quote": "CRISPR base editing off-target effects"
        }}),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["status"], "verified", "verbatim span present in cached full text: {body}");
    assert_eq!(sc["verified"], true);

    // A span that is NOT in the paper → not_found (cached, but the quote is absent).
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "verify_quote", "arguments": {
            "id": "PMC123456",
            "quote": "a sentence that does not appear anywhere in this paper"
        }}),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["status"], "not_found",
        "an absent quote in a cached paper is not_found: {body}"
    );
}

#[tokio::test]
async fn test_fetch_doi_without_mailto_returns_not_found() {
    // PRECONDITION (deliberate + load-bearing): crossref is enabled but NO
    // `mailto` is configured (and `find_email` scans BOTH the crossref and pubmed
    // connector configs — neither has one here). Unpaywall is the only DOI→PDF
    // path and requires a contact email, so with no mailto that path is SKIPPED
    // entirely (no live network) and the resolver reports `not_found` — i.e. "OA
    // status not determined" (re-resolvable once a mailto is set) — rather than
    // mislabeling the paper as definitively paywalled (`not_open_access`). If a
    // future change made `configure` set a default mailto, this test would start
    // hitting live Unpaywall — keep mailto unset here.
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string())],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_paywall_admin", admin_perms()).await;
    configure(&server, &admin.token, &["crossref"]).await; // intentionally no mailto
    let conv = seed_conversation(&server, &admin.user_id).await;

    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["10.9999/paywalled.xyz"] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let paper =
        res.json::<serde_json::Value>().await.unwrap()["result"]["structuredContent"]["papers"][0].clone();
    assert_eq!(paper["status"], "not_found");
    // A non-OA paper MUST still carry an (empty) `text` key — a workflow
    // `{{ paper.text }}` reference would raise MissingField otherwise, failing the
    // whole extract llm_map (which `on_error: skip` does NOT catch).
    assert_eq!(paper["text"], "", "every paper carries a text key (empty for non-OA): {paper}");
}

#[tokio::test]
async fn test_fetch_empty_ids_is_rejected() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_ft_empty_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["  "] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("must not be empty"),
        "blank ids should be rejected: {body}"
    );
}

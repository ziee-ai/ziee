//! Tier 2 — fetch_paper_fulltext: open-access resolution via a mock Europe PMC
//! fullTextXML server, the content-addressed cache (a second fetch is a hit, no
//! re-request), the per-conversation `/lit` view symlink, and the
//! paywalled → not_open_access path.

use std::sync::atomic::Ordering;

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};
use crate::lit_search::{
    configure, jsonrpc, jsonrpc_conv, start_mock_epmc_fulltext, start_mock_europepmc,
};

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

/// End-to-end multi-step researcher flow (gap 3bed): a SINGLE server instance
/// drives `literature_search` (mock Europe PMC /search) and THEN
/// `fetch_paper_fulltext` (mock Europe PMC fullTextXML), exercising the
/// search → open-access-fulltext chain that prior tests only covered in
/// isolation. (Quote-verification + screening are frontend right-panel state
/// per CLAUDE.md — no server tables — so the server-side chain ends at fetch.)
#[tokio::test]
async fn test_search_then_fetch_fulltext_end_to_end() {
    let search = start_mock_europepmc().await;
    let (fulltext, hits) = start_mock_epmc_fulltext().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{search}/search")),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), fulltext),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_e2e_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // Step 1 — discover.
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "crispr" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let records = body["result"]["structuredContent"]["records"]
        .as_array()
        .expect("search records");
    assert!(!records.is_empty(), "search step must return records: {body}");

    // Step 2 — fetch open-access full text for a resolved id, into the /lit view.
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
    let paper = &body["result"]["structuredContent"]["papers"][0];
    assert_eq!(paper["status"], "full_text", "fetch step body: {body}");
    assert!(
        paper["sandbox_path"].as_str().unwrap_or("").starts_with("/lit/"),
        "fetched paper must be mounted in the conversation /lit view: {paper}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1, "fulltext fetched from upstream once");
}

// audit id all-f952f5cbad73 — the multi-step lit workflow (search → fulltext →
// verify_quote) was only tested as isolated single steps. This drives all three
// tools in ONE conversation against loopback mocks: discover records, fetch a
// paper's open-access full text (cached + /lit-linked), then verify a verbatim
// quote against that cached full text. (Screening is the UI tail, covered by the
// 19-literature E2E.)
#[tokio::test]
async fn multi_step_search_then_fulltext_then_verify_quote() {
    let search = start_mock_europepmc().await;
    let (epmc_ft, _hits) = start_mock_epmc_fulltext().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{search}/search")),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), epmc_ft),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_multistep", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // Step 1 — search.
    let search_body: serde_json::Value = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "study" } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(search_body["error"].is_null(), "search: {search_body}");
    assert!(
        !search_body["result"]["structuredContent"]["records"].as_array().cloned().unwrap_or_default().is_empty(),
        "search must return records: {search_body}"
    );

    // Step 2 — fetch full text (populates the cache + /lit view).
    let ft: serde_json::Value = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": ["PMC123456"] } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(ft["result"]["structuredContent"]["papers"][0]["status"], "full_text", "fetch: {ft}");

    // Step 3 — verify a verbatim quote against the cached full text.
    let vq: serde_json::Value = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "verify_quote", "arguments": { "id": "PMC123456", "quote": "CRISPR base editing off-target effects" } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(vq["result"]["structuredContent"]["status"], "verified", "verify_quote: {vq}");
}

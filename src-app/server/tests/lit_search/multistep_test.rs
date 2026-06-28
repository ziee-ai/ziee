//! Tier 2 — the multi-step systematic-review user workflow chained END-TO-END
//! on the real handlers in one conversation/session:
//!
//!   literature_search  →  fetch_paper_fulltext  →  verify_quote
//!
//! The prior tests exercise each step in isolation with hard-coded ids
//! (mcp_test.rs searches only; fulltext_test.rs fetches/verifies a literal
//! `PMC123456`). This test instead takes the identifier the SEARCH step
//! actually returned (a PMID), feeds it into the fetch step (which resolves
//! PMID → PMCID → Europe PMC `fullTextXML` against the loopback mock), and then
//! grounds a quote against the fetched-and-cached text — the exact path a model
//! walks during screening. Only the upstream HTTP APIs are mocked; every
//! lit_search handler runs for real.

use serde_json::json;
use uuid::Uuid;

use crate::common::test_helpers::create_user_with_permissions;
use crate::common::{TestServer, TestServerOptions};
use crate::lit_search::{configure, jsonrpc_conv, start_mock_europepmc};

fn admin_perms() -> &'static [&'static str] {
    &["lit_search::admin::read", "lit_search::admin::manage"]
}

async fn seed_conversation(server: &TestServer, user_id: &str) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let conv_id = Uuid::new_v4();
    let uid = Uuid::parse_str(user_id).unwrap();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'lit multistep', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    conv_id
}

/// Mock for the Europe PMC `LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT` base, which
/// the resolver hits for BOTH steps a bare PMID needs:
///   * `GET /search` (PMID → PMCID conversion: `ext_id:<pmid> AND SRC:MED`)
///   * `GET /{source}/{id}/fullTextXML` (the JATS full text under `PMC`)
/// so feeding the PMID `111` (which the search mock returns) yields full text.
async fn start_mock_epmc_fulltext_base() -> String {
    use axum::{
        Json, Router, extract::Query, response::Html, routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new()
        .route(
            "/search",
            get(|q: Query<HashMap<String, String>>| async move {
                // Lock the PMID→PMCID request contract: the resolver must ask for
                // the specific ext_id under SRC:MED in json/lite form.
                let query = q.get("query").cloned().unwrap_or_default();
                assert!(
                    query.contains("ext_id:111") && query.contains("SRC:MED"),
                    "unexpected pmid→pmcid query: {query}"
                );
                Json(json!({
                    "resultList": { "result": [ { "pmcid": "PMC123456" } ] }
                }))
            }),
        )
        .route(
            "/{source}/{id}/fullTextXML",
            get(|| async {
                Html(
                    "<article><front><article-title>Full Paper</article-title></front>\
                     <body><sec><title>Methods</title>\
                     <p>The unique full-text sentence about CRISPR base editing off-target effects.</p>\
                     </sec></body></article>",
                )
            }),
        );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn search_then_fetch_fulltext_then_verify_quote_end_to_end() {
    let search_base = start_mock_europepmc().await;
    let fulltext_base = start_mock_epmc_fulltext_base().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{search_base}/search")),
            ("LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT".to_string(), fulltext_base),
        ],
        ..Default::default()
    })
    .await;
    let admin = create_user_with_permissions(&server, "ls_multistep_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let conv = seed_conversation(&server, &admin.user_id).await;

    // ── Step 1: literature_search → records with real identifiers ──────────
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
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
        .expect("search returns a records array");
    assert!(!records.is_empty(), "search must return records: {body}");

    // Pull the PMID the search ACTUALLY returned — the chain's linkage point.
    let pmid = records
        .iter()
        .find_map(|r| r["pmid"].as_str())
        .expect("at least one returned record carries a pmid")
        .to_string();
    assert_eq!(pmid, "111", "europepmc mock record's pmid drives the fetch step");

    // ── Step 2: fetch_paper_fulltext using the id FROM the search result ────
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "fetch_paper_fulltext", "arguments": { "ids": [pmid] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let paper = &body["result"]["structuredContent"]["papers"][0];
    assert_eq!(
        paper["status"], "full_text",
        "the searched PMID must resolve (PMID→PMCID→fullTextXML) to full text: {body}"
    );
    assert_eq!(paper["source"], "europepmc");

    // ── Step 3: verify_quote grounds a claim against the fetched/cached text ─
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "verify_quote", "arguments": {
            "id": "111",
            "quote": "CRISPR base editing off-target effects"
        }}),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(
        sc["status"], "verified",
        "a verbatim span of the chained full text must verify: {body}"
    );
    assert_eq!(sc["verified"], true);

    // Negative control: an absent quote against the SAME cached paper is
    // not_found (proves verify is reading the real cached text, not rubber-stamping).
    let res = jsonrpc_conv(
        &server,
        &admin.token,
        &conv.to_string(),
        "tools/call",
        json!({ "name": "verify_quote", "arguments": {
            "id": "111",
            "quote": "a sentence that never appears in the chained paper"
        }}),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(
        body["result"]["structuredContent"]["status"], "not_found",
        "an absent quote in the cached paper is not_found: {body}"
    );
}

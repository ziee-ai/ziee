//! Cross-module integration — the lit_search → citations handoff (audit
//! `all-917bbcbb50f7`).
//!
//! The audit names a "lit_search → workflow → citations" chain, but there is
//! NO code path wiring `workflow_mcp` to either lit_search or citations (a grep
//! of `src/modules/workflow_mcp` for `lit_search`/`citations` is empty). The
//! REAL, in-code cross-module data path — documented at
//! `citations/models.rs:97` ("A full CSL-JSON item … piped from a prior
//! literature_search result"), `citations/tools.rs:19`, and `citations/rest.rs`
//! ("the lit_search handoff") — is **lit_search → citations**: a
//! `literature_search` record carries a DOI, and that DOI is fed straight into
//! the citations `add_citations` MCP tool, which resolves + verifies it into a
//! persisted bibliography entry.
//!
//! This test exercises that real handoff end-to-end on the production handlers,
//! across BOTH built-in MCP servers, in one server process: mock Europe PMC
//! returns a CRISPR record whose DOI (`10.5555/known`) the mock doi.org resolver
//! recognises, so the chain `lit_search search → DOI → citations add → verified`
//! holds without fabricating any link. Only the upstream HTTP boundaries
//! (Europe PMC + doi.org) are mocked; every ziee handler runs for real.

use serde_json::{Value, json};

use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;

/// Mock Europe PMC `/search` returning a single CRISPR record whose DOI is the
/// one the citations doi.org mock (`crate::citations::start_mock_doi_resolver`)
/// resolves to verified CSL — so the lit_search output is a valid citations
/// input. Mirrors the request contract of `lit_search::start_mock_europepmc`
/// (requires `query` + `format=json`).
async fn start_mock_europepmc_crispr() -> String {
    use axum::{
        Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/search",
        get(|q: Query<HashMap<String, String>>| async move {
            let ok = q.get("query").map(|s| !s.is_empty()).unwrap_or(false)
                && q.get("format").map(|s| s == "json").unwrap_or(false);
            if !ok {
                return StatusCode::BAD_REQUEST.into_response();
            }
            Json(json!({
                "resultList": { "result": [
                    { "id": "MED9", "source": "MED", "pmid": "33495596",
                      "doi": "10.5555/known",
                      "title": "CRISPR interference in plant gene regulation",
                      "authorString": "Smith J", "journalTitle": "Nature",
                      "pubYear": "2021",
                      "abstractText": "europepmc abstract for the crispr study",
                      "citedByCount": 12 }
                ]}
            }))
            .into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// literature_search (lit_search MCP) → take the returned DOI → add_citations
/// (citations MCP) → the entry is resolved, verified, and persisted.
#[tokio::test]
async fn test_lit_search_doi_handoff_to_citations_is_verified_and_stored() {
    // doi.org mock (citations side) — recognises `10.5555/known`.
    let doi_resolver = crate::citations::start_mock_doi_resolver().await;
    // Europe PMC mock (lit_search side) — emits a record with that same DOI.
    let europepmc = start_mock_europepmc_crispr().await;

    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            // lit_search loopback seam.
            ("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string()),
            (
                "LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(),
                format!("{europepmc}/search"),
            ),
            // citations loopback seam.
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi_resolver),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await;

    // Admin enables lit_search + restricts to the one mocked connector.
    let admin = create_user_with_permissions(
        &server,
        "lwc_admin",
        &["lit_search::admin::read", "lit_search::admin::manage"],
    )
    .await;
    crate::lit_search::configure(&server, &admin.token, &["europepmc"]).await;

    // The end user holds both built-in tool permissions (also granted via the
    // default group, but explicit here).
    let user = create_user_with_permissions(
        &server,
        "lwc_user",
        &["lit_search::use", "citations::use"],
    )
    .await;

    // ── Step 1: literature_search (lit_search MCP) ────────────────────────
    let res = crate::lit_search::jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "crispr" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200, "literature_search call");
    let body: Value = res.json().await.unwrap();
    let records = body["result"]["structuredContent"]["records"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(records.len(), 1, "expected the one mocked record: {body}");
    let doi = records[0]["doi"]
        .as_str()
        .expect("the search record must carry a DOI")
        .to_string();
    assert_eq!(doi, "10.5555/known", "handoff DOI: {body}");

    // ── Step 2: feed that DOI into citations add_citations (citations MCP) ─
    let res = crate::citations::jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "add_citations", "arguments": { "items": [ { "id": doi } ] } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200, "add_citations call");
    let body: Value = res.json().await.unwrap();
    let result = &body["result"]["structuredContent"]["results"][0];
    // The DOI from lit_search resolved against doi.org → verified, freshly inserted.
    assert_eq!(result["verification_status"], "verified", "{body}");
    assert_eq!(result["dedup_outcome"], "inserted", "{body}");

    // ── Step 3: the resolved CSL is persisted in the library ──────────────
    let res = crate::citations::jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "list_citations", "arguments": {} }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200, "list_citations call");
    let body: Value = res.json().await.unwrap();
    let entries = body["result"]["structuredContent"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(entries.len(), 1, "one persisted entry: {body}");
    assert_eq!(
        entries[0]["title"], "CRISPR interference in plant gene regulation",
        "stored title reflects the resolved CSL, not the raw DOI: {body}"
    );
    assert_eq!(entries[0]["verification_status"], "verified", "{body}");
}

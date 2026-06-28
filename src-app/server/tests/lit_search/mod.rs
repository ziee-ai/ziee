// Integration + HTTP-handler tests for the lit_search module.
//
// Tier 2 — admin settings/connectors CRUD + permission gating + secret
// round-trip + sync (settings_test.rs); the JSON-RPC MCP handler
// (initialize / tools/list / tools/call) driving the UNION search over mock
// loopback upstreams + dedup + identified counts + completeness (mcp_test.rs);
// fetch_paper_fulltext over a mock Europe PMC fullTextXML server + the cache +
// per-conversation view symlink (fulltext_test.rs).
// Tier 3 — the per-conversation `/lit` read-only sandbox mount
// (sandbox_mount_test.rs, rootfs-gated env-skip).
// Tier 4 — real-network + real-LLM smoke (real_llm_test.rs, key-gated).
//
// The connectors hit fixed public hosts; the debug-only `LIT_SEARCH_<X>_ENDPOINT`
// seams (compiled out of release) point them at the loopback mocks below, paired
// with `LIT_SEARCH_ALLOW_LOOPBACK=1` so the SSRF policy permits 127.0.0.1.

mod citations_handoff_test;
mod fulltext_test;
mod mcp_test;
mod multistep_test;
mod real_llm_test;
mod sandbox_mount_test;
mod settings_test;

use serde_json::{Value, json};

use crate::common::TestServer;

/// Build a JSON-RPC request to the lit_search MCP endpoint.
pub fn jsonrpc(
    server: &TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/lit-search/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

/// Same, scoped to a conversation (the `x-conversation-id` header
/// fetch_paper_fulltext needs to link blobs into a per-conversation view).
pub fn jsonrpc_conv(
    server: &TestServer,
    token: &str,
    conversation_id: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    jsonrpc(server, token, method, params).header("x-conversation-id", conversation_id)
}

/// Enable literature search and restrict the active connectors. Restricting is
/// load-bearing in tests: the migration default enables 5 sources, and an
/// un-restricted run would egress to the real Europe PMC/Crossref/etc. hosts.
pub async fn configure(server: &TestServer, admin_token: &str, connectors: &[&str]) {
    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "enabled": true,
            "enabled_connectors": connectors,
            "completeness_estimate_enabled": true,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "configure settings failed");
}

// ---- Mock loopback upstreams ---------------------------------------------

/// Mock Europe PMC `/search` (Solr `resultType=core` JSON). Returns one record
/// that overlaps Crossref by DOI (`10.1/shared`) + one Europe-PMC-only record.
/// Set `LIT_SEARCH_EUROPEPMC_ENDPOINT=<base>/search`.
pub async fn start_mock_europepmc() -> String {
    use axum::{Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get};
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/search",
        get(|q: Query<HashMap<String, String>>| async move {
            // Lock the request contract: a regression dropping `query`/`format`
            // fails the search test rather than silently passing on canned data.
            let ok = q.get("query").map(|s| !s.is_empty()).unwrap_or(false)
                && q.get("format").map(|s| s == "json").unwrap_or(false);
            if !ok {
                return StatusCode::BAD_REQUEST.into_response();
            }
            Json(json!({
                "resultList": { "result": [
                    { "id": "MED1", "source": "MED", "pmid": "111", "doi": "10.1/shared",
                      "title": "Shared Study", "authorString": "Smith J, Doe A",
                      "journalTitle": "Nature", "pubYear": "2021",
                      "abstractText": "epmc abstract for the shared study", "citedByCount": 10 },
                    { "id": "MED2", "source": "MED", "pmid": "222", "doi": "10.1/epmc-only",
                      "title": "Europe PMC Only", "authorString": "Roe B", "pubYear": "2020",
                      "abstractText": "an abstract present only in europepmc" }
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

/// Mock Crossref `/works`. Returns the same `10.1/shared` DOI (with a LONGER
/// abstract, to exercise the merge-keeps-longest rule) + one Crossref-only DOI.
/// Set `LIT_SEARCH_CROSSREF_ENDPOINT=<base>/works`.
pub async fn start_mock_crossref() -> String {
    use axum::{Json, Router, extract::Query, http::StatusCode, response::IntoResponse, routing::get};
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/works",
        get(|q: Query<HashMap<String, String>>| async move {
            if !q.get("query").map(|s| !s.is_empty()).unwrap_or(false) {
                return StatusCode::BAD_REQUEST.into_response();
            }
            Json(json!({
                "message": { "items": [
                    { "DOI": "10.1/shared", "title": ["Shared Study"],
                      "author": [{ "given": "J", "family": "Smith" }],
                      "container-title": ["Nature"], "issued": { "date-parts": [[2021]] },
                      "abstract": "<jats:p>crossref carries a noticeably longer abstract for the shared study than europepmc does</jats:p>",
                      "is-referenced-by-count": 8, "type": "journal-article" },
                    { "DOI": "10.1/crossref-only", "title": ["Crossref Only"],
                      "issued": { "date-parts": [[2019]] }, "type": "journal-article" }
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

/// Mock Semantic Scholar that returns HTTP 429 on the FIRST hit (with
/// `Retry-After: 0`) and a 200 payload on the second — to exercise the
/// connector's single 429-retry. Returns (base_url, hit_counter).
/// Set `LIT_SEARCH_S2_ENDPOINT=<base>/search`.
pub async fn start_mock_s2_flaky()
-> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::{
        Json, Router, http::StatusCode, http::header, response::IntoResponse, routing::get,
    };
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
                let n = h.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    return ([(header::RETRY_AFTER, "0")], StatusCode::TOO_MANY_REQUESTS)
                        .into_response();
                }
                Json(json!({
                    "data": [{
                        "paperId": "s2-1", "title": "S2 Paper", "abstract": "from semantic scholar",
                        "year": 2022, "venue": "JMLR",
                        "externalIds": { "DOI": "10.1/s2only", "PubMed": "777" },
                        "citationCount": 3, "authors": [{ "name": "Jane Smith" }]
                    }]
                }))
                .into_response()
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

/// Mock Semantic Scholar paper-graph (references/citations) endpoint for the
/// `fetch_references` snowball tool. A wildcard route serves ANY
/// `/{s2id}/references|citations` path (so an embedded DOI slash in the s2id
/// doesn't break routing) with one cited + one citing paper, so the same mock
/// drives both directions. Returns the base url.
/// Set `LIT_SEARCH_S2_PAPER_ENDPOINT=<base>`.
pub async fn start_mock_s2_paper() -> String {
    use axum::{Json, Router, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/{*rest}",
        get(|| async {
            Json(json!({
                "data": [{
                    "citedPaper": {
                        "paperId": "ref-1", "title": "A Cited Reference",
                        "abstract": "the referenced work", "year": 2019, "venue": "Nature",
                        "externalIds": { "DOI": "10.9/cited", "PubMed": "555" },
                        "citationCount": 12, "authors": [{ "name": "Ref Author" }]
                    },
                    "citingPaper": {
                        "paperId": "cit-1", "title": "A Citing Paper",
                        "abstract": "the citing work", "year": 2023, "venue": "Cell",
                        "externalIds": { "DOI": "10.9/citing", "PubMed": "556" },
                        "citationCount": 1, "authors": [{ "name": "Cite Author" }]
                    }
                }]
            }))
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// Mock Europe PMC fullTextXML server. Serves `/{source}/{id}/fullTextXML`
/// (source ∈ MED/PMC) with a JATS body whose text is recognizable post-strip,
/// and counts hits (so a cache-hit test can prove the second fetch did NOT
/// re-request upstream). Returns (base_url, hit_counter).
/// Set `LIT_SEARCH_EUROPEPMC_FULLTEXT_ENDPOINT=<base>`.
pub async fn start_mock_epmc_fulltext()
-> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use axum::{Router, response::Html, routing::get};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let h = hits.clone();
    let app = Router::new().route(
        "/{source}/{id}/fullTextXML",
        get(move || {
            let h = h.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                Html(
                    "<article><front><article-title>Full Paper</article-title></front>\
                     <body><sec><title>Methods</title>\
                     <p>The unique full-text sentence about CRISPR base editing off-target effects.</p>\
                     </sec></body></article>",
                )
            }
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    (format!("http://127.0.0.1:{port}"), hits)
}

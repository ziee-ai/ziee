//! audit id all-533727562cfe — cross-subsystem flow: a record discovered by
//! lit_search feeds the citations subsystem (its DOI is added + verified). Both
//! real handlers run in one TestServer over loopback mocks; the DOI lit_search
//! returns is the exact input citations resolves. (The intermediate
//! get_tool_result recall is the chat transport, covered separately by
//! agentic_chat::model_recalls_prior_result_via_get_tool_result.)

use serde_json::{Value, json};

use crate::common::{TestServer, TestServerOptions};
use crate::common::test_helpers::create_user_with_permissions;
use super::{configure, jsonrpc};

/// europepmc mock returning ONE record whose DOI (10.5555/known) the citations
/// doi.org mock below can resolve to a real CSL record.
async fn mock_europepmc_known_doi() -> String {
    use axum::{Json, Router, extract::Query, routing::get};
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/search",
        get(|_q: Query<HashMap<String, String>>| async move {
            Json(json!({
                "resultList": { "result": [
                    { "id": "MED9", "source": "MED", "pmid": "999", "doi": "10.5555/known",
                      "title": "A Known Paper", "authorString": "Smith J", "pubYear": "2021",
                      "abstractText": "abstract", "citedByCount": 5 }
                ]}
            }))
        }),
    );
    tokio::spawn(async move { let _ = axum::serve(listener, app.into_make_service()).await; });
    format!("http://127.0.0.1:{port}")
}

/// doi.org resolver mock: 10.5555/known → canned CSL-JSON; everything else 404.
async fn mock_doi_resolver() -> String {
    use axum::{Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::get};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/{*doi}",
        get(|Path(doi): Path<String>| async move {
            if doi.to_lowercase() == "10.5555/known" {
                Json(json!({
                    "type": "article-journal",
                    "title": "A Known Paper",
                    "author": [{ "family": "Smith", "given": "J." }],
                    "issued": { "date-parts": [[2021]] },
                    "DOI": "10.5555/known"
                }))
                .into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }),
    );
    tokio::spawn(async move { let _ = axum::serve(listener, app.into_make_service()).await; });
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn lit_search_record_doi_feeds_citations_and_is_verified() {
    let epmc = mock_europepmc_known_doi().await;
    let doi = mock_doi_resolver().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".into(), "1".into()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".into(), format!("{epmc}/search")),
            ("CITATIONS_ALLOW_LOOPBACK".into(), "1".into()),
            ("CITATIONS_RESOLVER_ENDPOINT".into(), doi),
        ],
        ..Default::default()
    })
    .await;

    let user = create_user_with_permissions(
        &server,
        "xsub_user",
        &[
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
            "citations::use",
            "citations::manage",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc"]).await;

    // 1) lit_search discovers the record.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "known" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    let records = body["result"]["structuredContent"]["records"].as_array().expect("records");
    let found_doi = records
        .iter()
        .find_map(|r| r["doi"].as_str())
        .expect("a record with a DOI from lit_search")
        .to_string();
    assert_eq!(found_doi, "10.5555/known", "lit_search must surface the seeded DOI: {body}");

    // 2) Feed that DOI into citations — the real resolver verifies + stores it.
    let cit = reqwest::Client::new()
        .post(server.api_url("/citations/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "add_citations", "arguments": { "items": [{ "id": found_doi }] } }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(cit.status(), 200);
    let cbody: Value = cit.json().await.unwrap();
    let result0 = &cbody["result"]["structuredContent"]["results"][0];
    assert_eq!(
        result0["verification_status"], "verified",
        "the lit_search DOI must verify through citations: {cbody}"
    );

    // 3) It is persisted in the library with that DOI (cross-subsystem handoff).
    let list = reqwest::Client::new()
        .get(server.api_url("/citations"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap();
    let lbody: Value = list.json().await.unwrap();
    assert!(
        lbody["entries"].as_array().unwrap().iter().any(|e| e["doi"].as_str() == Some("10.5555/known")),
        "the lit_search-discovered paper must land in the citations library: {lbody}"
    );
}

// audit id all-1d2d31c53866 — MCP + memory + lit_search combined in ONE
// conversation. The mcp chat-collector (order 30) runs after memory (27) and
// lit_search (28); nothing exercised memory and lit_search built-ins together.
// Here one user/conversation drives memory remember→recall AND a lit_search
// literature_search (loopback europepmc) — both built-in MCP subsystems operate
// under the same conversation scope without interfering.
#[tokio::test]
async fn memory_and_lit_search_compose_in_one_conversation() {
    if crate::common::memory_setup::skip_if_no_embedding_key(
        "memory_and_lit_search_compose_in_one_conversation",
    ) {
        return;
    }
    let epmc = mock_europepmc_known_doi().await;
    let server = TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".into(), "1".into()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".into(), format!("{epmc}/search")),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(
        &server,
        "mem_lit_user",
        &[
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
            "memory::read",
            "memory::write",
            "memory::admin::read",
            "memory::admin::manage",
            "llm_providers::read",
            "llm_providers::edit",
            "llm_models::create",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc"]).await;

    // Memory now defaults OFF: enable it deployment-wide + configure the
    // embedding model (against the local bridge) before remember/recall.
    crate::common::memory_setup::enable_semantic_memory(&server, &user.token).await;

    let conv = uuid::Uuid::new_v4().to_string();

    // (1) memory subsystem: remember a research interest, then recall it.
    let remember = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", &conv)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "remember", "arguments": { "content": "User researches CRISPR base editing", "kind": "fact" } } }))
        .send()
        .await
        .unwrap();
    assert_eq!(remember.status(), 200);
    assert!(remember.json::<Value>().await.unwrap()["error"].is_null(), "remember should succeed");

    let recall: Value = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", &conv)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "recall", "arguments": { "query": "what does the user research", "top_k": 5 } } }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mems = recall["result"]["structuredContent"]["memories"].as_array().cloned().unwrap_or_default();
    assert!(
        mems.iter().filter_map(|m| m["content"].as_str()).any(|c| c.contains("CRISPR base editing")),
        "the remembered fact must be recallable in the same conversation: {recall}"
    );

    // (2) lit_search subsystem: a literature search in the SAME conversation.
    let search: Value = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "known" } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(search["error"].is_null(), "literature_search should succeed: {search}");
    let records = search["result"]["structuredContent"]["records"].as_array().cloned().unwrap_or_default();
    assert!(!records.is_empty(), "lit_search must return the seeded record alongside memory: {search}");
}

// audit id all-e514fadefce4 — concurrent multi-tool flow across THREE built-in
// MCP subsystems (bio_mcp + lit_search + citations) in one conversation. Prior
// tests exercise each in isolation; nothing fired them CONCURRENTLY to prove
// they don't interfere. We tokio::join! a lit_search, a citations add, and a
// bio_mcp proxy call: lit_search + citations must each return their own correct
// result; bio_mcp must return a well-formed response (200 when a sidecar is
// available, or a graceful 503 on a stub build) — never a 500 / hang / cross-
// contamination of the others.
#[tokio::test]
async fn concurrent_bio_lit_citations_do_not_interfere() {
    let epmc = mock_europepmc_known_doi().await;
    let server = TestServer::start_with_options(TestServerOptions {
        bio_mcp_enabled: true,
        extra_env: vec![
            ("LIT_SEARCH_ALLOW_LOOPBACK".into(), "1".into()),
            ("LIT_SEARCH_EUROPEPMC_ENDPOINT".into(), format!("{epmc}/search")),
        ],
        ..Default::default()
    })
    .await;
    let user = create_user_with_permissions(
        &server,
        "concurrent_xsub",
        &[
            "lit_search::use",
            "lit_search::admin::read",
            "lit_search::admin::manage",
            "citations::use",
            "citations::manage",
            "bio::query",
        ],
    )
    .await;
    configure(&server, &user.token, &["europepmc"]).await;

    let token = user.token.clone();
    let lit = jsonrpc(
        &server,
        &token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "known" } }),
    )
    .send();
    let cit = reqwest::Client::new()
        .post(server.api_url("/citations/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "add_citations", "arguments": { "items": [
                { "csl": { "type": "article-journal", "title": "Concurrent XSub Paper", "issued": { "date-parts": [[2023]] } } }
            ] } } }))
        .send();
    let bio = reqwest::Client::new()
        .post(server.api_url("/bio/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }))
        .send();

    // Fire all three concurrently.
    let (lit_res, cit_res, bio_res) = tokio::join!(lit, cit, bio);

    // lit_search returned its own record.
    let lit_body: Value = lit_res.unwrap().json().await.unwrap();
    assert!(lit_body["error"].is_null(), "concurrent lit_search: {lit_body}");
    assert!(
        !lit_body["result"]["structuredContent"]["records"].as_array().cloned().unwrap_or_default().is_empty(),
        "lit_search must return its record under concurrency: {lit_body}"
    );

    // citations stored its own entry.
    let cit_status = cit_res.unwrap();
    assert_eq!(cit_status.status(), 200, "concurrent citations call must 200");
    assert!(cit_status.json::<Value>().await.unwrap()["error"].is_null(), "citations add must succeed");

    // bio_mcp returned a well-formed response — 200 (sidecar) or graceful 503
    // (stub build); never a 500 / hang / contamination.
    let bio_status = bio_res.unwrap().status();
    assert!(
        bio_status == 200 || bio_status == 503,
        "bio_mcp under concurrency must be 200 or a graceful 503, got {bio_status}"
    );
}

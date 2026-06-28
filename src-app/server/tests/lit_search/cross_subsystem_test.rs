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

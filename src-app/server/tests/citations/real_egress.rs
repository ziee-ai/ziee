//! Real-network resolve/verify smoke against the live public APIs (doi.org
//! content negotiation + NCBI ID-Converter + arXiv DataCite). Gated on
//! `ZIEE_CITATIONS_REAL_NETWORK` (off in CI; catches upstream API drift) — a
//! SOFT-SKIP, not `#[ignore]`, so a sourced suite that sets the var runs it.
//! Mirrors `lit_search/real_llm_test.rs`'s `ZIEE_LIT_REAL_NETWORK` convention.

use serde_json::{Value, json};

use crate::citations::jsonrpc;
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn skip() -> bool {
    if std::env::var("ZIEE_CITATIONS_REAL_NETWORK").is_err() {
        eprintln!("skipping citations::real_egress — ZIEE_CITATIONS_REAL_NETWORK unset");
        true
    } else {
        false
    }
}

async fn verify_one(server: &TestServer, token: &str, item: Value) -> Value {
    let res = jsonrpc(server, token, "tools/call",
        json!({ "name": "verify_citations", "arguments": { "items": [item] } }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    body["result"]["structuredContent"]["results"][0].clone()
}

#[tokio::test]
async fn real_doi_verifies() {
    if skip() { return; }
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_re_doi", &[]).await;
    let r = verify_one(&server, &user.token, json!({ "id": "10.1038/s41586-020-2649-2" })).await;
    assert_eq!(r["verification_status"], "verified", "{r}");
}

#[tokio::test]
async fn real_pmid_verifies() {
    if skip() { return; }
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_re_pmid", &[]).await;
    let r = verify_one(&server, &user.token, json!({ "id": "33495596", "kind": "pmid" })).await;
    assert_eq!(r["verification_status"], "verified", "{r}");
}

#[tokio::test]
async fn fabricated_doi_is_not_found() {
    if skip() { return; }
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_re_fake", &[]).await;
    let r = verify_one(&server, &user.token, json!({ "id": "10.9999/this-doi-does-not-exist-zzzz" })).await;
    assert_eq!(r["verification_status"], "not_found", "{r}");
}

#[tokio::test]
async fn real_arxiv_verifies() {
    if skip() { return; }
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_re_arxiv", &[]).await;
    // "Attention Is All You Need".
    let r = verify_one(&server, &user.token, json!({ "id": "1706.03762", "kind": "arxiv" })).await;
    assert_eq!(r["verification_status"], "verified", "{r}");
}

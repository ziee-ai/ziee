//! Integration + HTTP-handler tests for the citations module.
//!
//! Tier 2 (REST library CRUD + permission gating + project links), Tier 3
//! (JSON-RPC MCP handler), and the deterministic mock-resolve tier: resolve +
//! verify + dedup against loopback mocks for all three upstreams (doi.org
//! content-negotiation, NCBI ID-Converter, Crossref title-search) via the debug
//! `CITATIONS_*_ENDPOINT` seams — no network.

use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use crate::common::sync_probe::SyncProbe;
use crate::common::test_helpers::{
    create_user_with_no_permissions, create_user_with_only_permissions, create_user_with_permissions,
};
use crate::common::{TestServer, TestServerOptions};

mod real_egress;
mod real_llm;

/// JSON-RPC request to the citations MCP endpoint. `pub` so the submodules reuse it.
pub fn jsonrpc(
    server: &TestServer,
    token: &str,
    method: &str,
    params: Value,
) -> reqwest::RequestBuilder {
    reqwest::Client::new()
        .post(server.api_url("/citations/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
}

/// Loopback doi.org mock: `GET /{doi}` → canned CSL-JSON for known DOIs, 404 for
/// everything else (the fabricated-DOI case).
pub async fn start_mock_doi_resolver() -> String {
    use axum::{
        Json, Router, extract::Path, http::StatusCode, response::IntoResponse, routing::get,
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/{*doi}",
        get(|Path(doi): Path<String>| async move {
            let csl = match doi.to_lowercase().as_str() {
                "10.5555/known" => json!({
                    "type": "article-journal",
                    "title": "CRISPR interference in plant gene regulation",
                    "author": [{ "family": "Smith", "given": "J." }],
                    "container-title": "Nature",
                    "issued": { "date-parts": [[2021, 6, 14]] },
                    "DOI": "10.5555/known"
                }),
                "10.5555/other" => json!({
                    "type": "article-journal",
                    "title": "A completely different paper about quantum optics",
                    "author": [{ "family": "Doe", "given": "A." }],
                    "issued": { "date-parts": [[2019]] },
                    "DOI": "10.5555/other"
                }),
                _ => return StatusCode::NOT_FOUND.into_response(),
            };
            Json(csl).into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// Loopback NCBI ID-Converter mock: maps a PMID to {doi | record-no-doi | not-found}.
pub async fn start_mock_idconv() -> String {
    use axum::{
        Json, Router, extract::Query, response::IntoResponse, routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/",
        get(|q: Query<HashMap<String, String>>| async move {
            let ids = q.get("ids").cloned().unwrap_or_default();
            let record = match ids.as_str() {
                // Real PMID with a DOI → resolves to the known DOI.
                "33495596" => json!({ "pmid": "33495596", "doi": "10.5555/known" }),
                // Real record, but no DOI registered → unverified, not not_found.
                "11112222" => json!({ "pmid": "11112222" }),
                // No such record.
                _ => json!({ "pmid": ids, "status": "error", "errmsg": "invalid article id" }),
            };
            Json(json!({ "records": [record] })).into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}/")
}

/// Loopback Crossref mock: title query → best match (DOI) or empty.
pub async fn start_mock_crossref() -> String {
    use axum::{
        Json, Router, extract::Query, response::IntoResponse, routing::get,
    };
    use std::collections::HashMap;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = Router::new().route(
        "/",
        get(|q: Query<HashMap<String, String>>| async move {
            let query = q.get("query.bibliographic").cloned().unwrap_or_default();
            let items = if query.to_lowercase().contains("crispr") {
                json!([{ "DOI": "10.5555/known", "title": ["CRISPR interference in plant gene regulation"] }])
            } else {
                json!([])
            };
            Json(json!({ "message": { "items": items } })).into_response()
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app.into_make_service()).await;
    });
    format!("http://127.0.0.1:{port}/")
}

/// A TestServer wired to all three loopback resolver mocks (no network).
pub async fn server_with_mock_resolver() -> TestServer {
    let doi = start_mock_doi_resolver().await;
    let idconv = start_mock_idconv().await;
    let crossref = start_mock_crossref().await;
    TestServer::start_with_options(TestServerOptions {
        extra_env: vec![
            ("CITATIONS_RESOLVER_ENDPOINT".to_string(), doi),
            ("CITATIONS_IDCONV_ENDPOINT".to_string(), idconv),
            ("CITATIONS_CROSSREF_ENDPOINT".to_string(), crossref),
            ("CITATIONS_ALLOW_LOOPBACK".to_string(), "1".to_string()),
        ],
        ..Default::default()
    })
    .await
}

/// Call add_citations with a single item, returning the per-item result.
async fn add_one_item(server: &TestServer, token: &str, item: Value) -> Value {
    let res = jsonrpc(
        server,
        token,
        "tools/call",
        json!({ "name": "add_citations", "arguments": { "items": [item] } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    body["result"]["structuredContent"]["results"][0].clone()
}

async fn list_entries(server: &TestServer, token: &str) -> Vec<Value> {
    let res = jsonrpc(server, token, "tools/call", json!({ "name": "list_citations", "arguments": {} }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    body["result"]["structuredContent"]["entries"].as_array().cloned().unwrap_or_default()
}

// ─────────────────────────── MCP discovery + gating ───────────────────────────

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_init", &["citations::use"]).await;
    let res = jsonrpc(&server, &user.token, "initialize", json!({})).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "citations");
}

#[tokio::test]
async fn test_tools_list_has_six_batch_tools() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_list", &["citations::use"]).await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({})).send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let names: Vec<&str> = body["result"]["tools"].as_array().unwrap().iter()
        .map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["lookup_citations","add_citations","verify_citations","list_citations","format_citations","remove_citations"] {
        assert!(names.contains(&n), "missing tool {n}: {names:?}");
    }
}

#[tokio::test]
async fn test_tools_call_requires_use_permission() {
    let server = TestServer::start().await;
    let user = create_user_with_no_permissions(&server, "cit_noperm").await;
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "list_citations", "arguments": {} })).send().await.unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_default_users_group_grants_citations_use() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_default_only", &[]).await;
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "list_citations", "arguments": {} })).send().await.unwrap();
    assert_eq!(res.status(), 200, "default-Users member must pass citations::use");
}

// ─────────────────────────── resolve + verify (mock) ───────────────────

#[tokio::test]
async fn test_add_real_doi_is_verified_and_stored() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_add", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "id": "10.5555/known" })).await;
    assert_eq!(r["verification_status"], "verified", "{r}");
    assert_eq!(r["dedup_outcome"], "inserted", "{r}");

    // Fidelity: the stored record's title + citation_key reflect the resolved CSL.
    let entries = list_entries(&server, &user.token).await;
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["title"], "CRISPR interference in plant gene regulation");
    assert_eq!(entries[0]["citation_key"], "smith2021");
    assert_eq!(entries[0]["verification_status"], "verified");
}

/// Adding a citation publishes an owner-scoped `bibliography_entry`/`create`
/// sync frame (handlers.rs:250 → emit_library_changed) so the user's other
/// devices refetch `/api/citations`. A different user (not the owner audience)
/// stays silent. add_citations is a batch op, so the wire id is `Uuid::nil`.
#[tokio::test]
async fn test_add_citation_emits_owner_scoped_sync() {
    let server = server_with_mock_resolver().await;
    let owner = create_user_with_permissions(&server, "cit_sync_owner", &[]).await;
    let other = create_user_with_permissions(&server, "cit_sync_other", &[]).await;

    let mut owner_probe = SyncProbe::open(&server, &owner.token).await;
    let mut other_probe = SyncProbe::open(&server, &other.token).await;

    let r = add_one_item(&server, &owner.token, json!({ "id": "10.5555/known" })).await;
    assert_eq!(r["dedup_outcome"], "inserted", "{r}");

    let frame = owner_probe
        .expect_event("bibliography_entry", "create", Duration::from_secs(5))
        .await;
    assert_eq!(frame.id, Uuid::nil().to_string(), "batch add carries nil id");

    // Owner-scoped: an unrelated user is not in the audience.
    other_probe.expect_silence(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn test_fabricated_doi_is_not_found_and_not_stored() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fake", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "id": "10.9999/fake" })).await;
    assert_eq!(r["verification_status"], "not_found", "{r}");
    assert!(r["entry_id"].is_null(), "fabricated DOI must not be stored: {r}");
    assert_eq!(list_entries(&server, &user.token).await.len(), 0);
}

#[tokio::test]
async fn test_wrong_title_for_real_doi_is_mismatch() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_mismatch", &[]).await;
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "verify_citations", "arguments": { "items": [
            { "id": "10.5555/known", "title": "Totally unrelated nonsense about turtles" }
        ] } })).send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["structuredContent"]["results"][0]["verification_status"], "mismatch", "{body}");
}

#[tokio::test]
async fn test_pmid_resolves_via_idconv_is_verified() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_pmid_ok", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "id": "33495596", "kind": "pmid" })).await;
    assert_eq!(r["verification_status"], "verified", "{r}");
}

#[tokio::test]
async fn test_pmid_no_record_is_not_found() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_pmid_404", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "id": "00000001", "kind": "pmid" })).await;
    assert_eq!(r["verification_status"], "not_found", "{r}");
}

#[tokio::test]
async fn test_pmid_record_without_doi_is_unverified_not_not_found() {
    // A real PMID whose record has no DOI must be UNVERIFIED (legit), not
    // not_found (which would wrongly reject a real reference).
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_pmid_nodoi", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "id": "11112222", "kind": "pmid" })).await;
    assert_eq!(r["verification_status"], "unverified", "record-without-DOI must be unverified: {r}");
    assert_eq!(r["dedup_outcome"], "inserted", "and it must be stored: {r}");
}

#[tokio::test]
async fn test_title_search_resolves_to_real_doi() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_title_ok", &[]).await;
    let r = add_one_item(&server, &user.token,
        json!({ "title": "CRISPR interference in plant gene regulation" })).await;
    assert_eq!(r["verification_status"], "verified", "title-search should resolve+verify: {r}");
}

#[tokio::test]
async fn test_title_search_miss_is_unverified_not_not_found() {
    // A title that doesn't match any record is UNVERIFIED — a search miss is not
    // proof the work doesn't exist (must NOT become not_found).
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_title_miss", &[]).await;
    let r = add_one_item(&server, &user.token,
        json!({ "title": "An obscure unfindable monograph about nothing" })).await;
    assert_eq!(r["verification_status"], "unverified", "{r}");
}

#[tokio::test]
async fn test_idless_csl_is_unverified_and_stored() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_idless", &[]).await;
    let r = add_one_item(&server, &user.token, json!({ "csl": {
        "type": "book", "title": "A Hand-Typed Book With No DOI",
        "author": [{ "family": "Knuth", "given": "D." }], "issued": { "date-parts": [[1997]] }
    } })).await;
    assert_eq!(r["verification_status"], "unverified", "{r}");
    assert_eq!(r["dedup_outcome"], "inserted", "{r}");
    assert_eq!(list_entries(&server, &user.token).await.len(), 1);
}

// ─────────────────────────── dedup ───────────────────────────

#[tokio::test]
async fn test_same_doi_twice_dedups_to_one_entry() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_dedup", &[]).await;
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "add_citations", "arguments": { "items": [
            { "id": "10.5555/known" }, { "id": "https://doi.org/10.5555/known" }
        ] } })).send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let results = body["result"]["structuredContent"]["results"].as_array().unwrap();
    // First inserted, second linked to the SAME entry.
    assert_eq!(results[0]["dedup_outcome"], "inserted", "{body}");
    assert_eq!(results[1]["dedup_outcome"], "linked_existing", "{body}");
    assert_eq!(results[0]["entry_id"], results[1]["entry_id"], "both must map to one entry: {body}");
    assert_eq!(list_entries(&server, &user.token).await.len(), 1, "{body}");
}

/// CONCURRENT add race (audit all-5e6b3968a246). `test_same_doi_twice_…` only
/// covers the SEQUENTIAL case (two items in one call — the loser is caught by
/// the pre-insert `find_by_doi` dedup at handlers.rs:600). The genuine race is
/// two INDEPENDENT requests for the same DOI firing at once: both can pass the
/// pre-insert find (each sees no existing row), both reach `insert_entry`, one
/// wins the partial-unique index and the other gets a 409 that the retry loop
/// (handlers.rs:690-720) must resolve by re-finding the winner and returning a
/// `linked_existing` — never a duplicate row and never a `failed`/5xx.
#[tokio::test]
async fn test_concurrent_add_same_doi_races_to_one_entry() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_race", &[]).await;

    // Fire two add_citations for the SAME DOI concurrently (each `jsonrpc` builds
    // its own reqwest::Client → independent connections that truly race at the DB).
    let req = |token: &str| {
        jsonrpc(
            &server,
            token,
            "tools/call",
            json!({ "name": "add_citations", "arguments": { "items": [{ "id": "10.5555/known" }] } }),
        )
        .send()
    };
    let (a, b) = tokio::join!(req(&user.token), req(&user.token));

    let result_of = |res: reqwest::Response| async move {
        let body: Value = res.json().await.unwrap();
        body["result"]["structuredContent"]["results"][0].clone()
    };
    let ra = result_of(a.unwrap()).await;
    let rb = result_of(b.unwrap()).await;

    // Neither request errored: each is a stored citation (inserted by the winner,
    // linked_existing by the 409-retry loser) — never `failed`/null.
    for r in [&ra, &rb] {
        let outcome = r["dedup_outcome"].as_str().unwrap_or("");
        assert!(
            outcome == "inserted" || outcome == "linked_existing",
            "concurrent add must resolve the 409 race, not fail: {r}"
        );
        assert!(!r["entry_id"].is_null(), "a winning/linked entry id is required: {r}");
    }

    // Exactly one of them inserted; the other linked to that same row.
    let inserted = [&ra, &rb].iter().filter(|r| r["dedup_outcome"] == "inserted").count();
    assert_eq!(inserted, 1, "exactly one concurrent add must insert: a={ra} b={rb}");
    assert_eq!(
        ra["entry_id"], rb["entry_id"],
        "both concurrent adds must map to the SAME single entry: a={ra} b={rb}"
    );

    // And the DB holds exactly one bibliography entry for that DOI (the race did
    // not create a duplicate).
    let entries = list_entries(&server, &user.token).await;
    assert_eq!(entries.len(), 1, "the 409 retry must collapse the race to one row: {entries:?}");
}

#[tokio::test]
async fn test_idless_exact_reimport_dedups_by_fingerprint() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fp", &[]).await;
    let item = json!({ "csl": {
        "type": "book", "title": "Fingerprint Dedup Test",
        "author": [{ "family": "Lee", "given": "K." }], "issued": { "date-parts": [[2010]] }
    } });
    let r1 = add_one_item(&server, &user.token, item.clone()).await;
    assert_eq!(r1["dedup_outcome"], "inserted", "{r1}");
    let r2 = add_one_item(&server, &user.token, item).await;
    assert_eq!(r2["dedup_outcome"], "linked_existing", "exact id-less re-import must dedup: {r2}");
    assert_eq!(list_entries(&server, &user.token).await.len(), 1);
}

#[tokio::test]
async fn test_idless_near_title_is_possible_duplicate_not_merged() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fuzzy", &[]).await;
    add_one_item(&server, &user.token, json!({ "csl": {
        "type": "article-journal", "title": "Genome wide association study maize",
        "author": [{ "family": "Park", "given": "S." }], "issued": { "date-parts": [[2021]] }
    } })).await;
    let r2 = add_one_item(&server, &user.token, json!({ "csl": {
        "type": "article-journal", "title": "A genome-wide association study of maize kernels",
        "author": [{ "family": "Park", "given": "S." }], "issued": { "date-parts": [[2021]] }
    } })).await;
    assert_eq!(r2["dedup_outcome"], "possible_duplicate", "near-title must be flagged, not merged: {r2}");
    assert!(!r2["possible_duplicate_of"].is_null(), "{r2}");
    // Not merged AND not a 2nd insert → exactly one entry remains? No — the
    // possible-duplicate is NOT stored, so the library still has just the first.
    assert_eq!(list_entries(&server, &user.token).await.len(), 1, "{r2}");
}

#[tokio::test]
async fn test_batch_over_cap_is_rejected_not_truncated() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_cap", &[]).await;
    let items: Vec<Value> = (0..101).map(|i| json!({ "id": format!("10.1/{i}") })).collect();
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "verify_citations", "arguments": { "items": items } })).send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "over-cap batch must error, not truncate: {body}");
}

// ─────────────────────────── reverify (persists) ───────────────────────────

#[tokio::test]
async fn test_reverify_persists_status() {
    // Add an id-less entry (unverified), then REST reverify → still unverified
    // and persisted; add a real DOI → verified persists.
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_reverify", &[]).await;
    add_one_item(&server, &user.token, json!({ "id": "10.5555/known" })).await;
    let r = reqwest::Client::new()
        .post(server.api_url("/citations/reverify"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["results"][0]["verification_status"], "verified", "{body}");
}

// ─────────────────────────── REST surface ───────────────────────────

#[tokio::test]
async fn test_rest_import_list_export_delete_roundtrip() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_rest", &[]).await;
    let client = reqwest::Client::new();
    let auth = || format!("Bearer {}", user.token);

    let r = client.post(server.api_url("/citations/import")).header("Authorization", auth())
        .json(&json!({ "items": [{ "id": "10.5555/known" }] })).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let body: Value = r.json().await.unwrap();
    let entry_id = body["results"][0]["entry_id"].as_str().unwrap().to_string();
    assert_eq!(body["results"][0]["verification_status"], "verified");

    let r = client.get(server.api_url("/citations")).header("Authorization", auth()).send().await.unwrap();
    let lbody: Value = r.json().await.unwrap();
    assert_eq!(lbody["entries"].as_array().unwrap().len(), 1);

    // Export RIS.
    let r = client.get(server.api_url("/citations/export?format=ris")).header("Authorization", auth()).send().await.unwrap();
    let ebody: Value = r.json().await.unwrap();
    assert!(ebody["output"].as_str().unwrap().contains("TY  - JOUR"), "{ebody}");

    // Export BibTeX — title double-braced (capitalization-preserving).
    let r = client.get(server.api_url("/citations/export?format=bibtex")).header("Authorization", auth()).send().await.unwrap();
    let bbody: Value = r.json().await.unwrap();
    let bib = bbody["output"].as_str().unwrap();
    assert!(bib.contains("@") && bib.to_lowercase().contains("title"), "expected a BibTeX entry: {bib}");

    // Export CSL-JSON — valid JSON array.
    let r = client.get(server.api_url("/citations/export?format=csljson")).header("Authorization", auth()).send().await.unwrap();
    let jbody: Value = r.json().await.unwrap();
    let parsed: Value = serde_json::from_str(jbody["output"].as_str().unwrap()).unwrap();
    assert!(parsed.is_array(), "csljson export must be a JSON array");

    let r = client.delete(server.api_url(&format!("/citations/{entry_id}"))).header("Authorization", auth()).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let r = client.get(server.api_url("/citations")).header("Authorization", auth()).send().await.unwrap();
    let lbody: Value = r.json().await.unwrap();
    assert_eq!(lbody["entries"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_rest_requires_auth() {
    let server = TestServer::start().await;
    let res = reqwest::Client::new().get(server.api_url("/citations")).send().await.unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn test_styles_endpoint_lists_bundled() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_styles", &["citations::use"]).await;
    let res = reqwest::Client::new().get(server.api_url("/citations/styles"))
        .header("Authorization", format!("Bearer {}", user.token)).send().await.unwrap();
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["styles"].is_array(), "{body}");
}

// ─────────────────────────── project reference lists ───────────────────────────

async fn create_project(server: &TestServer, token: &str, name: &str) -> String {
    let r = reqwest::Client::new().post(server.api_url("/projects"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "name": name })).send().await.unwrap();
    assert_eq!(r.status(), 201, "create project");
    let body: Value = r.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_project_attach_then_detach_keeps_entry_in_library() {
    let server = server_with_mock_resolver().await;
    let perms = &["citations::use","citations::manage","projects::create","projects::read","projects::edit","projects::delete"];
    let user = create_user_with_permissions(&server, "cit_proj", perms).await;
    let client = reqwest::Client::new();
    let auth = || format!("Bearer {}", user.token);

    // Library entry + a project.
    let body: Value = client.post(server.api_url("/citations/import")).header("Authorization", auth())
        .json(&json!({ "items": [{ "id": "10.5555/known" }] })).send().await.unwrap().json().await.unwrap();
    let entry_id = body["results"][0]["entry_id"].as_str().unwrap().to_string();
    let project_id = create_project(&server, &user.token, "Manuscript X").await;

    // Attach → project list has it.
    let r = client.post(server.api_url(&format!("/projects/{project_id}/citations"))).header("Authorization", auth())
        .json(&json!({ "entry_ids": [entry_id] })).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let plist: Value = client.get(server.api_url(&format!("/citations?project_id={project_id}"))).header("Authorization", auth())
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(plist["entries"].as_array().unwrap().len(), 1, "attach should add to project list: {plist}");

    // Detach → project list empty BUT library still has it (unlink ≠ delete).
    let r = client.delete(server.api_url(&format!("/projects/{project_id}/citations/{entry_id}"))).header("Authorization", auth())
        .send().await.unwrap();
    assert_eq!(r.status(), 200);
    let plist: Value = client.get(server.api_url(&format!("/citations?project_id={project_id}"))).header("Authorization", auth())
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(plist["entries"].as_array().unwrap().len(), 0, "detach should empty the project list");
    let lib: Value = client.get(server.api_url("/citations")).header("Authorization", auth()).send().await.unwrap().json().await.unwrap();
    assert_eq!(lib["entries"].as_array().unwrap().len(), 1, "detach must NOT delete from the library");
}

#[tokio::test]
async fn test_mcp_add_then_remove_with_project_id_links_and_unlinks() {
    // The MCP tools accept an optional `project_id`: `add_citations` with it
    // must create the library entry AND link it to that project's reference
    // list in one call; `remove_citations` with it must UNLINK from the
    // project (not delete) — leaving the library entry intact. Only the
    // REST attach/detach endpoints were covered before; the MCP project_id
    // path (handlers.rs add_one→attach_to_project / remove→detach_from_project)
    // had no test. Uses a DOI-less CSL item so no resolver upstream is hit.
    let server = server_with_mock_resolver().await;
    let perms = &[
        "citations::use",
        "projects::create",
        "projects::read",
        "projects::edit",
        "projects::delete",
    ];
    let user = create_user_with_permissions(&server, "cit_mcp_proj", perms).await;
    let project_id = create_project(&server, &user.token, "MCP Manuscript").await;

    // MCP add_citations WITH project_id → entry created + linked to the project.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "add_citations", "arguments": {
            "project_id": project_id,
            "items": [{ "csl": {
                "type": "book",
                "title": "MCP-Linked Reference With No DOI",
                "author": [{ "family": "Lamport", "given": "L." }],
                "issued": { "date-parts": [[1978]] }
            } }]
        } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    let result = &body["result"]["structuredContent"]["results"][0];
    assert_eq!(result["verification_status"], "unverified", "{body}");
    let entry_id = result["entry_id"]
        .as_str()
        .expect("MCP add must persist a library entry")
        .to_string();

    // The project reference list (MCP list_citations scoped by project_id) has it.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "list_citations", "arguments": { "project_id": project_id } }),
    )
    .send()
    .await
    .unwrap();
    let plist: Value = res.json().await.unwrap();
    let entries = plist["result"]["structuredContent"]["entries"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(entries.len(), 1, "MCP add with project_id must link to the project: {plist}");
    assert_eq!(entries[0]["id"].as_str().unwrap(), entry_id);

    // MCP remove_citations WITH project_id → unlink from the project only.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "remove_citations", "arguments": {
            "project_id": project_id,
            "ids": [entry_id]
        } }),
    )
    .send()
    .await
    .unwrap();
    let rbody: Value = res.json().await.unwrap();
    assert_eq!(rbody["result"]["structuredContent"]["removed"], 1, "{rbody}");
    // The MCP text verb is "unlinked" (not "deleted") when project_id is set.
    assert!(
        rbody["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("unlinked"),
        "remove with project_id must unlink, not delete: {rbody}"
    );

    // Project list now empty …
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "list_citations", "arguments": { "project_id": project_id } }),
    )
    .send()
    .await
    .unwrap();
    let plist: Value = res.json().await.unwrap();
    assert_eq!(
        plist["result"]["structuredContent"]["entries"].as_array().unwrap().len(),
        0,
        "MCP remove with project_id must empty the project list: {plist}"
    );

    // … but the library entry survives (unlink ≠ delete).
    assert_eq!(
        list_entries(&server, &user.token).await.len(),
        1,
        "MCP project unlink must NOT delete the library entry"
    );
}

#[tokio::test]
async fn test_rest_manage_endpoints_require_manage_permission() {
    // The use/manage split is a real authorization boundary: a user with ONLY
    // citations::use can read/list/export/verify but must be blocked (403) from
    // the manage-gated mutations (import/reverify/delete/attach).
    let server = TestServer::start().await;
    let user = create_user_with_only_permissions(&server, "cit_use_only", &["citations::use"]).await;
    let client = reqwest::Client::new();
    let auth = || format!("Bearer {}", user.token);

    // use-gated read → 200.
    let r = client.get(server.api_url("/citations")).header("Authorization", auth()).send().await.unwrap();
    assert_eq!(r.status(), 200, "use-only user must be able to list");

    // manage-gated mutations → 403.
    let r = client.post(server.api_url("/citations/import")).header("Authorization", auth())
        .json(&json!({ "items": [{ "id": "10.5555/known" }] })).send().await.unwrap();
    assert_eq!(r.status(), 403, "import must require citations::manage");

    let r = client.post(server.api_url("/citations/reverify")).header("Authorization", auth())
        .send().await.unwrap();
    assert_eq!(r.status(), 403, "reverify must require citations::manage");

    let r = client.delete(server.api_url(&format!("/citations/{}", uuid::Uuid::new_v4())))
        .header("Authorization", auth()).send().await.unwrap();
    assert_eq!(r.status(), 403, "delete must require citations::manage");
}

#[tokio::test]
async fn test_mcp_write_allowed_with_use_only_but_rest_requires_manage() {
    // Documents/locks the intentional asymmetry: a citations::use-only user CAN
    // add via the MCP tool (own-data, model-driven) but is 403'd on REST import.
    let server = server_with_mock_resolver().await;
    let user = create_user_with_only_permissions(&server, "cit_use_mcp", &["citations::use"]).await;

    // MCP add → succeeds (gated only on citations::use).
    let r = add_one_item(&server, &user.token, json!({ "id": "10.5555/known" })).await;
    assert_eq!(r["verification_status"], "verified", "MCP add must work with use-only: {r}");
    assert_eq!(r["dedup_outcome"], "inserted");

    // REST import → 403 (requires citations::manage).
    let rest = reqwest::Client::new()
        .post(server.api_url("/citations/import"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "items": [{ "id": "10.5555/other" }] }))
        .send().await.unwrap();
    assert_eq!(rest.status(), 403, "REST import must require manage even when MCP add is use-gated");
}

#[tokio::test]
async fn test_reverify_does_not_downgrade_idless_verified_on_miss() {
    // The transient-downgrade guard: an identifier-less entry that is stored
    // `verified` must NOT be demoted to `unverified` by a title-search miss on
    // reverify (a flaky upstream must not corrupt a good badge). Seed the
    // (otherwise-unreachable) id-less+verified state directly via SQL, then
    // reverify with a title the crossref mock won't match.
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_no_downgrade", &[]).await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let uid = uuid::Uuid::parse_str(&user.user_id).unwrap();
    sqlx::query(
        r#"INSERT INTO bibliography_entries
           (user_id, csl_json, title, citation_key, verification_status, source)
           VALUES ($1, $2, $3, $4, 'verified', 'manual')"#,
    )
    .bind(uid)
    .bind(serde_json::json!({ "type": "book", "title": "An Unfindable Hand-Curated Monograph" }))
    .bind("curated2000")
    .bind("curated2000")
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let r = reqwest::Client::new()
        .post(server.api_url("/citations/reverify"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send().await.unwrap();
    assert_eq!(r.status(), 200);
    // The list still shows it as verified (the guard suppressed the downgrade).
    let entries = list_entries(&server, &user.token).await;
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["verification_status"], "verified",
        "id-less verified entry must NOT be downgraded by a title-search miss: {entries:?}"
    );
}

#[tokio::test]
async fn test_format_citations_mcp_tool() {
    // Exercises the MCP format_citations dispatch arm (distinct from REST export).
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fmt_mcp", &[]).await;
    add_one_item(&server, &user.token, json!({ "id": "10.5555/known" })).await;
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "format_citations", "arguments": { "format": "ris" } }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    let out = body["result"]["structuredContent"]["output"].as_str().unwrap_or("");
    assert!(out.contains("TY  - JOUR"), "format_citations(ris) should render RIS: {body}");
}

#[tokio::test]
async fn test_format_citations_inline_items_does_not_persist() {
    // The inline `items` path formats CSL-JSON DIRECTLY (no DB load, no library
    // write) — the SR export path. Crux: it must render AND leave the library
    // empty (the no-auto-persist consent contract).
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fmt_inline", &[]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "format_citations",
            "arguments": {
                "format": "ris",
                "items": [{
                    "type": "article-journal",
                    "title": "Inline Only Study",
                    "DOI": "10.1234/inline",
                    "author": [{ "family": "Doe", "given": "Jane" }],
                    "issued": { "date-parts": [[2022]] }
                }]
            }
        }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    let out = body["result"]["structuredContent"]["output"].as_str().unwrap_or("");
    assert!(out.contains("TY  - JOUR"), "inline items render RIS: {body}");
    assert!(out.contains("Inline Only Study"), "inline title is rendered: {body}");
    // No `ids`/`project_id` were given and `items` was supplied → nothing persisted.
    assert_eq!(
        list_entries(&server, &user.token).await.len(),
        0,
        "inline format_citations MUST NOT write the bibliography"
    );
}

#[tokio::test]
async fn test_format_citations_inline_items_over_cap_is_rejected() {
    // The inline path enforces the same MAX_BATCH_ITEMS (100) cap as the other
    // citation tools — 101 inline items → an in-band JSON-RPC error.
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_fmt_cap", &[]).await;
    let items: Vec<Value> = (0..101)
        .map(|i| json!({ "type": "article-journal", "title": format!("Study {i}") }))
        .collect();
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "format_citations", "arguments": { "format": "ris", "items": items } }),
    )
    .send()
    .await
    .unwrap();
    let body: Value = res.json().await.unwrap();
    let msg = serde_json::to_string(&body).unwrap_or_default();
    assert!(
        msg.contains("too many items"),
        "over-cap inline items must be rejected: {body}"
    );
}

#[tokio::test]
async fn test_remove_citations_mcp_deletes_from_library() {
    // Exercises the MCP remove_citations (delete-from-library) dispatch arm.
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_rm_mcp", &[]).await;
    let added = add_one_item(&server, &user.token, json!({ "id": "10.5555/known" })).await;
    let id = added["entry_id"].as_str().unwrap().to_string();
    assert_eq!(list_entries(&server, &user.token).await.len(), 1);
    let res = jsonrpc(&server, &user.token, "tools/call",
        json!({ "name": "remove_citations", "arguments": { "ids": [id] } }))
        .send().await.unwrap();
    let body: Value = res.json().await.unwrap();
    assert_eq!(body["result"]["structuredContent"]["removed"], 1, "{body}");
    assert_eq!(list_entries(&server, &user.token).await.len(), 0, "remove must delete from library");
}

#[tokio::test]
async fn test_cannot_attach_to_another_users_project() {
    // Cross-tenant guard: user B cannot attach into user A's project (404).
    let server = server_with_mock_resolver().await;
    let perms = &["citations::use","citations::manage","projects::create","projects::read","projects::edit","projects::delete"];
    let user_a = create_user_with_permissions(&server, "cit_owner_a", perms).await;
    let user_b = create_user_with_permissions(&server, "cit_owner_b", perms).await;
    let client = reqwest::Client::new();

    let project_a = create_project(&server, &user_a.token, "A's project").await;
    // B has an entry of their own.
    let body: Value = client.post(server.api_url("/citations/import"))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .json(&json!({ "items": [{ "id": "10.5555/known" }] })).send().await.unwrap().json().await.unwrap();
    let b_entry = body["results"][0]["entry_id"].as_str().unwrap().to_string();

    let r = client.post(server.api_url(&format!("/projects/{project_a}/citations")))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .json(&json!({ "entry_ids": [b_entry] })).send().await.unwrap();
    assert_eq!(r.status(), 404, "B must not attach into A's project (cross-tenant): {}", r.status());
}

// audit id all-60a187cda6c6 — the citations MCP JSON-RPC handler's error
// responses (handlers.rs jsonrpc_handler) were untested: a malformed JSON body
// → 400 parse_error, and an unknown method → in-band method_not_found.
#[tokio::test]
async fn test_jsonrpc_malformed_body_is_parse_error() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_parse_err", &["citations::use"]).await;
    // Send a body that is NOT valid JSON.
    let res = reqwest::Client::new()
        .post(server.api_url("/citations/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("content-type", "application/json")
        .body("{ not valid json ")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400, "malformed JSON must be a 400 parse error");
    let body: Value = res.json().await.unwrap_or_default();
    assert!(body["error"].is_object(), "must carry a JSON-RPC error: {body}");
    // JSON-RPC parse error code is -32700.
    assert_eq!(body["error"]["code"], -32700, "parse error code: {body}");
}

#[tokio::test]
async fn test_jsonrpc_unknown_method_is_method_not_found() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "cit_method_err", &["citations::use"]).await;
    let res = jsonrpc(&server, &user.token, "no_such_method", json!({}))
        .send()
        .await
        .unwrap();
    // Unknown method → 200 transport with an in-band method_not_found error.
    assert_eq!(res.status(), 200);
    let body: Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "unknown method must be an in-band error: {body}");
    assert_eq!(body["error"]["code"], -32601, "method_not_found code: {body}");
}

// audit id all-b871095789a4 — delete_citation must CASCADE its project_bibliography
// links (repository.rs delete_entry comment "cascades its project links"). The
// existing test covers DETACH (unlink, library keeps the entry); this covers
// full DELETE: the entry is removed AND its project link disappears (no orphan
// row left dangling in project_bibliography).
#[tokio::test]
async fn test_delete_citation_cascades_project_bibliography_link() {
    let server = server_with_mock_resolver().await;
    let perms = &["citations::use","citations::manage","projects::create","projects::read","projects::edit","projects::delete"];
    let user = create_user_with_permissions(&server, "cit_del_cascade", perms).await;
    let client = reqwest::Client::new();
    let auth = || format!("Bearer {}", user.token);

    let body: Value = client.post(server.api_url("/citations/import")).header("Authorization", auth())
        .json(&json!({ "items": [{ "id": "10.5555/known" }] })).send().await.unwrap().json().await.unwrap();
    let entry_id = body["results"][0]["entry_id"].as_str().unwrap().to_string();
    let project_id = create_project(&server, &user.token, "Cascade Project").await;

    // Attach the entry to the project.
    let r = client.post(server.api_url(&format!("/projects/{project_id}/citations"))).header("Authorization", auth())
        .json(&json!({ "entry_ids": [entry_id] })).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let plist: Value = client.get(server.api_url(&format!("/citations?project_id={project_id}"))).header("Authorization", auth())
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(plist["entries"].as_array().unwrap().len(), 1, "attach should add to project list");

    // DELETE the citation from the library entirely.
    let r = client.delete(server.api_url(&format!("/citations/{entry_id}"))).header("Authorization", auth())
        .send().await.unwrap();
    assert_eq!(r.status(), 200, "delete citation");
    assert_eq!(r.json::<Value>().await.unwrap()["ok"], true);

    // The project's citation list must now be EMPTY — the project_bibliography
    // link was cascade-removed, not left as an orphan pointing at a dead entry.
    let plist: Value = client.get(server.api_url(&format!("/citations?project_id={project_id}"))).header("Authorization", auth())
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(
        plist["entries"].as_array().unwrap().len(),
        0,
        "deleting the citation must cascade-remove its project_bibliography link: {plist}"
    );

    // And the entry is gone from the library too.
    let lib: Value = client.get(server.api_url("/citations")).header("Authorization", auth())
        .send().await.unwrap().json().await.unwrap();
    assert!(
        !lib["entries"].as_array().unwrap().iter().any(|e| e["id"].as_str() == Some(entry_id.as_str())),
        "deleted entry must be gone from the library"
    );
}

// audit id all-a7891dc5f3b3 — cross-subsystem: memory combined with citations
// (memory has only ever been tested in isolation). One user, one conversation:
// remember a research fact via memory_mcp AND add a CSL citation via
// citations_mcp; both per-user subsystems must persist and read back
// independently under the same conversation scope.
#[tokio::test]
async fn memory_and_citations_compose_for_one_user() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "mem_cit_user",
        &["citations::use", "citations::manage", "memory::read", "memory::write"],
    )
    .await;
    let conv = Uuid::new_v4().to_string();

    // Memory: remember + recall.
    let remember = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", &conv)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "remember", "arguments": { "content": "User writes about base editing safety", "kind": "fact" } } }))
        .send()
        .await
        .unwrap();
    assert_eq!(remember.status(), 200);
    let recall: Value = reqwest::Client::new()
        .post(server.api_url("/memories/mcp"))
        .header("Authorization", format!("Bearer {}", user.token))
        .header("x-conversation-id", &conv)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "recall", "arguments": { "query": "what does the user write about", "top_k": 5 } } }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        recall["result"]["structuredContent"]["memories"].as_array().cloned().unwrap_or_default()
            .iter().filter_map(|m| m["content"].as_str()).any(|c| c.contains("base editing safety")),
        "memory must recall the fact alongside citations: {recall}"
    );

    // Citations: add a CSL-only citation (no network resolve) + list it back.
    let added = add_one_item(
        &server,
        &user.token,
        json!({ "csl": { "type": "article-journal", "title": "Base Editing Safety Review",
                          "author": [{ "family": "Roe" }], "issued": { "date-parts": [[2022]] } } }),
    )
    .await;
    assert!(!added.is_null(), "add_citations returned a result: {added}");
    let titles: Vec<String> = list_entries(&server, &user.token)
        .await
        .iter()
        .filter_map(|e| e["title"].as_str().map(String::from))
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("Base Editing Safety Review")),
        "the citation must persist in the same user's library: {titles:?}"
    );
}

// audit id all-719c24437b13 — the citations MCP add/remove with a `project_id`
// (handlers.rs:241-252 add-to-project, 196-231 remove-from-project) was untested
// (only the REST attach/detach path was). add_citations{project_id} must link
// the entry to the project's reference list; remove_citations{project_id,ids}
// must UNLINK it (detach) while keeping it in the library.
#[tokio::test]
async fn test_mcp_add_and_remove_with_project_id() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "cit_mcp_proj",
        &["citations::use", "citations::manage", "projects::create", "projects::read", "projects::edit"],
    )
    .await;
    let pid = create_project(&server, &user.token, "Ref List Project").await;
    let client = reqwest::Client::new();

    // MCP add_citations WITH project_id → entry created + linked to the project.
    let add: Value = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "add_citations", "arguments": {
            "project_id": pid,
            "items": [{ "csl": { "type": "article-journal", "title": "Project Linked Paper", "issued": { "date-parts": [[2024]] } } }]
        }}),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(add["error"].is_null(), "add_citations with project_id: {add}");

    // The project's reference list contains it.
    let proj_list: Value = client
        .get(server.api_url(&format!("/projects/{pid}/citations")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entries = proj_list["entries"].as_array().or_else(|| proj_list.as_array()).cloned().unwrap_or_default();
    let entry = entries
        .iter()
        .find(|e| e["title"].as_str() == Some("Project Linked Paper"))
        .unwrap_or_else(|| panic!("entry must be in the project list: {proj_list}"));
    let entry_id = entry["id"].as_str().unwrap().to_string();

    // MCP remove_citations WITH project_id → unlink (detach), keep in library.
    let remove: Value = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "remove_citations", "arguments": { "project_id": pid, "ids": [entry_id] } }),
    )
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert!(remove["error"].is_null(), "remove_citations with project_id: {remove}");

    // Detached from the project...
    let after: Value = client
        .get(server.api_url(&format!("/projects/{pid}/citations")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let after_entries = after["entries"].as_array().or_else(|| after.as_array()).cloned().unwrap_or_default();
    assert!(
        !after_entries.iter().any(|e| e["title"].as_str() == Some("Project Linked Paper")),
        "entry must be unlinked from the project: {after}"
    );
    // ...but still in the user's library (unlink ≠ delete).
    assert!(
        list_entries(&server, &user.token).await.iter().any(|e| e["title"].as_str() == Some("Project Linked Paper")),
        "unlinked entry must remain in the library"
    );
/// Concurrent add of the SAME work exercises the 409-retry race (handlers.rs:630-695,
/// repository.rs partial-unique-index 409). Two simultaneous add_citations for the
/// same DOI: one wins the insert, the other's insert collides (409) and re-finds +
/// links to the winner. Net result MUST be exactly one entry with outcomes
/// {inserted, linked_existing} — never two rows, never an error. The existing dedup
/// tests are all SEQUENTIAL (second add hits the pre-insert find, not the 409 path).
#[tokio::test]
async fn test_concurrent_add_same_doi_dedups_via_409_retry() {
    let server = server_with_mock_resolver().await;
    let user = create_user_with_permissions(&server, "cit_race", &[]).await;
    let item = json!({ "id": "10.5555/known" });

    let (r1, r2) = tokio::join!(
        add_one_item(&server, &user.token, item.clone()),
        add_one_item(&server, &user.token, item.clone()),
    );

    let outcomes = {
        let mut v = vec![
            r1["dedup_outcome"].as_str().unwrap_or("").to_string(),
            r2["dedup_outcome"].as_str().unwrap_or("").to_string(),
        ];
        v.sort();
        v
    };
    assert_eq!(
        outcomes,
        vec!["inserted".to_string(), "linked_existing".to_string()],
        "concurrent same-DOI adds must net one insert + one link; got r1={r1}, r2={r2}"
    );

    // Exactly one entry persisted (no duplicate row from the race).
    let entries = list_entries(&server, &user.token).await;
    assert_eq!(entries.len(), 1, "the race must collapse to a single entry: {entries:?}");
}

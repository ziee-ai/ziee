//! Tier 2/3 — lit_search JSON-RPC MCP handler: discovery, permission gating,
//! the disabled + empty-query error paths, and the headline UNION search over
//! mock loopback upstreams (dedup + identified counts + completeness), plus the
//! S2 429-retry and the CORE-enabled-but-unkeyed self-skip.

use serde_json::json;

use crate::common::test_helpers::{create_user_with_no_permissions, create_user_with_permissions};
use crate::common::{TestServer, TestServerOptions};
use crate::lit_search::{
    configure, jsonrpc, start_mock_crossref, start_mock_europepmc, start_mock_s2_flaky,
    start_mock_s2_paper,
};

fn admin_perms() -> &'static [&'static str] {
    &["lit_search::admin::read", "lit_search::admin::manage"]
}

/// Start a TestServer with the loopback seam enabled + the given endpoint
/// overrides. Mocks MUST be started first (their ports go into the env).
async fn server_with_seams(overrides: Vec<(String, String)>) -> TestServer {
    let mut extra_env = vec![("LIT_SEARCH_ALLOW_LOOPBACK".to_string(), "1".to_string())];
    extra_env.extend(overrides);
    TestServer::start_with_options(TestServerOptions { extra_env, ..Default::default() }).await
}

#[tokio::test]
async fn test_initialize_returns_server_info() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_init", &["lit_search::use"]).await;
    let res = jsonrpc(&server, &user.token, "initialize", json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "lit_search");
}

#[tokio::test]
async fn test_tools_list_has_search_and_fetch() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_list", &["lit_search::use"]).await;
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
    assert!(names.contains(&"literature_search"), "tools: {names:?}");
    assert!(names.contains(&"fetch_paper_fulltext"), "tools: {names:?}");
}

#[tokio::test]
async fn test_tools_call_requires_use_permission() {
    let server = TestServer::start().await;
    let user = create_user_with_no_permissions(&server, "ls_noperm").await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 403);
}

#[tokio::test]
async fn test_default_users_group_grants_lit_search_use() {
    // A user whose ONLY source of lit_search::use is default-Users membership
    // (migration 101) must pass the gate. Empty perm list = registered + default
    // group, no custom perms. A 403 here means migration 101's grant is broken.
    let server = TestServer::start().await;
    // Disable the feature first (via an admin) so the default-user call returns an
    // in-band LIT_SEARCH_DISABLED error rather than firing LIVE requests to all 5
    // default keyless connectors — this test only needs to prove the permission
    // gate passes (200, not 403), not exercise real upstreams.
    let admin = create_user_with_permissions(&server, "ls_default_admin", admin_perms()).await;
    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);

    let user = create_user_with_permissions(&server, "ls_default_only", &[]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "x" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(
        res.status(),
        200,
        "default-Users member must pass the lit_search::use gate (migration 101)"
    );
    // 200 + in-band error (feature disabled) — proves the gate passed (not 403),
    // with no network egress.
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "expected in-band disabled error: {body}");
}

#[tokio::test]
async fn test_search_when_disabled_returns_in_band_error() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_disabled_admin", admin_perms()).await;
    let r = reqwest::Client::new()
        .put(server.api_url("/lit-search/settings"))
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
        json!({ "name": "literature_search", "arguments": { "query": "rust" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200); // JSON-RPC carries the error in-band
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_object(), "expected LIT_SEARCH_DISABLED in-band error: {body}");
}

#[tokio::test]
async fn test_empty_query_is_rejected() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_empty", &["lit_search::use"]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "   " } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(
        body["error"]["message"].as_str().unwrap_or("").contains("must not be empty"),
        "blank query should be rejected: {body}"
    );
}

#[tokio::test]
async fn test_union_search_dedups_and_counts_via_mocks() {
    // The headline path: two mock upstreams, one shared DOI → UNION + dedup +
    // per-source `identified` counts + completeness, all the way through the
    // MCP tools/call → aggregate → dedup → rank → structuredContent pipeline.
    let epmc = start_mock_europepmc().await;
    let crossref = start_mock_crossref().await;
    let server = server_with_seams(vec![
        ("LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(), format!("{epmc}/search")),
        ("LIT_SEARCH_CROSSREF_ENDPOINT".to_string(), format!("{crossref}/works")),
    ])
    .await;
    let admin = create_user_with_permissions(&server, "ls_union_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc", "crossref"]).await;

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
    let sc = &body["result"]["structuredContent"];

    assert_eq!(sc["identified"]["europepmc"], 2, "per-source identified: {body}");
    assert_eq!(sc["identified"]["crossref"], 2, "per-source identified: {body}");
    assert_eq!(sc["after_dedup"], 3, "shared DOI must collapse 4→3: {body}");
    assert_eq!(sc["records"].as_array().unwrap().len(), 3);
    assert!(
        sc["degraded_sources"].as_array().map(|a| a.is_empty()).unwrap_or(false),
        "no source failed: {body}"
    );

    // The shared record carries a merge audit trail from BOTH sources and keeps
    // the LONGER (crossref) abstract.
    let shared = sc["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["doi"] == "10.1/shared")
        .expect("shared record present");
    assert!(
        shared["source_ids"].as_array().unwrap().len() >= 2,
        "merged record should accumulate both source_ids: {shared}"
    );
    assert!(
        shared["abstract_text"].as_str().unwrap_or("").contains("longer abstract"),
        "merge must keep the longest abstract (crossref's): {shared}"
    );

    // Completeness shipped on → a labeled bucket, never a recall %.
    let estimate = sc["completeness"]["estimate"].as_str().unwrap_or("");
    assert!(
        ["low", "moderate", "high"].contains(&estimate),
        "completeness must be a labeled bucket: {body}"
    );

    // The text digest (what the model reads) names the query and is sized to
    // survive the kept-result cap.
    let text = body["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert!(text.contains("crispr"), "digest should echo the query: {text}");
    assert!(text.chars().count() <= 8000, "digest must stay within the kept cap");
}

#[tokio::test]
async fn test_semantic_scholar_retries_on_429() {
    // The S2 keyless pool 429s aggressively; the connector does one retry that
    // honors Retry-After. First mock hit = 429, second = 200 → records returned.
    let (s2, hits) = start_mock_s2_flaky().await;
    let server = server_with_seams(vec![(
        "LIT_SEARCH_S2_ENDPOINT".to_string(),
        format!("{s2}/search"),
    )])
    .await;
    let admin = create_user_with_permissions(&server, "ls_s2_admin", admin_perms()).await;
    configure(&server, &admin.token, &["semanticscholar"]).await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "transformers" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["records"].as_array().unwrap().len(), 1, "retry should yield the record: {body}");
    assert_eq!(sc["records"][0]["doi"], "10.1/s2only");
    assert_eq!(
        hits.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "expected one 429 then one successful retry"
    );
}

#[tokio::test]
async fn test_core_enabled_but_unkeyed_self_skips_into_degraded() {
    // CORE needs a key; enabled-but-unkeyed must self-skip (NO HTTP call) and be
    // recorded in degraded_sources, while the keyless sources still return.
    let epmc = start_mock_europepmc().await;
    let server = server_with_seams(vec![(
        "LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(),
        format!("{epmc}/search"),
    )])
    .await;
    let admin = create_user_with_permissions(&server, "ls_core_admin", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc", "core"]).await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "literature_search", "arguments": { "query": "genomics" } }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    let degraded: Vec<&str> = sc["degraded_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect();
    assert!(degraded.contains(&"core"), "unkeyed CORE must self-skip into degraded: {body}");
    assert!(
        !sc["records"].as_array().unwrap().is_empty(),
        "keyless europepmc must still return records: {body}"
    );
}

// ── systematic-review tools: dedup_records / verify_quote (mock-free) ──

/// A minimal valid `LitRecord` as the tools emit it.
fn rec(doi: &str, source: &str) -> serde_json::Value {
    json!({
        "doi": doi, "pmid": null, "title": format!("Study {doi}"),
        "abstract_text": null, "authors": ["A B"], "year": 2021, "venue": null,
        "url": null, "source": source, "source_ids": [format!("{source}:1")],
        "cited_by_count": null, "is_preprint": false, "relevance": 0.0
    })
}

#[tokio::test]
async fn test_tools_list_includes_sr_tools() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_srlist", &["lit_search::use"]).await;
    let res = jsonrpc(&server, &user.token, "tools/list", json!({}))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for t in ["dedup_records", "verify_quote", "fetch_references"] {
        assert!(names.contains(&t), "missing {t}: {names:?}");
    }
}

#[tokio::test]
async fn test_dedup_records_merges_by_doi_and_counts_identified() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_dedup", &["lit_search::use"]).await;
    // The same DOI (10.1/x) appears in both sets (europepmc + crossref) → merges
    // to one record; 10.2/y is distinct → 2 after dedup. Pre-dedup per-source
    // counts: europepmc 2, crossref 1.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "dedup_records",
            "arguments": {
                "record_sets": [
                    [rec("10.1/x", "europepmc"), rec("10.2/y", "europepmc")],
                    [rec("10.1/x", "crossref")]
                ],
                "query": "x"
            }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["after_dedup"], 2, "DOI 10.1/x dedups across sets: {sc}");
    assert_eq!(sc["identified"]["europepmc"], 2);
    assert_eq!(sc["identified"]["crossref"], 1);
}

#[tokio::test]
async fn test_dedup_records_counts_dropped_malformed() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_dedup_drop", &["lit_search::use"]).await;
    // One valid record + one malformed object (missing required fields) in the set:
    // the valid one survives, the malformed one is counted in `dropped`, no error.
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "dedup_records",
            "arguments": {
                "record_sets": [[ rec("10.1/ok", "europepmc"), { "not": "a record" } ]]
            }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["dropped"], 1, "the malformed record is counted as dropped: {body}");
    assert_eq!(sc["after_dedup"], 1, "the valid record still survives: {body}");
    assert_eq!(sc["union_capped"], false);
}

#[tokio::test]
async fn test_fetch_references_backward_returns_cited_works() {
    // Snowball: backward = the works the seed CITES. The S2 paper-graph mock
    // returns one cited paper; assert it surfaces in the deduped record set.
    let s2 = start_mock_s2_paper().await;
    let server = server_with_seams(vec![(
        "LIT_SEARCH_S2_PAPER_ENDPOINT".to_string(),
        s2,
    )])
    .await;
    let admin = create_user_with_permissions(&server, "ls_snowball_admin", admin_perms()).await;
    // The per-connector gate requires semanticscholar enabled.
    configure(&server, &admin.token, &["semanticscholar"]).await;

    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "fetch_references",
            "arguments": { "ids": ["10.1234/seed"], "direction": "backward" }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    let recs = sc["records"].as_array().expect("records array");
    assert!(
        recs.iter().any(|r| r["doi"] == "10.9/cited"),
        "backward snowball returns the cited reference: {body}"
    );
    assert!(
        !recs.iter().any(|r| r["doi"] == "10.9/citing"),
        "backward must NOT include the citing paper: {body}"
    );
}

#[tokio::test]
async fn test_fetch_references_forward_returns_citing_works() {
    // direction=forward maps each item's `citingPaper` (papers that CITE the seed),
    // the mirror of the backward case.
    let s2 = start_mock_s2_paper().await;
    let server = server_with_seams(vec![("LIT_SEARCH_S2_PAPER_ENDPOINT".to_string(), s2)]).await;
    let admin = create_user_with_permissions(&server, "ls_snowball_fwd", admin_perms()).await;
    configure(&server, &admin.token, &["semanticscholar"]).await;
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "fetch_references",
            "arguments": { "ids": ["10.1234/seed"], "direction": "forward" }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let recs = body["result"]["structuredContent"]["records"]
        .as_array()
        .expect("records array");
    assert!(
        recs.iter().any(|r| r["doi"] == "10.9/citing"),
        "forward snowball returns the citing paper: {body}"
    );
    assert!(
        !recs.iter().any(|r| r["doi"] == "10.9/cited"),
        "forward must NOT include the cited reference: {body}"
    );
}

#[tokio::test]
async fn test_fetch_references_invalid_direction_is_rejected() {
    // `direction` is validated before the settings/connector checks, so a bad
    // value is an in-band JSON-RPC error naming the field.
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_snow_dir", &["lit_search::use"]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({ "name": "fetch_references", "arguments": { "ids": ["10.1/x"], "direction": "sideways" } }),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    let msg = serde_json::to_string(&body).unwrap_or_default();
    assert!(msg.contains("direction"), "invalid direction must be rejected: {body}");
}

#[tokio::test]
async fn test_fetch_references_disabled_connector_is_rejected() {
    // The snowball tool honors the per-connector enable gate: with semanticscholar
    // NOT enabled, the call is rejected rather than hitting S2 anyway.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "ls_snowball_off", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await; // semanticscholar NOT enabled
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({ "name": "fetch_references", "arguments": { "ids": ["10.1234/seed"] } }),
    )
    .send()
    .await
    .unwrap();
    let body: serde_json::Value = res.json().await.unwrap();
    // The JSON-RPC error carries the human message (not the error code), so
    // assert on the message text the response actually contains.
    let msg = serde_json::to_string(&body).unwrap_or_default();
    assert!(
        msg.contains("Semantic Scholar") && msg.contains("not enabled"),
        "disabled snowball connector must be rejected: {body}"
    );
}

#[tokio::test]
async fn test_verify_quote_uncached_paper_reports_not_cached() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_vq", &["lit_search::use"]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "verify_quote",
            "arguments": { "id": "10.9999/never-fetched", "quote": "some claimed span" }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    assert_eq!(sc["status"], "not_cached", "uncached paper: {body}");
    assert_eq!(sc["verified"], false);
}

#[tokio::test]
async fn test_select_included_partitions_decisions_via_http() {
    // The `select_included` SR tool takes a `decisions` array and returns the
    // de-duplicated `included_ids` plus include/exclude/skip counts. Drive it
    // through the real tools/call HTTP path (no upstream — pure decision logic).
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_select_incl", &["lit_search::use"]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "select_included",
            "arguments": {
                "decisions": [
                    { "id": "p1", "decision": "include" },
                    { "id": "p2", "decision": "exclude" },
                    { "id": "p1", "decision": "include" },   // duplicate id → deduped
                    { "id": "p3", "decision": "include" },
                    null,                                       // dropped llm_map item → skipped
                    { "no_decision_field": true }               // non-decision object → excluded path
                ]
            }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let sc = &body["result"]["structuredContent"];
    let ids: Vec<&str> = sc["included_ids"]
        .as_array()
        .expect("included_ids array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(ids, ["p1", "p3"], "deduped, first-seen order: {sc}");
    assert_eq!(sc["included"], 2, "two distinct included: {sc}");
    assert_eq!(sc["skipped"], 1, "the null entry is skipped: {sc}");
    // p2 (exclude) + the object missing `decision` both count as excluded.
    assert_eq!(sc["excluded"], 2, "non-include decisions are excluded: {sc}");
}

/// Inverted year-range rejection (gap 6df0f4c323be, handlers.rs:172-179): a
/// literature_search with year_from > year_to is rejected with a VALIDATION
/// error (otherwise it would silently yield zero results). The check runs
/// before the connector fan-out, so no mock upstream is needed.
#[tokio::test]
async fn test_inverted_year_range_is_rejected() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "ls_years", &["lit_search::use"]).await;
    let res = jsonrpc(
        &server,
        &user.token,
        "tools/call",
        json!({
            "name": "literature_search",
            "arguments": { "query": "crispr", "year_from": 2024, "year_to": 2000 }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("year_from") && msg.contains("year_to"),
        "inverted year range must be rejected naming both bounds: {body}"
    );
}

/// A valid (non-inverted) equal year range passes the inversion guard (it must
/// not reject from == to). Uses a mock Europe PMC so the call completes.
#[tokio::test]
async fn test_equal_year_range_passes_inversion_guard() {
    let epmc = start_mock_europepmc().await;
    let server = server_with_seams(vec![(
        "LIT_SEARCH_EUROPEPMC_ENDPOINT".to_string(),
        format!("{epmc}/search"),
    )])
    .await;
    let admin = create_user_with_permissions(&server, "ls_years_ok", admin_perms()).await;
    configure(&server, &admin.token, &["europepmc"]).await;
    let res = jsonrpc(
        &server,
        &admin.token,
        "tools/call",
        json!({
            "name": "literature_search",
            "arguments": { "query": "crispr", "year_from": 2021, "year_to": 2021 }
        }),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.unwrap();
    assert!(body["error"].is_null(), "from==to must NOT be rejected: {body}");
}

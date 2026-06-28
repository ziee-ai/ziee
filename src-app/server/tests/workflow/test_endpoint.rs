//! `POST /workflows/{id}/test` — the dev-surface bundled-fixture runner
//! (`workflow/handlers/dev.rs::test_workflow`). No prior test called this
//! endpoint.
//!
//! Coverage:
//! - A bundle carrying two `mode: ci` fixtures under `tests/` (every llm
//!   step mocked) runs through the REAL runner: one fixture's
//!   `expected_outputs` assertion passes, the other's fails →
//!   `{total: 2, passed: 1, failed: 1}` with the failing fixture's
//!   `failure` payload naming the `contains` assertion. (The `memo`
//!   output is `{{ write.output }}`, so a mock `write` value drives the
//!   pass/fail split deterministically — no provider is ever invoked.)
//! - A bundle with no `tests/` dir → `{total: 0, ...}` (the empty branch).
//! - A nonexistent workflow id → 404 (the access gate).

use std::io::Write;

use serde_json::{Value as Json, json};

use super::{FIXTURE_WORKFLOW_YAML, import_dev_workflow, plain_server, stub_conversation, workflow_user};

/// A `mode: ci` fixture mocking every llm step of `FIXTURE_WORKFLOW_YAML`.
/// `write` returns the marker the `memo` output surfaces; the fixture's
/// single `contains` assertion is templated in so one copy passes and one
/// fails (deterministic, no provider invoked).
fn ci_fixture(needle: &str) -> String {
    format!(
        r#"mode: ci
inputs:
  topic: widgets
mocks:
  research:
    - title: Mock A
      url: https://example.com/a
  summarize:
    - mocked summary line
  write: "MEMO_BODY_MARKER: deterministic memo from a ci fixture"
expected_outputs:
  memo:
    contains: "{needle}"
"#
    )
}

/// Build a `bundle.tar.gz` from `(path, contents)` entries — like the
/// shared `workflow_tarball` but able to carry `tests/*.yaml` fixtures.
fn bundle_with(entries: &[(&str, &str)]) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(enc);
    for (path, contents) in entries {
        let mut header = Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .expect("append tar entry");
    }
    let enc = builder.into_inner().expect("tar into_inner");
    let mut bytes = enc.finish().expect("gz finish");
    bytes.flush().ok();
    bytes
}

/// Dev-import a bundle (is_dev=true) carrying arbitrary files, returning
/// the created workflow JSON. Mirrors `import_dev_workflow` but lets the
/// caller ship `tests/` fixtures alongside `workflow.yaml`.
async fn import_bundle(server: &crate::common::TestServer, token: &str, slug: &str, entries: &[(&str, &str)]) -> Json {
    let part = reqwest::multipart::Part::bytes(bundle_with(entries))
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("bundle", part);
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/import?name={slug}")))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("import bundle");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse import body");
    assert_eq!(status, 201, "dev import should 201; got {status}: {body}");
    body
}

#[tokio::test]
async fn test_workflow_runs_ci_fixtures_pass_and_fail() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_test_ep").await;

    // A model snapshot source — ci fixtures never call the provider, but
    // `run_one_fixture` still needs a resolvable model (else: skipped).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    // load_fixtures sorts by file stem, so `aaa_*` precedes `zzz_*`.
    let passing = ci_fixture("MEMO_BODY_MARKER");
    let failing = ci_fixture("THIS_STRING_NEVER_APPEARS_XYZZY");

    let wf = import_bundle(
        &server,
        &user.token,
        "test-endpoint-fixtures",
        &[
            ("workflow.yaml", FIXTURE_WORKFLOW_YAML),
            ("tests/aaa_passing.yaml", passing.as_str()),
            ("tests/zzz_failing.yaml", failing.as_str()),
        ],
    )
    .await;
    let wf_id = wf["id"].as_str().expect("workflow id");

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{wf_id}/test")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "conversation_id": conv_id.to_string() }))
        .send()
        .await
        .expect("POST /test");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse test body");
    assert_eq!(status, 200, "test endpoint should 200; got {status}: {body}");

    assert_eq!(body["total"], 2, "two fixtures ran: {body}");
    assert_eq!(body["passed"], 1, "exactly the passing fixture passed: {body}");
    assert_eq!(body["failed"], 1, "exactly the failing fixture failed: {body}");
    assert_eq!(body["skipped"], 0, "no fixture skipped (a model was resolvable): {body}");

    let results = body["results"].as_array().expect("results array");
    let pass = results.iter().find(|r| r["name"] == "aaa_passing").expect("aaa_passing result");
    assert_eq!(pass["passed"], true, "aaa_passing must pass (memo contains the marker): {pass}");
    let failr = results.iter().find(|r| r["name"] == "zzz_failing").expect("zzz_failing result");
    assert_eq!(failr["passed"], false, "zzz_failing must fail: {failr}");
    assert_eq!(
        failr["failure"]["assertion"], "contains",
        "the failure must name the unmet `contains` assertion: {failr}"
    );
    assert_eq!(failr["failure"]["output_name"], "memo", "failure is on the `memo` output: {failr}");
}

#[tokio::test]
async fn test_workflow_with_no_fixtures_returns_zero_total() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_test_nofix").await;
    // The shared importer ships only workflow.yaml → no tests/ dir.
    let wf = import_dev_workflow(&server, &user.token, "test-endpoint-empty", FIXTURE_WORKFLOW_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id");

    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{wf_id}/test")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /test");
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.expect("parse");
    assert_eq!(body["total"], 0, "no fixtures → zero total: {body}");
    assert_eq!(body["results"].as_array().map(|a| a.len()), Some(0), "empty results: {body}");
}

#[tokio::test]
async fn test_workflow_nonexistent_id_returns_404() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_test_404").await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{}/test", uuid::Uuid::new_v4())))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({}))
        .send()
        .await
        .expect("POST /test");
    assert_eq!(resp.status(), 404, "unknown workflow id → 404 (access gate)");
}

//! Audit gaps S1–S4 + S7: per-run read-back surface access control.
//!
//! Every per-run read endpoint (`get_run`, `cancel_run`, the artifact
//! stream, the log stream) gates on `row.user_id == auth.user.id` and
//! returns 403 `WORKFLOW_RUN_FORBIDDEN` for a cross-user caller. The
//! artifact + log streams additionally reject path-traversal in the
//! caller-supplied path segments BEFORE touching the DB (400).
//!
//!   S1: cross-user GET    /workflow-runs/{id}                       → 403
//!   S2: cross-user POST   /workflow-runs/{id}/cancel                → 403
//!   S3: cross-user GET    /workflow-runs/{id}/artifact/{step}/{file}→ 403
//!   S4: cross-user GET    /workflow-runs/{id}/logs/{step}/{kind}    → 403
//!   S7: path-traversal guards on the artifact filename + log kind/step_id → 400
//!
//! Handler-behavior notes (verified against the real handlers — the
//! assertions below match REALITY, not the audit's a-priori expectation):
//!
//!   * `artifact_stream::read_artifact` runs its `filename` traversal guard
//!     (line 24, `ARTIFACT_PATH_INVALID`) FIRST, then `find_run`, then the
//!     OWNERSHIP check (line 33), and only THEN looks up the step's artifacts.
//!     So for a cross-user caller hitting an EXISTING run with a plausible
//!     (non-traversal) filename, ownership 403s before the artifact-not-found
//!     404 — S3 uses a real run owned by user A to exercise that ordering.
//!   * `log_stream::read_log` runs its `kind`-allowlist guard (line 31,
//!     `WORKFLOW_LOG_BAD_KIND`) FIRST, then the `step_id` traversal guard
//!     (line 44, `WORKFLOW_LOG_BAD_STEP_ID`), then `find_run`, then OWNERSHIP
//!     (line 53). So a cross-user caller must use a VALID `kind` to reach the
//!     ownership check — S4 uses `kind = "prompt"` on user A's real run.

use serde_json::json;
use uuid::Uuid;

use super::{
    SIMPLE_OK_YAML, import_dev_workflow, plain_server, poll_run, run_workflow, stub_model_for,
    workflow_user,
};
use crate::common::test_helpers::create_user_with_permissions;

/// Set up a real COMPLETED run owned by `owner`: dev-import SIMPLE_OK_YAML,
/// run it with a stub model + a `gen` mock, poll to terminal. Returns the
/// run id. The stub guard must be kept alive for the run's duration, so it
/// is returned alongside.
async fn completed_run_owned_by(
    server: &crate::common::TestServer,
    owner: &crate::common::test_helpers::TestUser,
    slug: &str,
) -> (crate::common::stub_engine::StubEngine, Uuid) {
    let (stub, model_id) = stub_model_for(server, &owner.user_id).await;
    let wf = import_dev_workflow(server, &owner.token, slug, SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id");
    let run = run_workflow(
        server,
        &owner.token,
        wf_id,
        json!({ "inputs": { "topic": "t" }, "model_id": model_id.to_string(), "mocks": { "gen": "x" } }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().expect("run_id")).unwrap();
    let final_run = poll_run(server, &owner.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "run should complete: {final_run}"
    );
    (stub, run_id)
}

/// A second user with the read + execute perms the run-level endpoints gate
/// on — so a 403 is the OWNERSHIP check firing, not a missing-permission 403.
async fn intruder(
    server: &crate::common::TestServer,
    name: &str,
) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(server, name, &["workflows::read", "workflows::execute"]).await
}

// ── S1: cross-user GET /workflow-runs/{id} ──────────────────────────────────

#[tokio::test]
async fn get_run_cross_user_is_forbidden() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_get_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "s1-get").await;

    let other = intruder(&server, "wf_get_intruder").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("get run cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user GET run must 403: {body}");
    assert!(
        body.contains("WORKFLOW_RUN_FORBIDDEN"),
        "code surfaced: {body}"
    );
}

// ── S2: cross-user POST /workflow-runs/{id}/cancel ──────────────────────────

#[tokio::test]
async fn cancel_run_cross_user_is_forbidden() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_cancel_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "s2-cancel").await;

    let other = intruder(&server, "wf_cancel_intruder").await;
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflow-runs/{run_id}/cancel")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("cancel run cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user cancel must 403: {body}");
    assert!(
        body.contains("WORKFLOW_RUN_FORBIDDEN"),
        "code surfaced: {body}"
    );
}

// ── S3: cross-user GET artifact stream ──────────────────────────────────────

#[tokio::test]
async fn artifact_stream_cross_user_is_forbidden() {
    // The artifact handler checks ownership (line 33) BEFORE the per-step
    // artifact lookup (line 40), so a cross-user caller hitting user A's
    // REAL run with a plausible (non-traversal) filename gets 403, not 404.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_art_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "s3-artifact").await;

    let other = intruder(&server, "wf_art_intruder").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{run_id}/artifact/gen/output.txt"
        )))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("artifact stream cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user artifact read must 403: {body}");
    assert!(
        body.contains("WORKFLOW_RUN_FORBIDDEN"),
        "code surfaced: {body}"
    );
}

// ── S4: cross-user GET log stream ───────────────────────────────────────────

#[tokio::test]
async fn log_stream_cross_user_is_forbidden() {
    // The log handler runs the `kind` allowlist guard first, so a cross-user
    // caller must use a VALID kind ("prompt") to reach the ownership check
    // (line 53), which 403s before the on-disk read.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_log_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "s4-log").await;

    let other = intruder(&server, "wf_log_intruder").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/logs/gen/prompt")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("log stream cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user log read must 403: {body}");
    assert!(
        body.contains("WORKFLOW_RUN_FORBIDDEN"),
        "code surfaced: {body}"
    );
}

// ── S7: path-traversal guards (no run needed — the guards run first) ─────────

#[tokio::test]
async fn artifact_filename_dotdot_traversal_is_rejected() {
    // `filename.contains("..")` → 400 ARTIFACT_PATH_INVALID (artifact_stream.rs:24).
    // The guard runs BEFORE find_run, so a freshly-minted (nonexistent) run id
    // is fine — and the OWNER's token reaches the guard (it gates on the
    // filename, not ownership).
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_art_traversal").await;
    let run_id = Uuid::new_v4();

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{run_id}/artifact/gen/..%2F..%2Fetc%2Fpasswd"
        )))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("artifact traversal");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "a `..`-containing artifact filename must 400: {body}"
    );
    assert!(
        body.contains("ARTIFACT_PATH_INVALID"),
        "code surfaced: {body}"
    );
}

#[tokio::test]
async fn artifact_filename_absolute_is_rejected() {
    // `filename.starts_with('/')` → 400 ARTIFACT_PATH_INVALID. Encode the
    // leading slash so axum routes it into the `{filename}` capture rather
    // than treating it as a new path segment.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_art_absolute").await;
    let run_id = Uuid::new_v4();

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{run_id}/artifact/gen/%2Fetc%2Fpasswd"
        )))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("artifact absolute");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "an absolute artifact filename must 400: {body}"
    );
    assert!(
        body.contains("ARTIFACT_PATH_INVALID"),
        "code surfaced: {body}"
    );
}

#[tokio::test]
async fn log_unknown_kind_is_rejected() {
    // `!ALLOWED_KINDS.contains(kind)` → 400 WORKFLOW_LOG_BAD_KIND (log_stream.rs:31).
    // This guard runs before find_run/ownership, so any authenticated reader
    // hits it.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_log_badkind").await;
    let run_id = Uuid::new_v4();

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{run_id}/logs/gen/etc-passwd"
        )))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("log bad kind");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 400, "an unrecognized log kind must 400: {body}");
    assert!(
        body.contains("WORKFLOW_LOG_BAD_KIND"),
        "code surfaced: {body}"
    );
}

#[tokio::test]
async fn log_step_id_dotdot_traversal_is_rejected() {
    // `step_id.contains("..")` → 400 WORKFLOW_LOG_BAD_STEP_ID (log_stream.rs:44).
    // The kind allowlist guard runs FIRST, so use a VALID kind ("prompt") to
    // reach the step_id guard. The step segment carries a literal `..` (axum's
    // `{step_id}` capture can't span a `/`, but a bare `..` substring is enough
    // to trip the guard).
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_log_stepid_traversal").await;
    let run_id = Uuid::new_v4();

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/logs/gen..bad/prompt")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("log step_id traversal");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "a `..`-containing log step_id must 400: {body}"
    );
    assert!(
        body.contains("WORKFLOW_LOG_BAD_STEP_ID"),
        "code surfaced: {body}"
    );
}

// ── read_output (GET /workflow-runs/{run_id}/output/{step_id}) edge cases ────
//
// `output_stream::read_output` has four distinct exit paths and none had a
// dedicated test (audit all-e9f9800f741c): (a) run not found → 404
// `RESOURCE_NOT_FOUND` "WorkflowRun not found"; (b) cross-user → 403
// `WORKFLOW_RUN_FORBIDDEN` (ownership is checked BEFORE the per-step lookup,
// so an intruder hitting a real run + a real step still 403s, never 404);
// (c) an unknown step_id on an OWNED run → 404 "step output not found";
// (d) the happy path → 200 streaming the `gen` step's text output. The four
// tests below pin each branch.

#[tokio::test]
async fn output_read_happy_path_streams_step_output() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_out_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "out-ok").await;

    // `gen` is SIMPLE_OK_YAML's only step; its output file exists on a
    // completed run, so the owner streams it back with a 200.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/gen")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("owner read gen output");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 200, "owner must read its own step output: {body}");
    assert!(!body.is_empty(), "streamed output should not be empty");
}

#[tokio::test]
async fn output_read_nonexistent_run_is_404() {
    let server = plain_server().await;
    // A user holding the gating perm but pointing at a random run id: the
    // `find_run(...).ok_or(not_found("WorkflowRun"))?` arm fires.
    let user = intruder(&server, "wf_out_norun").await;
    let missing = Uuid::new_v4();
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{missing}/output/gen")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("read output of missing run");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 404, "unknown run must 404: {body}");
    assert!(
        body.contains("WorkflowRun"),
        "missing-run message surfaced: {body}"
    );
}

#[tokio::test]
async fn output_read_cross_user_is_forbidden() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_out_xuser_owner").await;
    // A REAL run owned by `owner` with a REAL step (`gen`): ownership is
    // checked before the step lookup, so the intruder gets 403, not 404.
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "out-xuser").await;

    let other = intruder(&server, "wf_out_xuser_intruder").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/gen")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("cross-user read output");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user output read must 403: {body}");
    assert!(
        body.contains("WORKFLOW_RUN_FORBIDDEN"),
        "ownership 403 surfaced (not a step 404): {body}"
    );
}

#[tokio::test]
async fn output_read_unknown_step_is_404() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_out_nostep").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "out-nostep").await;

    // Owned run, but a step id that never produced an output → the
    // `step_outputs_json.get(...).ok_or(not_found("step output"))?` arm.
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/does-not-exist")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("read unknown step output");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 404, "unknown step must 404: {body}");
    assert!(
        body.contains("step output"),
        "step-not-found message surfaced (distinct from missing-run): {body}"
    );
}

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

// ── output_stream (read_output) error paths — fd22d822 ───────────────────────

#[tokio::test]
async fn output_stream_cross_user_is_forbidden() {
    // read_output checks ownership (output_stream.rs:27) before the per-step
    // output lookup, so a cross-user caller on user A's real run gets 403.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_out_owner").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "out-cross").await;

    let other = intruder(&server, "wf_out_intruder").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/gen")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("output stream cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user output read must 403: {body}");
    assert!(body.contains("WORKFLOW_RUN_FORBIDDEN"), "code surfaced: {body}");
}

#[tokio::test]
async fn output_stream_missing_run_is_404() {
    // A valid user requesting an output for a run that doesn't exist → 404
    // (the find_run guard at output_stream.rs:24-26).
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_out_missing").await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{}/output/gen",
            uuid::Uuid::new_v4()
        )))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("output stream missing run");
    assert_eq!(resp.status(), 404, "a nonexistent run's output must 404");
}

#[tokio::test]
async fn output_stream_unknown_step_is_404() {
    // An existing, owned run but a step name with no recorded output → 404
    // (the step_outputs_json lookup at output_stream.rs:34-37).
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_out_badstep").await;
    let (_stub, run_id) = completed_run_owned_by(&server, &owner, "out-badstep").await;

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflow-runs/{run_id}/output/no_such_step"
        )))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("output stream unknown step");
    assert_eq!(resp.status(), 404, "an unknown step's output must 404");
}

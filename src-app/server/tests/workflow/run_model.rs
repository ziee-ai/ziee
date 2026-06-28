//! A1 — standalone run with an explicit `model_id` (no conversation).
//!
//! Plan Part-B matrix item 4:
//!   - `POST /workflows/{id}/run` with `model_id` → 202 + the `workflow_runs`
//!     row has `model_id` set, `conversation_id` NULL, `invocation_source =
//!     'manual'`;
//!   - an INACCESSIBLE model → 403 `ACCESS_DENIED`;
//!   - neither `model_id` nor `conversation_id` → 400 `WORKFLOW_NO_MODEL_SOURCE`.
//!
//! The workflow is a single-step `llm` whose only step is mock-short-circuited,
//! so no token is spent; the run still snapshots the model at start.

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    SIMPLE_OK_YAML, db_pool, import_dev_workflow, plain_server, poll_run, run_workflow,
    stub_conversation, stub_model_for, workflow_user,
};

/// Issue a `POST /workflows/{id}/run` and return the raw response (so callers
/// can assert non-202 statuses too).
async fn post_run(
    server: &crate::common::TestServer,
    token: &str,
    workflow_id: &str,
    body: Json,
) -> reqwest::Response {
    reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{workflow_id}/run")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("post run")
}

#[tokio::test]
async fn standalone_run_with_model_id_sets_manual_source_and_null_conversation() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_run_model_ok").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let wf = import_dev_workflow(&server, &user.token, "model-run", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    // model_id, no conversation, mock the sole `gen` step → completes fast.
    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "t" },
            "model_id": model_id.to_string(),
            "mocks": { "gen": "mocked output" }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    // The run row: model_id set, conversation_id NULL, invocation_source manual.
    let pool = db_pool(&server).await;
    let row = sqlx::query_as::<_, (Option<Uuid>, Option<Uuid>, String)>(
        "SELECT model_id, conversation_id, invocation_source FROM workflow_runs WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("run row");
    assert_eq!(row.0, Some(model_id), "model_id is the explicit model");
    assert_eq!(row.1, None, "standalone run has NULL conversation_id");
    assert_eq!(row.2, "manual", "REST /run path is invocation_source=manual");
    pool.close().await;

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "mocked standalone run completes: {final_run}"
    );
}

#[tokio::test]
async fn run_with_inaccessible_model_is_forbidden() {
    let server = plain_server().await;
    let runner = workflow_user(&server, "wf_run_model_403").await;

    // A model created for + accessible to a DIFFERENT user — `runner` has no
    // provider access to it.
    let other = workflow_user(&server, "wf_other_owner").await;
    let (_stub, other_model_id) = stub_model_for(&server, &other.user_id).await;

    let wf = import_dev_workflow(&server, &runner.token, "model-run-403", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let resp = post_run(
        &server,
        &runner.token,
        wf_id,
        json!({
            "inputs": { "topic": "t" },
            "model_id": other_model_id.to_string(),
        }),
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 403,
        "running with an inaccessible model must 403: {body}"
    );
    assert!(
        body.contains("ACCESS_DENIED"),
        "403 surfaces ACCESS_DENIED: {body}"
    );
}

#[tokio::test]
async fn run_with_no_model_source_is_bad_request() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_run_no_source").await;

    let wf = import_dev_workflow(&server, &user.token, "model-run-400", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    // Neither model_id nor conversation_id.
    let resp = post_run(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": { "topic": "t" } }),
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 400,
        "no model source must 400: {body}"
    );
    assert!(
        body.contains("WORKFLOW_NO_MODEL_SOURCE"),
        "400 surfaces WORKFLOW_NO_MODEL_SOURCE: {body}"
    );
}

#[tokio::test]
async fn run_with_foreign_conversation_id_is_rejected() {
    // SECURITY (cross-tenant): an explicit `model_id` PLUS a `conversation_id`
    // the caller does NOT own must be rejected — otherwise the foreign
    // conversation_id becomes this run's sandbox workspace key, mounting the
    // victim's workspace into the attacker's `kind: sandbox` step.
    let server = plain_server().await;
    let runner = workflow_user(&server, "wf_run_foreign_conv").await;
    let (_stub, my_model_id) = stub_model_for(&server, &runner.user_id).await;

    // A conversation owned by a DIFFERENT user.
    let victim = workflow_user(&server, "wf_conv_victim").await;
    let (_vstub, victim_conv_id) =
        stub_conversation(&server, &victim.user_id, &victim.token).await;

    let wf = import_dev_workflow(&server, &runner.token, "foreign-conv", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let resp = post_run(
        &server,
        &runner.token,
        wf_id,
        json!({
            "inputs": { "topic": "t" },
            "model_id": my_model_id.to_string(),
            "conversation_id": victim_conv_id.to_string(),
        }),
    )
    .await;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        status, 404,
        "a foreign conversation_id must be rejected (not started): {body}"
    );
}

#[tokio::test]
async fn conversation_run_still_works_as_a_regression() {
    // The pre-existing conversation path (no explicit model_id) still resolves
    // the model from the conversation snapshot.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_run_conv").await;
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let wf = import_dev_workflow(&server, &user.token, "model-run-conv", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "t" },
            "conversation_id": conv_id.to_string(),
            "mocks": { "gen": "mocked" }
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    let pool = db_pool(&server).await;
    let conv_on_row: Option<Uuid> =
        sqlx::query_scalar("SELECT conversation_id FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("run row");
    assert_eq!(
        conv_on_row,
        Some(conv_id),
        "conversation run records the conversation_id"
    );
    pool.close().await;

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "conversation run completes: {final_run}");
}

#[tokio::test]
async fn dispatched_llm_step_accrues_token_cost_on_run_row() {
    // Cost tracking (audit all-75e02bfb7833): a workflow whose `llm` step is
    // NOT mocked actually dispatches to the model, and the step's real token
    // usage is accumulated onto the run row's `total_tokens` cost field (the
    // `persist_step_meta` `total_tokens = total_tokens + $4` accumulator fed by
    // the runner's `tokens_used`).
    //
    // Every other run test mocks all llm steps via the `mocks` map, which
    // short-circuits dispatch (`run_mock_step`) and accrues ZERO tokens; and
    // `status_machine::persist_step_meta_merges_steps_and_accumulates_tokens`
    // only drives the repository fn with SYNTHETIC deltas. So the full
    // runner → dispatch → `tokens_used` → run-row `total_tokens` path — i.e.
    // that the cost field is populated from real step usage — was untested.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_cost_tokens").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let wf = import_dev_workflow(&server, &user.token, "cost-tracking", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    // NO `mocks` map → the sole `gen` llm step really dispatches to the stub
    // model, which returns a `usage` payload (total_tokens > 0). A mocked step
    // would short-circuit dispatch and accrue zero, hiding the cost path.
    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "topic": "workflow cost tracking" },
            "model_id": model_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();

    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "dispatched (un-mocked) run completes: {final_run}"
    );

    // The run row's cost field must be populated from the real step usage —
    // a strictly-positive token count proves the runner accrued the dispatched
    // step's usage (not the 0 a mocked/short-circuited step would leave).
    let pool = db_pool(&server).await;
    let total_tokens: i64 =
        sqlx::query_scalar("SELECT total_tokens FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("read run-row total_tokens");
    pool.close().await;
    assert!(
        total_tokens > 0,
        "a dispatched llm step must accrue token cost on the run row \
         (got total_tokens={total_tokens})"
    );
}

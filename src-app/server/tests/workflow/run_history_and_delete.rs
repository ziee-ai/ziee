//! A4 (run history) + A5 (delete run) end-to-end.
//!
//! Plan Part-B matrix:
//!   5. `GET /workflows/{id}/runs` — owner-scoped, newest-first, correct
//!      `invocation_source`; a cross-user run-level operation is forbidden.
//!   6. `DELETE /workflow-runs/{id}` — no-conversation → run-created files +
//!      blobs gone, a referenced `is_saved:true` file KEPT; with-conversation →
//!      files kept (`workflow_run_id` → NULL); terminal-only; cross-user 403.
//!
//! The runs that persist a `created_by="workflow"` file are driven through the
//! real `tool`-step → `resource_link is_saved:false` path (the
//! `MockMcpServer`'s byte route), so the durable artifact is produced exactly
//! as in production. Runs whose history is merely listed use a mock-short-
//! circuited `llm` step (no tokens).

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{
    SIMPLE_OK_YAML, count_files_for_run, db_pool, import_dev_workflow, plain_server,
    plain_server_allow_loopback, poll_run,
    register_mock_as_user_server, run_workflow, stub_conversation, stub_model_for, workflow_user,
};
use crate::common::test_helpers::create_user_with_permissions;
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};

/// A user holding the workflow perms + MCP + file perms needed to drive a
/// tool-step run that persists an artifact AND to pre-upload a referenced file.
async fn run_user(
    server: &crate::common::TestServer,
    name: &str,
) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "mcp_servers::create",
            "mcp_servers::read",
            "files::upload",
            "files::read",
        ],
    )
    .await
}

/// Single-step `tool` workflow that returns BOTH an `is_saved:false`
/// resource_link (persisted, run-owned) AND an `is_saved:true` resource_link
/// (referenced existing file). Returns the workflow id.
async fn import_artifact_tool_workflow(
    server: &crate::common::TestServer,
    token: &str,
    slug: &str,
    server_name: &str,
) -> String {
    let yaml = format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: call
    kind: tool
    server: {server_name}
    tool: produce_files
    arguments: {{}}
outputs:
  - name: result
    from: "{{{{ call.output }}}}"
    expose: full
"#
    );
    let wf = import_dev_workflow(server, token, slug, &yaml).await;
    wf["id"].as_str().expect("workflow id").to_string()
}

/// Upload a tiny file and return its id (used as the `is_saved:true` reference).
async fn upload_file(
    server: &crate::common::TestServer,
    token: &str,
    name: &str,
    bytes: &[u8],
) -> String {
    let resp = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(bytes.to_vec())
                    .file_name(name.to_string())
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .expect("upload");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse upload");
    assert_eq!(status, 201, "upload {name}: {body}");
    body["id"].as_str().expect("file id").to_string()
}

/// GET a file row by id; returns the HTTP status (200 = exists, 404 = gone).
async fn file_status(
    server: &crate::common::TestServer,
    token: &str,
    file_id: &str,
) -> reqwest::StatusCode {
    reqwest::Client::new()
        .get(server.api_url(&format!("/files/{file_id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("get file")
        .status()
}

/// Program the mock to return one `is_saved:false` link (byte route) + one
/// `is_saved:true` link (referencing `existing_id`).
fn program_two_links(mock: &MockMcpServer, existing_id: &str) {
    mock.on_download("made.csv", "text/csv", b"x,y\n1,2\n");
    let dl = mock.download_url("made.csv");
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "produced" },
                {
                    "type": "resource_link",
                    "uri": dl,
                    "name": "made.csv",
                    "mimeType": "text/csv",
                    "is_saved": false,
                },
                {
                    "type": "resource_link",
                    "uri": format!("/api/files/{existing_id}/download"),
                    "name": "ref.txt",
                    "mimeType": "text/plain",
                    "is_saved": true,
                    "file_id": existing_id,
                }
            ],
            "isError": false,
        })),
    );
}

// ── A4 run history ───────────────────────────────────────────────────────────

#[tokio::test]
async fn run_history_lists_manual_and_conversation_runs_owner_scoped() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_hist_owner").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let wf = import_dev_workflow(&server, &user.token, "hist-wf", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();

    // Run 1: standalone (manual).
    let r1 = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": { "topic": "a" }, "model_id": model_id.to_string(), "mocks": { "gen": "x" } }),
    )
    .await;
    let r1_id = Uuid::parse_str(r1["run_id"].as_str().unwrap()).unwrap();
    poll_run(&server, &user.token, r1_id).await;

    // Run 2: conversation-bound.
    let (_stub2, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;
    let r2 = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": { "topic": "b" }, "conversation_id": conv_id.to_string(), "mocks": { "gen": "y" } }),
    )
    .await;
    let r2_id = Uuid::parse_str(r2["run_id"].as_str().unwrap()).unwrap();
    poll_run(&server, &user.token, r2_id).await;

    // NOTE: the REST `/run` path is ALWAYS `invocation_source='manual'` — it's a
    // manual launch even when it targets a conversation. The `'conversation'`
    // source is set only by the `workflow_mcp` (LLM-invoked-as-tool) path, which
    // is heavy to stand up here. Simulate it with a direct update so this test —
    // whose focus is the LISTING endpoint (owner-scope, newest-first, source
    // rendering) — exercises BOTH source badges. (The workflow_mcp path that
    // actually stamps 'conversation' is covered by the workflow_mcp tests.)
    {
        let pool = db_pool(&server).await;
        sqlx::query("UPDATE workflow_runs SET invocation_source = 'conversation' WHERE id = $1")
            .bind(r2_id)
            .execute(&pool)
            .await
            .expect("simulate conversation-sourced run");
        pool.close().await;
    }

    // List → both runs, newest-first, correct invocation_source.
    let list: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/{wf_id}/runs")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list runs")
        .json()
        .await
        .expect("parse runs");
    let runs = list["runs"].as_array().expect("runs array");
    assert!(runs.len() >= 2, "both runs listed: {list}");
    // Newest-first: run 2 (the later conversation run) precedes run 1.
    let pos_r1 = runs.iter().position(|r| r["id"] == json!(r1_id.to_string()));
    let pos_r2 = runs.iter().position(|r| r["id"] == json!(r2_id.to_string()));
    assert!(pos_r1.is_some() && pos_r2.is_some(), "both runs present: {list}");
    assert!(pos_r2.unwrap() < pos_r1.unwrap(), "newest-first ordering: {list}");
    // invocation_source per row.
    let row_r1 = &runs[pos_r1.unwrap()];
    let row_r2 = &runs[pos_r2.unwrap()];
    assert_eq!(row_r1["invocation_source"], "manual", "standalone → manual");
    assert_eq!(
        row_r2["invocation_source"], "conversation",
        "conversation run → conversation source"
    );

    // Owner scope: a SECOND user listing the SAME workflow id sees none of the
    // owner's runs (runs are per-user even for a shared workflow).
    let other = workflow_user(&server, "wf_hist_other").await;
    let other_list: Json = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/{wf_id}/runs")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("list runs other")
        .json()
        .await
        .expect("parse");
    let leaked = other_list["runs"]
        .as_array()
        .map(|a| {
            a.iter()
                .any(|r| r["id"] == json!(r1_id.to_string()) || r["id"] == json!(r2_id.to_string()))
        })
        .unwrap_or(false);
    assert!(!leaked, "another user must NOT see the owner's runs: {other_list}");
}

#[tokio::test]
async fn delete_run_cross_user_is_forbidden() {
    // A5 cross-user: deleting another user's run → 403 WORKFLOW_RUN_FORBIDDEN.
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_del_owner").await;
    let (_stub, model_id) = stub_model_for(&server, &owner.user_id).await;

    let wf = import_dev_workflow(&server, &owner.token, "del-cross", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().unwrap();
    let run = run_workflow(
        &server,
        &owner.token,
        wf_id,
        json!({ "inputs": { "topic": "t" }, "model_id": model_id.to_string(), "mocks": { "gen": "x" } }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    poll_run(&server, &owner.token, run_id).await;

    let other = create_user_with_permissions(
        &server,
        "wf_del_intruder",
        &["workflows::read", "workflows::execute"],
    )
    .await;
    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("delete cross-user");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 403, "cross-user delete must 403: {body}");
    assert!(body.contains("WORKFLOW_RUN_FORBIDDEN"), "code surfaced: {body}");
}

// ── A5 delete ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_no_conversation_run_cascades_created_files_keeps_referenced() {
    // The tool step ingests a `resource_link` served by a loopback mock, so
    // relax the resource_link external-fetch SSRF policy.
    let server = plain_server_allow_loopback().await;
    let user = run_user(&server, "wf_del_noconv").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    // Pre-existing referenced file (is_saved:true → must survive the delete).
    let ref_id = upload_file(&server, &user.token, "ref.txt", b"KEEP ME").await;

    let mock = MockMcpServer::start().await;
    program_two_links(&mock, &ref_id);
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_del_mock", &mock.base_url()).await;

    let wf_id = import_artifact_tool_workflow(&server, &user.token, "del-noconv-wf", &sname).await;
    // Standalone run (NO conversation) → the created file is owned by the run.
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completes: {final_run}");

    // Exactly one run-created file; the referenced file is NOT linked.
    let pool = db_pool(&server).await;
    assert_eq!(
        count_files_for_run(&pool, run_id, "workflow").await,
        1,
        "one run-created file before delete"
    );
    let created_id: Uuid =
        sqlx::query_scalar("SELECT file_id FROM file_workflow_runs WHERE workflow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("created file id");
    pool.close().await;

    // Both files exist before delete.
    assert_eq!(
        file_status(&server, &user.token, &created_id.to_string()).await,
        200,
        "run-created file exists before delete"
    );
    assert_eq!(
        file_status(&server, &user.token, &ref_id).await,
        200,
        "referenced file exists before delete"
    );

    // Delete the no-conversation run → 204.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete run");
    assert_eq!(del.status(), 204, "delete should 204");

    // The run-created file (+ blob) is gone; the referenced file is kept.
    assert_eq!(
        file_status(&server, &user.token, &created_id.to_string()).await,
        404,
        "run-created file must be deleted with the no-conversation run"
    );
    assert_eq!(
        file_status(&server, &user.token, &ref_id).await,
        200,
        "a referenced (is_saved:true) file must be KEPT"
    );
}

#[tokio::test]
async fn delete_with_conversation_run_keeps_files_nulling_run_link() {
    // The tool step ingests a `resource_link` served by a loopback mock, so
    // relax the resource_link external-fetch SSRF policy.
    let server = plain_server_allow_loopback().await;
    let user = run_user(&server, "wf_del_conv").await;
    // A conversation (also supplies the model snapshot).
    let (_stub, conv_id) = stub_conversation(&server, &user.user_id, &user.token).await;

    let mock = MockMcpServer::start().await;
    // No referenced file needed; just one created file.
    mock.on_download("conv.csv", "text/csv", b"a,b\n3,4\n");
    let dl = mock.download_url("conv.csv");
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "produced" },
                {
                    "type": "resource_link",
                    "uri": dl,
                    "name": "conv.csv",
                    "mimeType": "text/csv",
                    "is_saved": false,
                }
            ],
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_del_conv_mock", &mock.base_url())
            .await;

    let wf_id = import_artifact_tool_workflow(&server, &user.token, "del-conv-wf", &sname).await;
    // Conversation-bound run → the file belongs to the chat context, kept on delete.
    let run = run_workflow(
        &server,
        &user.token,
        &wf_id,
        json!({ "inputs": {}, "conversation_id": conv_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completes: {final_run}");

    let pool = db_pool(&server).await;
    let created_id: Uuid =
        sqlx::query_scalar("SELECT file_id FROM file_workflow_runs WHERE workflow_run_id = $1")
            .bind(run_id)
            .fetch_one(&pool)
            .await
            .expect("created file id");
    pool.close().await;

    // Delete the conversation run → 204.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete run");
    assert_eq!(del.status(), 204, "delete should 204");

    // The file is KEPT (conversation context); its run link is removed — the
    // `file_workflow_runs` join row CASCADE-deletes with the run (chunk
    // `ziee-file`: replaces the former `workflow_run_id` FK ON DELETE SET NULL).
    assert_eq!(
        file_status(&server, &user.token, &created_id.to_string()).await,
        200,
        "a conversation-run's file must be KEPT after delete"
    );
    let pool = db_pool(&server).await;
    let link: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM file_workflow_runs WHERE file_id = $1")
            .bind(created_id)
            .fetch_optional(&pool)
            .await
            .expect("read run link");
    assert_eq!(link, None, "run link must be removed (join row cascades) on run delete");
    pool.close().await;
}

#[tokio::test]
async fn delete_non_terminal_run_is_rejected() {
    // A5 terminal-only: a non-terminal run can't be deleted (409).
    // We insert a `running` run row directly so the guard is exercised
    // deterministically (no race with a fast-completing mock run).
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_del_nonterm").await;
    let wf = import_dev_workflow(&server, &user.token, "del-nonterm", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let pool = db_pool(&server).await;
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, 'running') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&pool)
    .await
    .expect("insert running run");
    pool.close().await;

    let resp = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete non-terminal");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(status, 409, "deleting a non-terminal run must 409: {body}");
    assert!(body.contains("WORKFLOW_RUN_NOT_TERMINAL"), "code surfaced: {body}");
}

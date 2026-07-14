//! A6 — the generic `kind: tool` step, driven end-to-end through the real
//! path: `POST /run` (standalone, with `model_id`) → runner → `ToolDispatcher`
//! → a USER MCP server (the in-process `MockMcpServer`) → result capture into
//! `outputs/<step>.json`.
//!
//! The mock's `tools/call` is programmed to return any result shape
//! (`structuredContent`, text, `isError`, a `resource_link`), and it records
//! the args it received (for the arg-flow asserts) + serves bytes at a
//! `/download/<name>` route (for the `is_saved:false` fetch).
//!
//! Plan Part-B Tier-2 matrix items implemented here:
//!   1. JSON `structuredContent` → `outputs/<step>.json`; text block; `isError`
//!      → run failed; `resource_link is_saved:false` → a `files` row
//!      `created_by="workflow"` + `workflow_run_id` set + blob; `is_saved:true`
//!      → NO duplicate file, `workflow_run_id` NOT set.
//!   2. arg flow — input → `{query, max_results:<number>}` (number preserved);
//!      whole-value `{{ A.output.items }}` → a real JSON array.
//!   3/6. access — a server not in the user's set → run fails
//!      `WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE`.
//!
//! Every `llm`-free workflow here snapshots a stub model at run start (the
//! runner always snapshots), so the run needs an accessible `model_id` even
//! though no token is ever spent.

use serde_json::{Value as Json, json};
use sqlx::Row;
use uuid::Uuid;

use super::{
    count_files_for_run, db_pool, import_dev_workflow, poll_run, register_mock_as_user_server,
    run_workflow, stub_model_for, workflow_tool_user,
};
use crate::mcp::fixtures::mock_mcp_server::{MockMcpServer, MockResponse};

/// Build a single-step `tool` workflow whose step calls `server: <server_name>`,
/// `tool: <tool>`, with the given `arguments` JSON, and exposes the step output
/// `full`. The `$schema` line mirrors the other fixtures.
fn tool_workflow_yaml(server_name: &str, tool: &str, arguments: &Json) -> String {
    // Render the arguments block as inline YAML-compatible JSON (YAML is a JSON
    // superset, so a JSON object is valid as the `arguments:` mapping value).
    let args_json = serde_json::to_string(arguments).expect("serialize arguments");
    format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    description: "subject"
    required: false
steps:
  - id: call
    kind: tool
    server: {server_name}
    tool: {tool}
    arguments: {args_json}
outputs:
  - name: result
    from: "{{{{ call.output }}}}"
    expose: full
"#
    )
}

/// Read the `tool`-step output file via the per-step output endpoint, returning
/// the raw text. The runner writes `outputs/<step>.json` for a tool step.
async fn read_step_output(
    server: &crate::common::TestServer,
    token: &str,
    run_id: Uuid,
    step: &str,
) -> (reqwest::StatusCode, String) {
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!("/workflow-runs/{run_id}/output/{step}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("read step output");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

// ── 1. Result shapes ─────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_step_json_structured_content_becomes_output() {
    // structuredContent → `outputs/<step>.json` carries the typed JSON.
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_json").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "see structured" }],
            "structuredContent": { "results": [{ "title": "Alpha" }, { "title": "Beta" }] },
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_mock_json", &mock.base_url()).await;

    let yaml = tool_workflow_yaml(&sname, "search", &json!({ "q": "x" }));
    let wf = import_dev_workflow(&server, &user.token, "tool-json", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "tool step (structuredContent) should complete: {final_run}"
    );

    let (status, body) = read_step_output(&server, &user.token, run_id, "call").await;
    assert_eq!(status, 200, "output endpoint 200: {body}");
    let out: Json = serde_json::from_str(&body).expect("output is JSON");
    assert_eq!(
        out["results"][0]["title"], "Alpha",
        "structuredContent is the step output: {out}"
    );
}

#[tokio::test]
async fn tool_step_text_block_becomes_output() {
    // No structuredContent → the concatenated text blocks become the output.
    // A plain (non-JSON) string lands as text.
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_text").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "TOOL_TEXT_MARKER hello" }],
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_mock_text", &mock.base_url()).await;

    let yaml = tool_workflow_yaml(&sname, "echo", &json!({}));
    let wf = import_dev_workflow(&server, &user.token, "tool-text", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "tool step (text) should complete: {final_run}"
    );

    let (status, body) = read_step_output(&server, &user.token, run_id, "call").await;
    assert_eq!(status, 200, "output endpoint 200: {body}");
    assert!(
        body.contains("TOOL_TEXT_MARKER"),
        "text block is the step output: {body}"
    );
}

#[tokio::test]
async fn tool_step_is_error_fails_the_run() {
    // A tool result with `isError: true` → the step fails → run `status=failed`.
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_err").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "the tool blew up" }],
            "isError": true,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_mock_err", &mock.base_url()).await;

    let yaml = tool_workflow_yaml(&sname, "boom", &json!({}));
    let wf = import_dev_workflow(&server, &user.token, "tool-err", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "failed",
        "tool step (isError) must fail the run: {final_run}"
    );
}

#[tokio::test]
async fn tool_step_resource_link_is_saved_false_persists_a_workflow_file() {
    // A `resource_link is_saved:false` whose `uri` points at the mock's byte
    // route → the dispatcher fetches the bytes over HTTP and `persist_links`
    // ingests them: a `files` row `created_by="workflow"` + `workflow_run_id`
    // set, the blob stored, and the step output carries `files[0].{file_id}`.
    // The resource_link points at a loopback mock download server; relax the
    // resource_link external-fetch SSRF policy so it's reachable in tests.
    let server = crate::common::TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: vec![("MCP_RESOURCE_LINK_ALLOW_LOOPBACK".into(), "1".into())],
        ..Default::default()
    })
    .await;
    let user = workflow_tool_user(&server, "wf_tool_rl_unsaved").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    mock.on_download("chart.csv", "text/csv", b"col_a,col_b\n1,2\n");
    let dl_url = mock.download_url("chart.csv");
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                { "type": "text", "text": "produced a chart" },
                {
                    "type": "resource_link",
                    "uri": dl_url,
                    "name": "chart.csv",
                    "mimeType": "text/csv",
                    "is_saved": false,
                }
            ],
            "isError": false,
        })),
    );
    let (_sid, sname) = register_mock_as_user_server(
        &server,
        &user.token,
        "wf_mock_rl_unsaved",
        &mock.base_url(),
    )
    .await;

    let yaml = tool_workflow_yaml(&sname, "make_chart", &json!({}));
    let wf = import_dev_workflow(&server, &user.token, "tool-rl-unsaved", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "tool step (resource_link) should complete: {final_run}"
    );

    // A `files` row was created for the run, authored by the workflow.
    let pool = db_pool(&server).await;
    assert_eq!(
        count_files_for_run(&pool, run_id, "workflow").await,
        1,
        "exactly one workflow-authored file is linked to the run"
    );
    // The blob exists: the file is downloadable via the file-store REST path,
    // and the bytes round-trip.
    // The file<->run link lives in the `file_workflow_runs` join table
    // (chunk `ziee-file`: the store carries no run column).
    let file_row = sqlx::query(
        "SELECT f.id, f.created_by, fwr.workflow_run_id \
         FROM files f JOIN file_workflow_runs fwr ON fwr.file_id = f.id \
         WHERE fwr.workflow_run_id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("file row");
    let file_id: Uuid = file_row.get("id");
    assert_eq!(file_row.get::<String, _>("created_by"), "workflow");
    assert_eq!(
        file_row.get::<Option<Uuid>, _>("workflow_run_id"),
        Some(run_id),
        "the run-created file links to the run"
    );
    pool.close().await;

    // The step output references the persisted file by id (downstream steps can
    // read `{{ call.output.files[0].file_id }}`).
    let (status, body) = read_step_output(&server, &user.token, run_id, "call").await;
    assert_eq!(status, 200, "output endpoint 200: {body}");
    let out: Json = serde_json::from_str(&body).expect("output JSON");
    assert_eq!(
        out["files"][0]["file_id"], json!(file_id.to_string()),
        "step output carries the persisted file id: {out}"
    );
    assert!(
        !body.contains("ziee://") && !body.contains("/download/chart.csv"),
        "no host path / raw fetch URL leaks into the step output: {body}"
    );
}

#[tokio::test]
async fn tool_step_resource_link_is_saved_true_references_without_duplicating() {
    // A pre-existing file in the store, surfaced via a `resource_link
    // is_saved:true` carrying its download URL → NO new `files` row, and the
    // existing file's `workflow_run_id` stays NULL (the run only references it).
    let server = crate::common::TestServer::start().await;
    // `files::upload` so the test can pre-create the referenced file.
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "wf_tool_rl_saved",
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
    .await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    // Pre-create a real file through the file-store REST endpoint; the upload
    // response carries the new file id directly.
    let upload = reqwest::Client::new()
        .post(server.api_url("/files/upload"))
        .header("Authorization", format!("Bearer {}", user.token))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(b"PREEXISTING".to_vec())
                    .file_name("preexisting.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .expect("upload preexisting file");
    let upload_status = upload.status();
    let uploaded: Json = upload.json().await.expect("parse upload response");
    assert_eq!(upload_status, 201, "upload preexisting: {uploaded}");
    let existing_id = uploaded["id"].as_str().expect("preexisting file id").to_string();

    let mock = MockMcpServer::start().await;
    // is_saved:true links carry a download URL the dispatcher must NOT re-save.
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [
                {
                    "type": "resource_link",
                    "uri": format!("/api/files/{existing_id}/download"),
                    "name": "preexisting.txt",
                    "mimeType": "text/plain",
                    "is_saved": true,
                    "file_id": existing_id,
                }
            ],
            "isError": false,
        })),
    );
    let (_sid, sname) = register_mock_as_user_server(
        &server,
        &user.token,
        "wf_mock_rl_saved",
        &mock.base_url(),
    )
    .await;

    let yaml = tool_workflow_yaml(&sname, "attach_existing", &json!({}));
    let wf = import_dev_workflow(&server, &user.token, "tool-rl-saved", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "tool step (is_saved:true) should complete: {final_run}"
    );

    let pool = db_pool(&server).await;
    // No workflow-authored file row for this run (the referenced file is NOT
    // re-saved).
    assert_eq!(
        count_files_for_run(&pool, run_id, "workflow").await,
        0,
        "an is_saved:true link must NOT create a duplicate workflow file"
    );
    // The pre-existing file has NO run link (only referenced) — no join row.
    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT workflow_run_id FROM file_workflow_runs WHERE file_id = $1")
            .bind(Uuid::parse_str(&existing_id).unwrap())
            .fetch_optional(&pool)
            .await
            .expect("read referenced file run link");
    assert_eq!(
        linked, None,
        "a referenced (is_saved:true) file must NOT be linked to the run"
    );
    pool.close().await;
}

// ── 2. Argument flow ─────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_step_args_preserve_number_type_from_inputs() {
    // `arguments: { query, max_results }` where max_results is a whole-value
    // `{{ inputs.n }}` → the mock RECEIVES a JSON number, not a string.
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_argnum").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "ok" }],
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_mock_argnum", &mock.base_url())
            .await;

    // `inputs.q` interpolates into a string; `inputs.n` is a whole-value ref →
    // preserved as a number.
    let yaml = format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: q
    required: true
  - name: n
    required: true
steps:
  - id: call
    kind: tool
    server: {sname}
    tool: web_search
    arguments:
      query: "{{{{ inputs.q }}}}"
      max_results: "{{{{ inputs.n }}}}"
outputs:
  - name: result
    from: "{{{{ call.output }}}}"
"#
    );
    let wf = import_dev_workflow(&server, &user.token, "tool-argnum", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({
            "inputs": { "q": "quantum", "n": 7 },
            "model_id": model_id.to_string(),
        }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "arg-num run should complete: {final_run}"
    );

    // The mock saw exactly one tools/call with the typed args.
    let calls: Vec<_> = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .collect();
    assert_eq!(calls.len(), 1, "exactly one tools/call");
    let args = &calls[0].body["params"]["arguments"];
    assert_eq!(args["query"], json!("quantum"), "string arg interpolated");
    assert_eq!(
        args["max_results"],
        json!(7),
        "whole-value {{{{ inputs.n }}}} preserved the JSON NUMBER type: {args}"
    );
    assert!(
        args["max_results"].is_number(),
        "max_results must be a JSON number, not a string: {args}"
    );
}

#[tokio::test]
async fn tool_step_prior_step_whole_value_ref_is_a_real_array() {
    // A first `tool` step returns a JSON array via structuredContent; a second
    // `tool` step passes `{{ A.output.items }}` as a whole-value arg → the mock
    // RECEIVES a real JSON array (chaining + typing).
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_argarr").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    let mock = MockMcpServer::start().await;
    // First call → an object with an `items` array.
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "first" }],
            "structuredContent": { "items": ["one", "two", "three"] },
            "isError": false,
        })),
    );
    // Second call → plain ack.
    mock.on_method(
        "tools/call",
        MockResponse::JsonOk(json!({
            "content": [{ "type": "text", "text": "second" }],
            "isError": false,
        })),
    );
    let (_sid, sname) =
        register_mock_as_user_server(&server, &user.token, "wf_mock_argarr", &mock.base_url())
            .await;

    let yaml = format!(
        r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs: []
steps:
  - id: produce
    kind: tool
    server: {sname}
    tool: produce
    arguments: {{}}
  - id: consume
    kind: tool
    server: {sname}
    tool: consume
    arguments:
      payload: "{{{{ produce.output.items }}}}"
    depends_on: [produce]
outputs:
  - name: result
    from: "{{{{ consume.output }}}}"
"#
    );
    let wf = import_dev_workflow(&server, &user.token, "tool-argarr", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "arg-array run should complete: {final_run}"
    );

    // The SECOND tools/call carried the whole array (typed), not a stringified
    // JSON blob.
    let calls: Vec<_> = mock
        .received()
        .into_iter()
        .filter(|r| r.method == "tools/call")
        .collect();
    assert_eq!(calls.len(), 2, "two tools/call (produce + consume)");
    let payload = &calls[1].body["params"]["arguments"]["payload"];
    assert_eq!(
        payload,
        &json!(["one", "two", "three"]),
        "whole-value {{{{ produce.output.items }}}} arrived as a real JSON array: {payload}"
    );
    assert!(payload.is_array(), "payload must be a JSON array: {payload}");
}

// ── 3 / 6. Access ────────────────────────────────────────────────────────────

#[tokio::test]
async fn tool_step_inaccessible_server_fails_the_run() {
    // A `tool` step naming a server the user does NOT have → the run fails with
    // `WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE` (resolved at run time, not install).
    let server = crate::common::TestServer::start().await;
    let user = workflow_tool_user(&server, "wf_tool_noaccess").await;
    let (_stub, model_id) = stub_model_for(&server, &user.user_id).await;

    // Name a server that was never registered for this user.
    let yaml = tool_workflow_yaml("nonexistent_server_xyz", "search", &json!({}));
    let wf = import_dev_workflow(&server, &user.token, "tool-noaccess", &yaml).await;
    let wf_id = wf["id"].as_str().unwrap();

    let run = run_workflow(
        &server,
        &user.token,
        wf_id,
        json!({ "inputs": {}, "model_id": model_id.to_string() }),
    )
    .await;
    let run_id = Uuid::parse_str(run["run_id"].as_str().unwrap()).unwrap();
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "failed",
        "naming an inaccessible server must fail the run: {final_run}"
    );
    // The failure message surfaces the access error code (`error_message` on
    // the run row).
    let err = final_run["error_message"].as_str().unwrap_or("");
    assert!(
        err.contains("WORKFLOW_TOOL_SERVER_NOT_ACCESSIBLE")
            || err.contains("not accessible")
            || err.contains("nonexistent_server_xyz"),
        "failure should reference the inaccessible-server error: {final_run}"
    );
}

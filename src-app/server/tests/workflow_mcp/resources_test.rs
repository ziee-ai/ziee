use serde_json::Value as Json;
use serde_json::json;
use uuid::Uuid;
use crate::common::TestServer;
use crate::workflow::import_dev_workflow;
use crate::workflow::poll_run;
use super::jsonrpc;
use super::mcp_user;
use super::wf_tool_name;

/// JSON-RPC INVALID_PARAMS code (the bucket 4xx AppErrors land in via
/// `JsonRpcError::from_app_error`). Mirrors
/// `code_sandbox::types::JsonRpcError::INVALID_PARAMS`.
const JSONRPC_INVALID_PARAMS: i64 = -32602;

/// A single-step `llm` workflow that dispatches FOR REAL against the stub model
/// (NO `mock:` baked in). `log: full` makes the dispatcher capture the rendered
/// `prompt` + the `raw_output` to disk; `expose_logs: always` makes those logs
/// surfaceable over the MCP resource path. The sole output is `expose:
/// artifact`, which always surfaces as a resource in `resources/list`.
const REAL_LOGGED_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
expose_logs: always
inputs:
  - name: topic
    description: "What to summarize"
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
    log: full
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: artifact
"#;

/// Same shape but `expose_logs: never` — the confidentiality control under M6.
/// Logs are written to disk but `logs_surfaceable` returns false, so
/// `resources/read` of a log must be refused fail-closed.
const NEVER_LOGS_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
expose_logs: never
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
    log: full
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: full
"#;

/// A workflow whose sole step FAILS: the stub returns plain text
/// ("Hello from stub"), but `output_format: json` demands valid JSON → the
/// `gen` step fails ("expected JSON output, parse failed") → the run is
/// `failed`. `expose_logs: never` means the error result carries NO
/// `logs_resource` link (M6). No external MCP server needed.
const FAILING_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
expose_logs: never
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
    output_format: json
    log: full
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: full
"#;

/// Spawn a conversation-sourced run via `tools/call wf_<slug>` and return its
/// `run_id` + the formatted `CallToolResult`. The workflow dispatches for real
/// against the stub model bound to the conversation (no tokens spent — the stub
/// replies deterministically). Asserts the tool call 200s.
async fn run_via_tools_call(
    server: &TestServer,
    user: &crate::common::test_helpers::TestUser,
    slug_seed: &str,
    yaml: &str,
) -> (Uuid, Json, Uuid) {
    // Stub model + conversation so the MCP path's model snapshot succeeds.
    // `tools/call` BLOCKS until the run reaches a terminal status, so the stub
    // process is only needed for the duration of this helper — by the time we
    // return (after the 200), the run has already dispatched + completed, so
    // letting `_stub` drop here is safe (it stays alive across the await).
    let (_stub, model) = crate::chat::helpers::create_stub_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("wf-mcp resources conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();

    let wf = import_dev_workflow(server, &user.token, slug_seed, yaml).await;
    let wf_name = wf["name"].as_str().expect("workflow name");
    let leaf = wf_tool_name(wf_name);

    let resp = jsonrpc(
        server,
        &user.token,
        Some(conv_id),
        "tools/call",
        json!({ "name": leaf, "arguments": { "topic": "espresso" } }),
    )
    .await;
    assert_eq!(resp.status(), 200, "tools/call should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "tools/call had no JSON-RPC error: {body}");
    let result = body["result"].clone();
    let run_id = Uuid::parse_str(
        result["structuredContent"]["metadata"]["run_id"]
            .as_str()
            .unwrap_or_else(|| panic!("run_id in result metadata: {result}")),
    )
    .expect("run_id uuid");
    (run_id, result, conv_id)
}

// ── S5: ownership gate on resources/read ─────────────────────────────────────

#[tokio::test]
async fn resources_read_for_another_users_run_is_rejected() {
    let server = TestServer::start().await;

    // User A drives a completed run.
    let owner = mcp_user(&server, "wf_res_owner").await;
    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &owner, "s5-owner", REAL_LOGGED_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &owner.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "owner's run completed: {final_run}"
    );

    // User B (a different user, also with execute) tries to read A's run's
    // output resource. The ownership gate (`run.user_id != user_id`) refuses it.
    let attacker = mcp_user(&server, "wf_res_attacker").await;
    let uri = format!("ziee://workflow-runs/{run_id}/outputs/summary");
    let resp = jsonrpc(
        &server,
        &attacker.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    // HTTP 200 with a JSON-RPC error body (the dispatch maps every resource
    // error through `error_response(id, StatusCode::OK, …)`).
    assert_eq!(resp.status(), 200, "JSON-RPC errors ride a 200 envelope");
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["result"].is_null(),
        "a forbidden read produces no result: {body}"
    );
    let err = &body["error"];
    assert_eq!(
        err["code"].as_i64(),
        Some(JSONRPC_INVALID_PARAMS),
        "403 ownership refusal is classified as invalid_params (-32602): {body}"
    );
    // Display writes only the AppError message (not the error_code), so assert
    // the message text the handler produces.
    let msg = err["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("owned by another user"),
        "the refusal surfaces the cross-owner reason: {body}"
    );
}

// ── M4: resources/list enumerates a completed run's resource URIs ────────────

#[tokio::test]
async fn resources_list_enumerates_completed_run_output_resource() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_list").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m4-list", REAL_LOGGED_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    let resp = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    assert_eq!(resp.status(), 200, "resources/list should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "resources/list had no error: {body}");
    let resources = body["result"]["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("resources array: {body}"));

    // The `expose: artifact` output `summary` surfaces as a resource whose URI
    // is `ziee://workflow-runs/<run>/outputs/summary`.
    let expected_uri = format!("ziee://workflow-runs/{run_id}/outputs/summary");
    let found = resources
        .iter()
        .find(|r| r["uri"].as_str() == Some(expected_uri.as_str()));
    assert!(
        found.is_some(),
        "the completed run's `summary` output resource must be listed ({expected_uri}); got: {body}"
    );
    let res = found.unwrap();
    assert_eq!(
        res["name"].as_str(),
        Some("summary"),
        "the listed output resource is named after the output: {res}"
    );
    // Every listed resource is a `ziee://workflow-runs/...` resource (the only
    // scheme this server enumerates), scoping the listing to the user's runs.
    assert!(
        resources.iter().all(|r| r["uri"]
            .as_str()
            .map(|u| u.starts_with("ziee://workflow-runs/"))
            .unwrap_or(false)),
        "all listed resources use the workflow-runs scheme: {body}"
    );
}

// ── M5: resources/read over the MCP path ─────────────────────────────────────

#[tokio::test]
async fn resources_read_returns_captured_log_body() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_readlog").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m5-readlog", REAL_LOGGED_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    // The dispatcher wrote `logs/gen/raw_output` to disk (log: full). Read it
    // back over the MCP resource path; `expose_logs: always` makes it
    // surfaceable. The stub replies deterministically "Hello from stub".
    let uri = format!("ziee://workflow-runs/{run_id}/logs/gen/raw_output");
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "resources/read should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "log read had no error: {body}");
    let content = &body["result"]["contents"][0];
    assert_eq!(
        content["uri"].as_str(),
        Some(uri.as_str()),
        "the content echoes the requested uri: {body}"
    );
    // raw_output is text/plain → returned as `text`.
    let text = content["text"]
        .as_str()
        .unwrap_or_else(|| panic!("raw_output log returns a text body: {body}"));
    assert!(
        text.contains("Hello from stub"),
        "the captured raw_output log carries the stub model's reply: {body}"
    );
}

#[tokio::test]
async fn resources_read_rejects_unknown_log_kind() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_badkind").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m5-badkind", REAL_LOGGED_WORKFLOW_YAML).await;
    poll_run(&server, &user.token, run_id).await;

    // `bogus` is not in `read_log`'s LOG_KINDS whitelist → WORKFLOW_LOG_BAD_KIND.
    let uri = format!("ziee://workflow-runs/{run_id}/logs/gen/bogus");
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "JSON-RPC error rides a 200 envelope");
    let body: Json = resp.json().await.unwrap();
    assert!(body["result"].is_null(), "a bad kind produces no result: {body}");
    assert_eq!(
        body["error"]["code"].as_i64(),
        Some(JSONRPC_INVALID_PARAMS),
        "an unknown log kind is a client (invalid_params) error: {body}"
    );
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("not recognized") || msg.contains("bogus"),
        "the error names the unrecognized log kind: {body}"
    );
}

#[tokio::test]
async fn resources_read_rejects_dotdot_traversal_uri() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_dotdot").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m5-dotdot", REAL_LOGGED_WORKFLOW_YAML).await;
    poll_run(&server, &user.token, run_id).await;

    // A `..` step-id segment in a log URI must be rejected by parse_uri's
    // sanitize_uri_component (WORKFLOW_URI_INVALID), before any disk access.
    let uri = format!("ziee://workflow-runs/{run_id}/logs/../raw_output");
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "JSON-RPC error rides a 200 envelope");
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["result"].is_null(),
        "a traversal URI produces no result: {body}"
    );
    assert_eq!(
        body["error"]["code"].as_i64(),
        Some(JSONRPC_INVALID_PARAMS),
        "a traversal URI is a client (invalid_params) error: {body}"
    );
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("safe path segment") || msg.contains("WORKFLOW_URI_INVALID") || msg.contains("'..'"),
        "the error references the unsafe path segment: {body}"
    );
}

// ── M6: logs_surfaceable fail-closed + failed-run error formatting ───────────

#[tokio::test]
async fn resources_read_log_is_refused_when_expose_logs_never() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_never").await;

    // A completed run whose def has `expose_logs: never`. The dispatcher STILL
    // wrote `logs/gen/raw_output` to disk (log: full), but `logs_surfaceable`
    // returns false → resources/read must refuse it fail-closed, even for the
    // OWNER. The refusal is a 403 WORKFLOW_LOG_NOT_EXPOSED ("excluded by the
    // workflow's expose_logs setting").
    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m6-never", NEVER_LOGS_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    let uri = format!("ziee://workflow-runs/{run_id}/logs/gen/raw_output");
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "JSON-RPC error rides a 200 envelope");
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["result"].is_null(),
        "expose_logs:never must NOT return a log body even to the owner: {body}"
    );
    // 403 FORBIDDEN maps to invalid_params via from_app_error's 4xx arm.
    assert_eq!(
        body["error"]["code"].as_i64(),
        Some(JSONRPC_INVALID_PARAMS),
        "a fail-closed log refusal is a client (invalid_params) error: {body}"
    );
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("expose_logs") || msg.contains("excluded"),
        "the refusal cites the expose_logs:never gate: {body}"
    );
}

#[tokio::test]
async fn failed_run_tools_call_surfaces_error_without_logs_resource() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_failed").await;

    // The sole `gen` step fails (text reply but output_format:json). `tools/call`
    // returns `build_error_result` (isError:true), and with expose_logs:never
    // the result carries NO `logs_resource` link.
    let (run_id, result, _conv) =
        run_via_tools_call(&server, &user, "m6-failed", FAILING_WORKFLOW_YAML).await;

    // The run is terminally failed.
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "failed",
        "the json-parse failure fails the run: {final_run}"
    );

    // The formatted CallToolResult is an error result.
    assert_eq!(
        result["isError"], json!(true),
        "a failed run's tool result is an error result: {result}"
    );
    // build_error_result inlines the error into the text body.
    let text = result["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("error result text body: {result}"));
    let parsed: Json = serde_json::from_str(text).unwrap_or_else(|_| json!({}));
    assert!(
        parsed["error"].is_string(),
        "the error result carries a human-readable error: {text}"
    );
    assert_eq!(
        parsed["metadata"]["status"].as_str(),
        Some("failed"),
        "the error result metadata reflects the failed status: {text}"
    );
    // Reality note: `build_error_result` keys `failed_step` off `run.current_step`,
    // which is only set by `persist_step_meta` on a COMPLETED step. The sole
    // `gen` step fails on the FIRST step, so `current_step` stays NULL and the
    // error body carries no `failed_step` key — so we don't assert on it here.
    //
    // M6: expose_logs:never → NO logs_resource link in the error body (the
    // `failed_step` gate AND the `logs_surfaceable` gate both keep it off).
    assert!(
        parsed.get("logs_resource").is_none()
            && result["structuredContent"].get("logs_resource").is_none(),
        "expose_logs:never must NOT attach a logs_resource link to the error result: {result}"
    );
}

// ── bd04131ef657: resources/list + read for a cleaned-up (deleted) run ───────

/// After a run is DELETED (the "expired / cleaned-up run" case), its resources
/// must disappear from `resources/list` and a `resources/read` of one of its
/// URIs must fail GRACEFULLY (a JSON-RPC error, never a panic / 500). Exercises
/// resources.rs:95-182 against a run whose metadata no longer exists.
#[tokio::test]
async fn resources_list_omits_deleted_run_and_read_errors() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_deleted").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "bd04-deleted", REAL_LOGGED_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    let output_uri = format!("ziee://workflow-runs/{run_id}/outputs/summary");

    // Precondition: the completed run's output is listed.
    let resp = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["uri"].as_str() == Some(output_uri.as_str())),
        "precondition: run output must be listed before deletion: {body}"
    );

    // Clean up the run (terminal → deletable).
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/workflow-runs/{run_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("delete run");
    assert!(
        del.status().is_success(),
        "deleting a terminal run must succeed, got {}",
        del.status()
    );

    // resources/list no longer enumerates the deleted run's resources.
    let resp = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    assert_eq!(resp.status(), 200, "resources/list still 200 after cleanup");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "resources/list must not error post-cleanup: {body}");
    assert!(
        !body["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["uri"].as_str().map(|u| u.contains(&run_id.to_string())).unwrap_or(false)),
        "no resource of the deleted run may be listed: {body}"
    );

    // resources/read of the now-gone resource fails gracefully (JSON-RPC error).
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": output_uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "JSON-RPC transport still 200");
    let body: Json = resp.json().await.unwrap();
    assert!(
        body["error"].is_object() || body["result"]["isError"] == json!(true),
        "reading a deleted run's resource must surface an error, not crash: {body}"
    );
}

/// Same shape as `REAL_LOGGED_WORKFLOW_YAML` but the sole `expose: artifact`
/// output declares a BINARY `mime_type` (`application/octet-stream`, which is
/// not in `is_text_mime`'s allow-list). So `resources/read` of the output must
/// take the base64 BLOB branch (resources.rs:296-299) rather than the text
/// branch — even though the underlying bytes are the stub model's plain-text
/// reply. The declared mime_type wins over `parsed_as`.
const BINARY_OUTPUT_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
expose_logs: always
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "summarize {{ inputs.topic }}"
    log: full
outputs:
  - name: summary
    from: "{{ gen.output }}"
    expose: artifact
    mime_type: application/octet-stream
"#;

/// M5 (binary branch): an output declaring a non-text `mime_type` must be
/// returned as a base64 `blob`, NOT a `text` body — and the base64 must decode
/// back to the original bytes. Exercises resources.rs:296-299 (the
/// `else { … encode … "blob" }` arm), which every other resources/read test
/// (all of which hit text mimes) leaves uncovered.
#[tokio::test]
async fn resources_read_returns_binary_output_as_base64_blob() {
    use base64::Engine as _;

    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_binblob").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "m5-binblob", BINARY_OUTPUT_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    // The `summary` output bytes are the stub model's reply, but the output
    // declares `mime_type: application/octet-stream` → resources/read must take
    // the binary blob branch.
    let uri = format!("ziee://workflow-runs/{run_id}/outputs/summary");
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "resources/read",
        json!({ "uri": uri }),
    )
    .await;
    assert_eq!(resp.status(), 200, "resources/read should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "binary read had no error: {body}");
    let content = &body["result"]["contents"][0];
    assert_eq!(
        content["uri"].as_str(),
        Some(uri.as_str()),
        "the content echoes the requested uri: {body}"
    );
    assert_eq!(
        content["mimeType"].as_str(),
        Some("application/octet-stream"),
        "the declared binary mime_type is surfaced: {body}"
    );
    // The binary branch returns `blob` (base64), NEVER `text`.
    assert!(
        content.get("text").is_none(),
        "a binary-mime resource must NOT be returned as a text body: {body}"
    );
    let b64 = content["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("binary output returns a base64 `blob` field: {body}"));
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("the blob must be valid base64");
    // Round-trips back to the original output bytes (the stub's reply).
    let decoded_text = String::from_utf8(decoded).expect("stub reply bytes are utf-8");
    assert!(
        decoded_text.contains("Hello from stub"),
        "the decoded blob carries the original output bytes (the stub reply): got {decoded_text:?}"
    );
}

/// A minimal SANDBOX workflow (no llm/tool/elicit steps, so `tools/call` runs to
/// terminal without blocking): one `kind: sandbox` step writes an artifact and
/// emits a byte count, surfaced as an `expose: artifact` output.
const SANDBOX_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: topic
    required: true
steps:
  - id: process
    kind: sandbox
    run: >-
      echo "SANDBOX_RAN {{ inputs.topic }}" | tee artifacts/process/out.txt | wc -c
outputs:
  - name: size
    from: "{{ process.output }}"
    expose: artifact
"#;

/// f7915c78 — workflow MCP + sandbox combined over the PRODUCTION MCP path.
/// `resources_test`'s other cases run llm-only workflows (no rootfs), and
/// `workflow::real_stack` runs a sandbox workflow but via the direct runner,
/// NOT `/api/workflows/mcp`. This pins the missing combination: a real sandbox
/// step, dispatched by `tools/call wf_<slug>`, whose artifact output is then
/// enumerated through the SAME workflow_mcp `resources/list`. Rootfs-gated
/// (clean skip) exactly like every other Tier-6 sandbox test.
///
/// (Note: "memory" is not a workflow step kind, so the faithful combined
/// coverage here is workflow_mcp + sandbox; the memory built-in's own
/// recall/inject + recording paths are covered by the memory + mcp suites.)
#[tokio::test]
async fn workflow_mcp_sandbox_run_artifact_listed_over_mcp() {
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        eprintln!(
            "workflow_mcp_sandbox: skipping — sandbox backend/rootfs unavailable on this host"
        );
        return;
    };

    let user = mcp_user(&server, "wf_mcp_sandbox").await;
    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "wfmcp-sandbox", SANDBOX_WORKFLOW_YAML).await;

    let final_run = crate::workflow::poll_run(&server, &user.token, run_id).await;
    assert_eq!(
        final_run["status"], "completed",
        "the sandbox workflow run must complete: {final_run}"
    );

    // The sandbox step's `expose: artifact` output is enumerated over the same
    // workflow_mcp JSON-RPC endpoint the chat MCP client uses.
    let resp = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    assert_eq!(resp.status(), 200, "resources/list should 200");
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "resources/list had no error: {body}");
    let resources = body["result"]["resources"]
        .as_array()
        .unwrap_or_else(|| panic!("resources array: {body}"));
    let expected_uri = format!("ziee://workflow-runs/{run_id}/outputs/size");
    assert!(
        resources
            .iter()
            .any(|r| r["uri"].as_str() == Some(expected_uri.as_str())),
        "the sandbox run's `size` artifact output must be listed over workflow_mcp ({expected_uri}); got: {body}"
    );
}

/// 130d696 — await_terminal's no-progress guard (M5 crashed-runner detection):
/// a run stuck in `running` whose `updated_at` never advances (the runner task
/// died without marking it terminal) must fail the tool call rather than hang.
/// We insert a run, mark it running, then NEVER touch it (no runner), and drive
/// the real await loop with the debug-only WORKFLOW_MCP_NO_PROGRESS_SECS=1 seam
/// so the 5-minute guard reproduces in ~1-2s.
#[tokio::test]
async fn await_terminal_fails_a_stalled_run_via_no_progress_guard() {
    let server = crate::common::TestServer::start().await;
    let user = mcp_user(&server, "noprog_user").await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    let wf = crate::workflow::import_dev_workflow(
        &server,
        &user.token,
        "noprog-wf",
        REAL_LOGGED_WORKFLOW_YAML,
    )
    .await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db");

    // A run that never gets a live runner: insert + mark running, then leave it.
    let run = ziee::workflow::insert_run(
        &pool,
        ziee::workflow::CreateWorkflowRun {
            workflow_id: wf_id,
            conversation_id: None,
            user_id,
            model_id: None,
            sandbox_flavor: None,
            run_kind: "normal".into(),
            invocation_source: "manual".into(),
            inputs_json: serde_json::json!({}),
        },
    )
    .await
    .expect("insert run");
    ziee::workflow::mark_running(&pool, run.id).await.expect("mark running");

    // Shrink the no-progress limit so the guard fires in ~1-2s, not 5 minutes.
    unsafe { std::env::set_var("WORKFLOW_MCP_NO_PROGRESS_SECS", "1") };
    let result = ziee::workflow_mcp_internal::await_terminal_for_test(&pool, run.id).await;
    unsafe { std::env::remove_var("WORKFLOW_MCP_NO_PROGRESS_SECS") };
    pool.close().await;

    let err = result.expect_err("a stalled running run must fail, not hang");
    let msg = format!("{err}").to_lowercase();
    assert!(
        msg.contains("no progress") || msg.contains("crashed"),
        "the failure must cite the no-progress/crashed-runner reason; got: {err}"
    );
}

// ── resources/list gracefully skips a run whose workflow def is gone ─────────

/// `resources_list` loads each run's workflow.yaml from disk (`workflow_def_for_run`).
/// If a workflow's extracted bundle is cleaned up / unreadable, the def load
/// fails — and `resources_list` must SKIP that run (logged `continue`) and still
/// return the rest, NOT error the whole listing. The happy path is M4 above.
#[tokio::test]
async fn resources_list_skips_run_with_unreadable_workflow_def() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_res_gone").await;

    let (run_id, _result, _conv) =
        run_via_tools_call(&server, &user, "gone-def", REAL_LOGGED_WORKFLOW_YAML).await;
    let final_run = poll_run(&server, &user.token, run_id).await;
    assert_eq!(final_run["status"], "completed", "run completed: {final_run}");

    // Sanity: the run's output resource is listed while the def is readable.
    let before = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    let before_body: Json = before.json().await.unwrap();
    let expected_uri = format!("ziee://workflow-runs/{run_id}/outputs/summary");
    assert!(
        before_body["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["uri"].as_str() == Some(expected_uri.as_str())),
        "sanity: resource listed before cleanup: {before_body}"
    );

    // Simulate the run's bundle being cleaned up: point the workflow's
    // extracted_path at a nonexistent dir so the workflow.yaml read fails.
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let affected = sqlx::query("UPDATE workflows SET extracted_path = '/nonexistent/ziee-cleaned'")
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
    assert!(affected >= 1, "the run's workflow row must exist to corrupt");
    pool.close().await;

    // resources/list must still 200 with NO error, and the now-undefinable run
    // is skipped (its resource is gone) — not a crash.
    let after = jsonrpc(&server, &user.token, None, "resources/list", json!({})).await;
    assert_eq!(after.status(), 200, "resources/list still 200 after cleanup");
    let after_body: Json = after.json().await.unwrap();
    assert!(
        after_body["error"].is_null(),
        "resources/list must not error on a run with a gone def: {after_body}"
    );
    assert!(
        !after_body["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["uri"].as_str() == Some(expected_uri.as_str())),
        "the run with an unreadable def must be skipped from the listing: {after_body}"
    );
}


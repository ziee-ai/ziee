//! Tier-2 integration tests for the "run/validate/save a workflow from the
//! sandbox workspace" verbs + the workspace-save / workspace-export REST
//! endpoints.
//!
//! These cover everything that does NOT require a mounted rootfs: validation,
//! dir confinement / cross-tenant isolation, structured error surfacing, the
//! ephemeral row's exclusion from every listing, and the graduation
//! (save / export) paths. The completing-green run + real-stderr capture live
//! in the rootfs-gated Tier-3 file (`tests/workflow/workspace_run_rootfs.rs`).

use serde_json::json;
use serde_json::Value as Json;
use uuid::Uuid;

use super::{db_pool, jsonrpc, mcp_user, wf_slug_tools, wf_tool_name};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

const SANDBOX_WF: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
inputs:
  - name: name
    required: true
steps:
  - id: greet
    kind: sandbox
    run: echo "hello {{ inputs.name }}"
outputs:
  - name: greeting
    from: "{{ greet.output }}"
    expose: full
"#;

/// The per-conversation workspace root the runner reads (`workflow_workspace_root`
/// re-exported for tests). Files written here at `<conv>/<dir>/` are what the
/// server ingests.
fn workspace_dir_for(conv_id: Uuid, dir: &str) -> std::path::PathBuf {
    ziee::workflow::workflow_workspace_root()
        .join(conv_id.to_string())
        .join(dir)
}

/// Write a workflow.yaml (+ optional extra files) into a conversation's
/// workspace subdir, as if the model had authored them with the code_sandbox
/// tools.
fn author_workspace(conv_id: Uuid, dir: &str, yaml: &str, extra: &[(&str, &str)]) {
    let root = workspace_dir_for(conv_id, dir);
    std::fs::create_dir_all(&root).expect("mkdir workspace dir");
    std::fs::write(root.join("workflow.yaml"), yaml).expect("write workflow.yaml");
    for (rel, contents) in extra {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, contents).unwrap();
    }
}

/// A user + stub model + conversation so `run_from_workspace`'s model snapshot
/// succeeds. Returns `(token, user_id, conversation_id)`.
async fn user_with_conversation(server: &TestServer, name: &str) -> (String, String, Uuid) {
    let user = mcp_user(server, name).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("workspace conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    (user.token, user.user_id, conv_id)
}

// ── validate_from_workspace ───────────────────────────────────────────

#[tokio::test]
async fn t2_validate_from_workspace_valid() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_valid").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "validate_from_workspace", "arguments": { "dir": "flow" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    let result = &body["result"];
    assert_eq!(
        result["structuredContent"]["valid"],
        json!(true),
        "{result}"
    );
    assert_eq!(result["isError"], json!(false));
}

#[tokio::test]
async fn t2_validate_from_workspace_reports_parse_error_no_run_row() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_parse").await;
    author_workspace(conv, "flow", "this: : : not valid yaml\n  - broken", &[]);

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "validate_from_workspace", "arguments": { "dir": "flow" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    assert_eq!(
        body["result"]["isError"],
        json!(true),
        "parse error → isError: {body}"
    );

    // No workflow_runs row was created for a validate call.
    let pool = db_pool(&server).await;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflow_runs WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0, "validate must not create a run row");
    pool.close().await;
}

#[tokio::test]
async fn t2_validate_from_workspace_missing_dir_errors() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_nodir").await;
    // No files written.
    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "validate_from_workspace", "arguments": { "dir": "ghost" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["result"]["isError"], json!(true));
    assert_eq!(
        body["result"]["structuredContent"]["code"],
        json!("WORKFLOW_WORKSPACE_MISSING"),
        "{body}"
    );
}

// ── run_from_workspace: confinement + error surfacing ─────────────────

#[tokio::test]
async fn t2_run_from_workspace_requires_conversation() {
    let server = TestServer::start().await;
    let user = mcp_user(&server, "wf_ws_noconv").await;
    // No x-conversation-id header.
    let resp = jsonrpc(
        &server,
        &user.token,
        None,
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["result"]["isError"], json!(true));
    assert_eq!(
        body["result"]["structuredContent"]["code"],
        json!("WORKFLOW_NO_CONVERSATION"),
        "{body}"
    );
}

#[tokio::test]
async fn t2_run_from_workspace_rejects_traversal() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_trav").await;
    for bad in ["../../etc", "/etc", "a/../../b"] {
        let resp = jsonrpc(
            &server,
            &token,
            Some(conv),
            "tools/call",
            json!({ "name": "run_from_workspace", "arguments": { "dir": bad } }),
        )
        .await;
        let body: Json = resp.json().await.unwrap();
        assert_eq!(body["result"]["isError"], json!(true), "dir={bad}: {body}");
        assert_eq!(
            body["result"]["structuredContent"]["code"],
            json!("WORKFLOW_WORKSPACE_BAD_DIR"),
            "dir={bad} must be rejected as bad dir: {body}"
        );
    }
}

/// Conversation A cannot ingest a dir that only exists under conversation B's
/// workspace — the dir is always resolved against the CALLER's conversation.
#[tokio::test]
async fn t2_cross_conversation_dir_isolation() {
    let server = TestServer::start().await;
    let (token_a, _ua, conv_a) = user_with_conversation(&server, "wf_ws_iso_a").await;
    let (_token_b, _ub, conv_b) = user_with_conversation(&server, "wf_ws_iso_b").await;
    // Author a valid bundle ONLY under conv B.
    author_workspace(conv_b, "flow", SANDBOX_WF, &[]);

    // Caller A (its own conv) names "flow" — which doesn't exist under A.
    let resp = jsonrpc(
        &server,
        &token_a,
        Some(conv_a),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow" } }),
    )
    .await;
    let body: Json = resp.json().await.unwrap();
    assert_eq!(body["result"]["isError"], json!(true), "{body}");
    assert_eq!(
        body["result"]["structuredContent"]["code"],
        json!("WORKFLOW_WORKSPACE_MISSING"),
        "A must not reach B's dir: {body}"
    );
}

/// SECURITY (IDOR): caller A passing caller B's `conversation_id` must be
/// rejected on EVERY entry point — the MCP verbs (header) and the REST
/// save/export (request body/query) — so A can never read / pack / run B's
/// workspace files. B authors a real bundle so the ONLY thing standing between
/// A and B's files is the ownership gate.
#[tokio::test]
async fn t2_cross_user_conversation_ownership_is_enforced() {
    let server = TestServer::start().await;
    let (token_a, _ua, _conv_a) = user_with_conversation(&server, "wf_ws_idor_a").await;
    let (_token_b, _ub, conv_b) = user_with_conversation(&server, "wf_ws_idor_b").await;
    author_workspace(conv_b, "flow", SANDBOX_WF, &[]);

    // MCP: A supplies B's conversation id in the header.
    for verb in ["run_from_workspace", "validate_from_workspace", "save_workflow"] {
        let resp = jsonrpc(
            &server,
            &token_a,
            Some(conv_b),
            "tools/call",
            json!({ "name": verb, "arguments": { "dir": "flow" } }),
        )
        .await;
        let body: Json = resp.json().await.unwrap();
        assert_eq!(
            body["result"]["isError"],
            json!(true),
            "{verb}: A must be denied B's conversation: {body}"
        );
        // The verb must NOT have proceeded to read B's files.
        assert_ne!(
            body["result"]["structuredContent"]["valid"],
            json!(true),
            "{verb}: must not validate B's file: {body}"
        );
    }
    // No ephemeral run row was created under B's conversation by A's attempt.
    let pool = db_pool(&server).await;
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflows WHERE conversation_id = $1")
            .bind(conv_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 0, "A must not materialize an ephemeral row under B's conversation");
    pool.close().await;

    // REST save: A targets B's conversation → not-found (owner-scoped), not 201.
    let save = save_via_rest(&server, &token_a, conv_b, "flow", Some("user")).await;
    assert_eq!(save.status(), 404, "REST save cross-user must 404");

    // REST export: A targets B's conversation → not-found, not 200.
    let export = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflows/workspace-export?conversation_id={conv_b}&dir=flow"
        )))
        .header("Authorization", format!("Bearer {token_a}"))
        .send()
        .await
        .unwrap();
    assert_eq!(export.status(), 404, "REST export cross-user must 404");
}

/// A run whose sandbox step can't execute (no rootfs mounted in this tier)
/// still surfaces a STRUCTURED error naming the failed step — and creates the
/// ephemeral row, which the next test asserts is hidden from listings.
#[tokio::test]
async fn t2_run_from_workspace_error_surfaces_failed_step() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_err").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow", "inputs": { "name": "x" } } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    let result = &body["result"];
    assert_eq!(
        result["isError"],
        json!(true),
        "no rootfs → the run fails: {result}"
    );
    assert_eq!(
        result["structuredContent"]["failed_step"]["id"],
        json!("greet"),
        "the error names the failed step: {result}"
    );
}

/// After a `run_from_workspace` call the ephemeral row exists but is EXCLUDED
/// from tools/list (no `wf_<slug>`) and from the REST workflows list.
#[tokio::test]
async fn t2_ephemeral_row_absent_from_all_listings() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_hidden").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);

    // Trigger a run (creates the ephemeral row even though it fails w/o rootfs).
    let _ = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow", "inputs": { "name": "x" } } }),
    )
    .await;

    // The ephemeral row exists in the DB.
    let pool = db_pool(&server).await;
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflows WHERE ephemeral = TRUE AND conversation_id = $1",
    )
    .bind(conv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "the ephemeral workflow row was created");
    pool.close().await;

    // tools/list has NO per-workflow wf_ tool for it.
    let list = jsonrpc(&server, &token, Some(conv), "tools/list", json!({})).await;
    let list_body: Json = list.json().await.unwrap();
    assert!(
        wf_slug_tools(&list_body).is_empty(),
        "ephemeral workflow must not surface as a wf_<slug> tool: {list_body}"
    );

    // REST /workflows list omits it.
    let rest: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = rest
        .as_array()
        .or_else(|| rest["workflows"].as_array())
        .map(|a| a.iter().filter_map(|w| w["name"].as_str()).collect())
        .unwrap_or_default();
    assert!(
        !names.iter().any(|n| n.starts_with("ephemeral.")),
        "ephemeral workflow must be absent from the REST list: {rest}"
    );
}

/// Deleting the conversation CASCADE-cleans its ephemeral workflow rows
/// (migration 126's `ON DELETE CASCADE`) — an LLM-authored throwaway never
/// outlives its conversation.
#[tokio::test]
async fn t2_conversation_delete_cascades_ephemeral() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_cascade").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);
    // A run (fails w/o rootfs, but still materializes the ephemeral row).
    let _ = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow", "inputs": { "name": "x" } } }),
    )
    .await;

    let pool = db_pool(&server).await;
    let before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflows WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, 1, "the ephemeral row exists before delete");

    // Delete the conversation via REST.
    let del = reqwest::Client::new()
        .delete(server.api_url(&format!("/conversations/{conv}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert!(del.status().is_success(), "conversation delete: {}", del.status());

    let after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM workflows WHERE conversation_id = $1")
            .bind(conv)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, 0, "ephemeral rows CASCADE-deleted with the conversation");
    pool.close().await;
}

// ── save / export (graduation) ────────────────────────────────────────

async fn save_via_rest(
    server: &TestServer,
    token: &str,
    conv: Uuid,
    dir: &str,
    scope: Option<&str>,
) -> reqwest::Response {
    let mut body = json!({ "conversation_id": conv, "dir": dir });
    if let Some(s) = scope {
        body["scope"] = json!(s);
    }
    reqwest::Client::new()
        .post(server.api_url("/workflows/workspace-save"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("workspace-save")
}

#[tokio::test]
async fn t2_save_scope_user_creates_permanent_listed_workflow() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_save").await;
    author_workspace(conv, "keeper", SANDBOX_WF, &[]);

    let resp = save_via_rest(&server, &token, conv, "keeper", Some("user")).await;
    assert_eq!(resp.status(), 201, "user-scope save creates the workflow");
    let wf: Json = resp.json().await.unwrap();
    let name = wf["name"].as_str().unwrap().to_string();
    assert_eq!(wf["ephemeral"], json!(false), "saved workflow is permanent");

    // It now surfaces as a wf_<slug> tool.
    let list = jsonrpc(&server, &token, Some(conv), "tools/list", json!({})).await;
    let list_body: Json = list.json().await.unwrap();
    let leaf = wf_tool_name(&name);
    assert!(
        wf_slug_tools(&list_body).iter().any(|t| t["name"]
            .as_str()
            .map(|n| n.ends_with(&leaf))
            .unwrap_or(false)),
        "the promoted workflow is now a wf_<slug> tool: {list_body}"
    );
}

#[tokio::test]
async fn t2_save_scope_system_forbidden_for_non_admin() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_sys_denied").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);

    let resp = save_via_rest(&server, &token, conv, "flow", Some("system")).await;
    assert_eq!(
        resp.status(),
        403,
        "a non-admin cannot promote to system scope"
    );
}

#[tokio::test]
async fn t2_save_scope_system_ok_for_admin() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_ws_sys_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::execute",
        ],
    )
    .await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &admin.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        &server,
        &admin.token,
        Some(model_id),
        Some("admin conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    author_workspace(conv_id, "flow", SANDBOX_WF, &[]);

    let resp = save_via_rest(&server, &admin.token, conv_id, "flow", Some("system")).await;
    assert_eq!(resp.status(), 201, "admin may promote to system scope");
    let wf: Json = resp.json().await.unwrap();
    assert_eq!(wf["scope"], json!("system"), "{wf}");
    assert!(
        wf["owner_user_id"].is_null(),
        "system rows are unowned: {wf}"
    );
}

#[tokio::test]
async fn t2_save_confinement_rejects_traversal() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_save_trav").await;
    let resp = save_via_rest(&server, &token, conv, "../../etc", Some("user")).await;
    assert_eq!(resp.status(), 400, "save must reject a traversal dir");
}

#[tokio::test]
async fn t2_export_streams_targz_and_roundtrips() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_export").await;
    author_workspace(conv, "dl", SANDBOX_WF, &[("scripts/run.sh", "echo hi\n")]);

    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflows/workspace-export?conversation_id={conv}&dir=dl"
        )))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("workspace-export");
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/gzip"),
        "export is a gzip stream"
    );
    let disp = resp
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(disp.contains(".tar.gz"), "attachment filename: {disp}");
    let bytes = resp.bytes().await.unwrap();
    assert!(!bytes.is_empty(), "the tar.gz has content");

    // Round-trip: the exported bytes install through the existing import path.
    let part = reqwest::multipart::Part::bytes(bytes.to_vec())
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .part("bundle", part)
        .text("name", "roundtripped")
        .text("scope", "user");
    let import: Json = reqwest::Client::new()
        .post(server.api_url("/workflows/import"))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("re-import exported bundle")
        .json()
        .await
        .unwrap();
    assert!(
        import["id"].is_string(),
        "exported bundle re-installs cleanly: {import}"
    );
}

#[tokio::test]
async fn t2_export_perm_gate_403() {
    // A user stripped of workflows::execute cannot export.
    let server = TestServer::start().await;
    let stripped =
        crate::common::test_helpers::create_user_with_no_permissions(&server, "wf_ws_exp_noperm")
            .await;
    let resp = reqwest::Client::new()
        .get(server.api_url(&format!(
            "/workflows/workspace-export?conversation_id={}&dir=x",
            Uuid::new_v4()
        )))
        .header("Authorization", format!("Bearer {}", stripped.token))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

/// The `save_workflow` MCP verb promotes at user scope (parity with REST save).
#[tokio::test]
async fn t2_save_via_mcp_verb() {
    let server = TestServer::start().await;
    let (token, _uid, conv) = user_with_conversation(&server, "wf_ws_mcp_save").await;
    author_workspace(conv, "flow", SANDBOX_WF, &[]);

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "save_workflow", "arguments": { "dir": "flow", "name": "mcp-saved" } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    assert!(body["error"].is_null(), "{body}");
    let sc = &body["result"]["structuredContent"];
    assert!(
        sc["workflow_id"].is_string(),
        "save returns the new id: {body}"
    );
}

// ── Tier 3: real bwrap (rootfs-gated, self-skips without a mounted rootfs) ──
//
// These author the workflow the AUTHENTIC way — a real code_sandbox
// `execute_command` writing `~/flow/workflow.yaml` into the per-conversation
// sandbox home — then run it via `run_from_workspace`. That mirrors exactly what
// the chat model does (write files with the sandbox, then run) and sidesteps the
// multi-server `OnceCell` workspace-root timing a direct filesystem write hits.

const FAILING_SANDBOX_WF: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
sandbox:
  flavor: minimal
steps:
  - id: boom
    kind: sandbox
    run: 'echo "diagnostic detail on stderr" >&2; exit 3'
    log: full
outputs: []
"#;

/// A user with BOTH code_sandbox execute + workflow perms, a stub model, and a
/// conversation — so it can write files via the sandbox AND run_from_workspace.
async fn sandbox_workflow_user(server: &TestServer, name: &str) -> (String, Uuid) {
    let user = create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "code_sandbox::execute",
        ],
    )
    .await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("t3 workspace conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    (user.token, conv_id)
}

/// Write `~/<dir>/workflow.yaml` via a real sandbox `execute_command` (the
/// production authoring flow).
async fn author_via_sandbox(server: &TestServer, jwt: &str, conv: Uuid, dir: &str, yaml: &str) {
    let cmd = format!(
        "mkdir -p ~/{dir} && cat > ~/{dir}/workflow.yaml <<'ZIEEWFEOF'\n{yaml}\nZIEEWFEOF\necho AUTHORED"
    );
    let resp = crate::code_sandbox::harness::tool_call(
        server,
        jwt,
        conv,
        "execute_command",
        json!({ "command": cmd }),
    )
    .await;
    let stdout = resp["result"]["structuredContent"]["stdout"]
        .as_str()
        .unwrap_or_default();
    assert!(
        stdout.contains("AUTHORED"),
        "sandbox author failed: {resp:#}"
    );
}

/// End-to-end through the FULL bwrap path: author in the sandbox,
/// `run_from_workspace` ingests + runs it, real stdout is captured.
#[tokio::test]
async fn t3_real_bwrap_run_from_workspace_completes() {
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        return; // no rootfs on this host — clean skip
    };
    let (token, conv) = sandbox_workflow_user(&server, "wf_ws_t3_ok").await;
    author_via_sandbox(&server, &token, conv, "flow", SANDBOX_WF).await;

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow", "inputs": { "name": "ziee" } } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    let result = &body["result"];
    assert_eq!(
        result["isError"],
        json!(false),
        "real bwrap run completes: {result}"
    );
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("hello ziee"),
        "the sandbox stdout is captured in the result: {text}"
    );
}

/// A failing sandbox step's real stderr comes back — inline in the error AND
/// readable via the `logs_resource` handle (the debug loop).
#[tokio::test]
async fn t3_real_bwrap_error_captures_stderr() {
    let Some(server) = crate::code_sandbox::harness::enabled_test_server().await else {
        return;
    };
    let (token, conv) = sandbox_workflow_user(&server, "wf_ws_t3_err").await;
    author_via_sandbox(&server, &token, conv, "flow", FAILING_SANDBOX_WF).await;

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow" } }),
    )
    .await;
    let body: Json = resp.json().await.unwrap();
    let result = &body["result"];
    assert_eq!(
        result["isError"],
        json!(true),
        "the failing run errors: {result}"
    );
    assert_eq!(
        result["structuredContent"]["failed_step"]["id"],
        json!("boom")
    );
    // The failing step's real stderr rides back INLINE in the error (the reliable
    // debug-loop channel — `dispatch.rs` embeds the exit code + stderr), so the
    // model sees WHY it failed without a second round-trip.
    let err = result["structuredContent"]["error"].as_str().unwrap_or("");
    assert!(
        err.contains("diagnostic detail on stderr") && err.contains("exit code 3"),
        "the failed step's real stderr + exit code are inline in the error: {err}"
    );
}

// ── Tier 4: real LLM via the bridge (gated on ANTHROPIC_API_KEY) ───────
//
// An `llm`-only workflow needs no rootfs, so these run on a plain (sandbox-
// disabled) server; the workspace files are authored directly (the fallback
// workspace root is stable when the sandbox never warms). Gated on a real
// provider key — locally that's the DeepSeek/Qwen bridge
// (ANTHROPIC_API_KEY=sk-local-audit + ANTHROPIC_BASE_URL=.../v1).

const LLM_WF: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "Reply with the single word BANANA and nothing else. (topic: {{ inputs.topic }})"
outputs:
  - name: result
    from: "{{ gen.output }}"
    expose: full
"#;

/// A user with workflow + model-read perms, a REAL (bridge-backed) model, and a
/// conversation. Returns `(token, user_id, conversation_id, model_id, branch_id)`.
async fn real_model_conversation(server: &TestServer, name: &str) -> (String, Uuid, Uuid, Uuid, Uuid) {
    let user = create_user_with_permissions(
        server,
        name,
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
            "llm_models::read",
            "llm_providers::read",
        ],
    )
    .await;
    let model = crate::chat::helpers::get_or_create_test_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("t4 real-llm conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    let branch_id = Uuid::parse_str(conv["active_branch_id"].as_str().unwrap()).unwrap();
    let user_id = Uuid::parse_str(&user.user_id).unwrap();
    (user.token, user_id, conv_id, model_id, branch_id)
}

/// RELIABLE (non-agentic): `run_from_workspace` drives a real `llm` step
/// end-to-end (is_dev=false, no mocks) against the bridge model, and completes.
#[tokio::test]
async fn t4_run_from_workspace_drives_real_llm_step() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return; // no provider key / bridge — clean skip
    }
    let server = TestServer::start().await;
    let (token, _uid, conv, _model, _branch) =
        real_model_conversation(&server, "wf_ws_t4_llm").await;
    author_workspace(conv, "flow", LLM_WF, &[]);

    let resp = jsonrpc(
        &server,
        &token,
        Some(conv),
        "tools/call",
        json!({ "name": "run_from_workspace", "arguments": { "dir": "flow", "inputs": { "topic": "fruit" } } }),
    )
    .await;
    assert_eq!(resp.status(), 200);
    let body: Json = resp.json().await.unwrap();
    let result = &body["result"];
    assert_eq!(
        result["isError"],
        json!(false),
        "the real-LLM workflow completed: {result}"
    );
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(!text.is_empty(), "the llm step produced output: {result}");
}

/// Assign a built-in MCP server to the user's custom test group so the chat
/// model can see its tools (mirrors the sandbox real-LLM setup).
async fn assign_server_to_test_group(server: &TestServer, user_id: Uuid, server_id: Uuid) {
    let pool = db_pool(server).await;
    let group_id: Uuid = sqlx::query_scalar(
        "SELECT g.id FROM groups g \
         JOIN user_groups ug ON ug.group_id = g.id \
         WHERE ug.user_id = $1 AND g.is_default = false AND g.is_system = false \
           AND g.name LIKE 'test_group_%' ORDER BY g.created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("user must be in a custom test group");
    sqlx::query(
        "INSERT INTO user_group_mcp_servers (group_id, mcp_server_id, assigned_at) \
         VALUES ($1, $2, NOW()) ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(server_id)
    .execute(&pool)
    .await
    .expect("assign server to test group");
    pool.close().await;
}

/// AGENTIC: the chat model DISCOVERS + CALLS `run_from_workspace` (and then
/// `save_workflow`) on the workflow_mcp server it was given. Two explicit
/// single-tool turns (each reliable — the model is told to call the tool).
#[tokio::test]
async fn t4_llm_agentically_runs_and_saves_workflow() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return;
    }
    let server = TestServer::start().await;
    let (token, user_id, conv, model_id, branch_id) =
        real_model_conversation(&server, "wf_ws_t4_agentic").await;
    let workflow_server = ziee::workflow_mcp::workflow_mcp_server_id();
    assign_server_to_test_group(&server, user_id, workflow_server).await;
    // Auto-approve the workspace verbs at the conversation level so the model's
    // tool calls EXECUTE (they mutate, so they aren't in the read-only bypass).
    set_conversation_auto_approve(
        &server,
        &token,
        conv,
        workflow_server,
        &["run_from_workspace", "save_workflow", "validate_from_workspace"],
    )
    .await;
    author_workspace(conv, "flow", LLM_WF, &[]);

    let send = |content: String| {
        let payload = json!({
            "content": content,
            "model_id": model_id,
            "branch_id": branch_id,
            "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": workflow_server, "tools": [] } ] }
        });
        crate::chat::helpers::send_body_and_collect_events(&server, &token, conv, payload, &[])
    };

    // Turn 1 — the model DISCOVERS + CALLS run_from_workspace (auto-approved →
    // it actually runs, creating the ephemeral row).
    let events = send(
        "Use the run_from_workspace tool RIGHT NOW with dir='flow' and inputs {\"topic\":\"fruit\"}. \
         Call the tool; do not reply with plain text."
            .into(),
    )
    .await;
    assert_tool_called(&events, "run_from_workspace");

    let pool = db_pool(&server).await;
    let ran: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflows WHERE conversation_id = $1 AND ephemeral = TRUE",
    )
    .bind(conv)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(ran >= 1, "the agentic run_from_workspace materialized an ephemeral workflow");

    // Turn 2 — the model calls save_workflow → a permanent row lands.
    let events2 = send(
        "Now use the save_workflow tool with dir='flow' to save that workflow to my library. \
         Call the tool; do not reply with plain text."
            .into(),
    )
    .await;
    assert_tool_called(&events2, "save_workflow");

    let saved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM workflows WHERE owner_user_id = $1 AND ephemeral = FALSE",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(saved >= 1, "the agentic save_workflow persisted a permanent workflow row");
    pool.close().await;
}

/// Assert the model invoked `tool` — either it ran (`mcpToolStart`) or it
/// reached the approval gate naming the tool (`mcpApprovalRequired`). Both
/// prove the model DISCOVERED + CALLED the verb.
fn assert_tool_called(events: &[crate::chat::helpers::SSEEvent], tool: &str) {
    let hit = events.iter().any(|e| {
        (e.event == "mcpToolStart" || e.event == "mcpApprovalRequired")
            && e.data
                .get("tool_name")
                .and_then(|v| v.as_str())
                .map(|n| n.contains(tool))
                .unwrap_or(false)
    });
    let seen: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
    assert!(hit, "the model must call '{tool}' agentically; events: {seen:?}");
}

/// PUT conversation mcp-settings to auto-approve specific tools on a server.
async fn set_conversation_auto_approve(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    server_id: Uuid,
    tools: &[&str],
) {
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "approval_mode": "auto_approve",
            "auto_approved_tools": [ { "server_id": server_id, "tools": tools } ]
        }))
        .send()
        .await
        .expect("set mcp settings");
    assert!(resp.status().is_success(), "mcp-settings PUT failed: {}", resp.status());
}

/// PUT the conversation's global approval mode (no per-tool list).
async fn set_conversation_approval_mode(
    server: &TestServer,
    token: &str,
    conv_id: Uuid,
    mode: &str,
) {
    let resp = reqwest::Client::new()
        .put(server.api_url(&format!("/conversations/{conv_id}/mcp-settings")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "approval_mode": mode, "auto_approved_tools": [] }))
        .send()
        .await
        .expect("set mcp settings");
    assert!(resp.status().is_success(), "mcp-settings PUT failed: {}", resp.status());
}

/// The workspace verbs are NOT unconditionally approval-bypassed (they execute
/// authored code). Instead they honor the conversation's approval mode: GLOBAL
/// `auto_approve` (bypass) → they run without a prompt; the default
/// `manual_approve` → they stall at an approval gate. This locks that contract.
#[tokio::test]
async fn t4_workspace_verbs_honor_approval_mode() {
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        return;
    }
    let server = TestServer::start().await;
    let workflow_server = ziee::workflow_mcp::workflow_mcp_server_id();

    // (a) GLOBAL auto_approve, NO per-tool entry → the verb bypasses approval.
    let (tok_a, uid_a, conv_a, mid_a, br_a) =
        real_model_conversation(&server, "wf_ws_bypass_auto").await;
    assign_server_to_test_group(&server, uid_a, workflow_server).await;
    set_conversation_approval_mode(&server, &tok_a, conv_a, "auto_approve").await;
    author_workspace(conv_a, "flow", LLM_WF, &[]);
    let ev_a = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &tok_a,
        conv_a,
        json!({
            "content": "Use the run_from_workspace tool with dir='flow' and inputs {\"topic\":\"x\"}. Call the tool; no plain text.",
            "model_id": mid_a, "branch_id": br_a, "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": workflow_server, "tools": [] } ] }
        }),
        &[],
    )
    .await;
    let names_a: Vec<&str> = ev_a.iter().map(|e| e.event.as_str()).collect();
    assert!(
        ev_a.iter().any(|e| e.event == "mcpToolStart"
            && e.data["tool_name"].as_str().map(|n| n.contains("run_from_workspace")).unwrap_or(false)),
        "auto_approve: the verb must EXECUTE (mcpToolStart), events: {names_a:?}"
    );
    assert!(
        !names_a.contains(&"mcpApprovalRequired"),
        "auto_approve: the verb must NOT stall at approval, events: {names_a:?}"
    );

    // (b) manual_approve (default posture) → the verb stalls at the approval gate.
    let (tok_b, uid_b, conv_b, mid_b, br_b) =
        real_model_conversation(&server, "wf_ws_bypass_manual").await;
    assign_server_to_test_group(&server, uid_b, workflow_server).await;
    set_conversation_approval_mode(&server, &tok_b, conv_b, "manual_approve").await;
    author_workspace(conv_b, "flow", LLM_WF, &[]);
    let ev_b = crate::chat::helpers::send_body_and_collect_events(
        &server,
        &tok_b,
        conv_b,
        json!({
            "content": "Use the run_from_workspace tool with dir='flow' and inputs {\"topic\":\"x\"}. Call the tool; no plain text.",
            "model_id": mid_b, "branch_id": br_b, "enable_mcp": true,
            "mcp_config": { "mcp_servers": [ { "server_id": workflow_server, "tools": [] } ] }
        }),
        &["mcpApprovalRequired"],
    )
    .await;
    let names_b: Vec<&str> = ev_b.iter().map(|e| e.event.as_str()).collect();
    assert!(
        names_b.contains(&"mcpApprovalRequired"),
        "manual_approve: the verb must require approval, events: {names_b:?}"
    );
}

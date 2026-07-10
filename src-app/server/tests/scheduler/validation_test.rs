//! Create/update validation gates on scheduled tasks (ITEM-5/6/15/18):
//!   * TEST-8  — model_id existence + accessibility gating on create.
//!   * TEST-10 — update re-validates assistant / model / re-enable quota.
//!   * TEST-11 — the re-enable quota count excludes the row being re-enabled
//!               (no off-by-one) — real-path coverage of `count_active_for_user`.
//!   * TEST-28 — the unattended allow-list may only reference accessible servers.
//!   * TEST-33 — a workflow whose IR needs interactive input (an `elicit` step)
//!               can't be scheduled unattended.
//!
//! All drive the real REST handlers against the stub-model harness; no cosmetic
//! assertions — each exercises a distinct rejection/accept branch.

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

/// A `prompt`-target recurring task body against `model_id`.
fn prompt_body(model_id: &str, name: &str) -> Value {
    json!({
        "name": name,
        "target_kind": "prompt",
        "prompt": "Summarize today's news.",
        "model_id": model_id,
        "schedule_kind": "recurring",
        "cron_expr": "0 9 * * 1",
        "timezone": "UTC",
    })
}

async fn create_task(server: &TestServer, token: &str, body: &Value) -> reqwest::Response {
    client()
        .post(server.api_url("/scheduled-tasks"))
        .header("Authorization", format!("Bearer {token}"))
        .json(body)
        .send()
        .await
        .unwrap()
}

async fn set_cap(server: &TestServer, admin_token: &str, cap: i64) {
    let put = client()
        .put(server.api_url("/scheduler/admin-settings"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({
            "max_active_tasks_per_user": cap,
            "min_interval_seconds": 300,
            "max_consecutive_failures": 5,
            "notification_retention_days": 30,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), StatusCode::OK, "admin-settings PUT should 200");
}

// ── TEST-8 — model access gating on create ─────────────────────────────────

#[tokio::test]
async fn create_rejects_missing_and_inaccessible_model_accepts_valid() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(&server, "sched_v8", &["scheduler::use"]).await;
    // A model this user CAN access.
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // A model that EXISTS but belongs to a provider this user cannot reach
    // (granted only to `other`).
    let other = create_user_with_permissions(&server, "sched_v8_other", &["scheduler::use"]).await;
    let (_stub_other, foreign_model) =
        crate::chat::helpers::create_stub_model(&server, &other.user_id).await;
    let foreign_model_id = foreign_model["id"].as_str().unwrap();

    // Non-existent model → 404 (don't leak existence; NOT a 500 from the FK).
    let missing = Uuid::new_v4().to_string();
    let resp = create_task(&server, &user.token, &prompt_body(&missing, "missing")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "missing model_id → 404");

    // Existing-but-inaccessible model → 403.
    let resp = create_task(&server, &user.token, &prompt_body(foreign_model_id, "foreign")).await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "inaccessible model_id → 403"
    );

    // Valid accessible model → 201.
    let resp = create_task(&server, &user.token, &prompt_body(model_id, "ok")).await;
    assert_eq!(resp.status(), StatusCode::CREATED, "valid model → 201");
}

// ── TEST-10 — update re-validates referenced entities + re-enable quota ─────

#[tokio::test]
async fn update_revalidates_assistant_model_and_reenable_quota() {
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "sched_v10_admin",
        &["scheduler::admin::read", "scheduler::admin::manage"],
    )
    .await;
    // Cap = 1 so we can force a re-enable-over-quota.
    set_cap(&server, &admin.token, 1).await;

    let user = create_user_with_permissions(&server, "sched_v10", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // A model owned by a different user (inaccessible to `user`).
    let other = create_user_with_permissions(&server, "sched_v10_other", &["scheduler::use"]).await;
    let (_stub_other, foreign_model) =
        crate::chat::helpers::create_stub_model(&server, &other.user_id).await;
    let foreign_model_id = foreign_model["id"].as_str().unwrap();

    // Create task1 (enabled, active=1 at cap).
    let t1: Value = create_task(&server, &user.token, &prompt_body(model_id, "t1"))
        .await
        .json()
        .await
        .unwrap();
    let t1_id = t1["id"].as_str().unwrap().to_string();

    // Update to a foreign / non-owned assistant is rejected. NOTE: the handler
    // returns 404 (`not_found("Assistant")`) for an assistant the user can't
    // reach — indistinguishable from a non-existent one, so existence isn't
    // leaked. (TESTS.md TEST-10 phrases this as "403"; the implementation uses
    // 404-not-403 — see handlers.rs::update_task. Either way the update is
    // rejected and the foreign assistant is never written.)
    let foreign_assistant = Uuid::new_v4().to_string();
    let resp = client()
        .put(server.api_url(&format!("/scheduled-tasks/{t1_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "assistant_id": foreign_assistant }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "update to an inaccessible assistant is rejected (404, existence not leaked)"
    );

    // Update to an inaccessible model → 403.
    let resp = client()
        .put(server.api_url(&format!("/scheduled-tasks/{t1_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "model_id": foreign_model_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "update to an inaccessible model → 403"
    );

    // Disable task1 (active → 0), then create task2 (active → 1, at cap).
    let disable = client()
        .put(server.api_url(&format!("/scheduled-tasks/{t1_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(disable.status(), StatusCode::OK);
    let t2 = create_task(&server, &user.token, &prompt_body(model_id, "t2")).await;
    assert_eq!(t2.status(), StatusCode::CREATED, "task2 fits under cap=1");

    // Re-enable task1 now that task2 occupies the single active slot → over cap → 4xx.
    let reenable = client()
        .put(server.api_url(&format!("/scheduled-tasks/{t1_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        reenable.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "re-enabling over the active-task cap → 422"
    );

    // Raise the cap to 2, then re-enable succeeds (under cap → 200).
    set_cap(&server, &admin.token, 2).await;
    let reenable = client()
        .put(server.api_url(&format!("/scheduled-tasks/{t1_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        reenable.status(),
        StatusCode::OK,
        "re-enabling under the (raised) cap → 200"
    );
    let reenabled: Value = reenable.json().await.unwrap();
    assert_eq!(reenabled["enabled"], true, "task re-enabled");
    assert!(
        reenabled["paused_reason"].is_null(),
        "a user re-enable clears any paused_reason"
    );
}

// ── TEST-11 — re-enable quota excludes the row being re-enabled ─────────────

#[tokio::test]
async fn reenable_quota_count_excludes_the_disabled_row() {
    // Real-path coverage of the "no off-by-one" property: with cap=1 and exactly
    // ONE task that is currently DISABLED, re-enabling it must be allowed (the
    // active count is 0, not 1). If the count wrongly included the row being
    // re-enabled, this would 422.
    let server = TestServer::start().await;
    let admin = create_user_with_permissions(
        &server,
        "sched_v11_admin",
        &["scheduler::admin::read", "scheduler::admin::manage"],
    )
    .await;
    set_cap(&server, &admin.token, 1).await;

    let user = create_user_with_permissions(&server, "sched_v11", &["scheduler::use"]).await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    let task: Value = create_task(&server, &user.token, &prompt_body(model_id, "only"))
        .await
        .json()
        .await
        .unwrap();
    let id = task["id"].as_str().unwrap().to_string();

    // Disable (active → 0).
    let disable = client()
        .put(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(disable.status(), StatusCode::OK);

    // Re-enable the SAME (and only) task at cap=1 → must succeed (count excludes it).
    let reenable = client()
        .put(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({ "enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        reenable.status(),
        StatusCode::OK,
        "re-enabling the only (disabled) task at cap=1 must not 422 (no off-by-one)"
    );
}

// ── TEST-28 — unattended allow-list narrows, never widens, access ───────────

/// Create an enabled, user-owned MCP server the caller can access; returns its id.
async fn create_user_mcp_server(server: &TestServer, token: &str) -> String {
    let resp = client()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "name": format!("allow_srv_{}", &Uuid::new_v4().to_string()[..8]),
            "display_name": "Allow-list server",
            "enabled": true,
            "transport_type": "http",
            "url": "http://127.0.0.1:9/mcp",
            "timeout_seconds": 10
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "user MCP server create should 201"
    );
    let srv: Value = resp.json().await.unwrap();
    srv["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn allow_list_accepts_accessible_and_rejects_inaccessible_entries() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "sched_v28",
        &["scheduler::use", "mcp_servers::create", "mcp_servers::read", "mcp_servers::edit"],
    )
    .await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();
    let accessible_server = create_user_mcp_server(&server, &user.token).await;

    // Create with an allow-list ⊆ the user's accessible servers → 201, persisted.
    let mut body = prompt_body(model_id, "allowlisted");
    body["allowed_unattended_tools"] = json!([{ "server_id": accessible_server }]);
    let resp = create_task(&server, &user.token, &body).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "allow-list of accessible servers → 201"
    );
    let task: Value = resp.json().await.unwrap();
    let id = task["id"].as_str().unwrap().to_string();
    // Round-trips (the entry is persisted).
    let allowed = task["allowed_unattended_tools"].as_array().unwrap();
    assert_eq!(allowed.len(), 1, "allow-list persisted");
    assert_eq!(allowed[0]["server_id"], accessible_server);

    // Create with an allow-list referencing a server the user CANNOT access → 4xx.
    let mut bad = prompt_body(model_id, "widen");
    bad["allowed_unattended_tools"] = json!([{ "server_id": Uuid::new_v4().to_string() }]);
    let resp = create_task(&server, &user.token, &bad).await;
    assert!(
        resp.status().is_client_error(),
        "allow-list referencing an inaccessible server must be rejected; got {}",
        resp.status()
    );
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "inaccessible allow-list entry → 403 (SCHEDULER_ALLOWLIST_INACCESSIBLE)"
    );

    // Same guard on UPDATE.
    let resp = client()
        .put(server.api_url(&format!("/scheduled-tasks/{id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&json!({
            "allowed_unattended_tools": [{ "server_id": Uuid::new_v4().to_string() }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "update to an inaccessible allow-list entry → 403"
    );
}

// ── TEST-33 — a workflow needing interactive input can't be scheduled ───────

/// A single-step workflow that PAUSES on human input (an `elicit` step) — it
/// would park as `waiting` under a headless scheduled run.
const ELICIT_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
inputs:
  - name: topic
    required: true
steps:
  - id: confirm
    kind: elicit
    message: "Proceed with {{ inputs.topic }}?"
    schema:
      type: object
      properties:
        proceed:
          type: boolean
          title: "Proceed?"
      required: [proceed]
    timeout_ms: 300000
outputs:
  - name: decision
    from: "{{ confirm.output }}"
"#;

/// A single-step workflow with NO human-input step (safe to run unattended).
const NO_ELICIT_WORKFLOW_YAML: &str = r#"inputs:
  - name: topic
    required: true
steps:
  - id: gen
    kind: llm
    prompt: "say something about {{ inputs.topic }}"
outputs:
  - name: result
    from: "{{ gen.output }}"
"#;

fn workflow_task_body(workflow_id: &str, model_id: &str, name: &str) -> Value {
    json!({
        "name": name,
        "target_kind": "workflow",
        "workflow_id": workflow_id,
        "model_id": model_id,
        "schedule_kind": "recurring",
        "cron_expr": "0 9 * * 1",
        "timezone": "UTC",
    })
}

#[tokio::test]
async fn workflow_with_elicit_step_cannot_be_scheduled() {
    let server = TestServer::start().await;
    let user = create_user_with_permissions(
        &server,
        "sched_v33",
        &[
            "scheduler::use",
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::execute",
        ],
    )
    .await;
    let (_stub, model) = crate::chat::helpers::create_stub_model(&server, &user.user_id).await;
    let model_id = model["id"].as_str().unwrap();

    // Import an elicit workflow (owned by this user → accessible).
    let elicit_wf =
        crate::workflow::import_dev_workflow(&server, &user.token, "sched-elicit", ELICIT_WORKFLOW_YAML)
            .await;
    let elicit_wf_id = elicit_wf["id"].as_str().unwrap();

    let resp = create_task(
        &server,
        &user.token,
        &workflow_task_body(elicit_wf_id, model_id, "needs-input"),
    )
    .await;
    assert!(
        resp.status().is_client_error(),
        "scheduling a workflow that needs interactive input must be rejected; got {}",
        resp.status()
    );
    let status = resp.status();
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "elicit workflow → 400: {body}"
    );
    assert_eq!(
        body["error_code"], "SCHEDULER_WORKFLOW_NEEDS_INPUT",
        "clear needs-interactive-input error: {body}"
    );

    // A workflow with NO elicit step is schedulable → 201.
    let ok_wf = crate::workflow::import_dev_workflow(
        &server,
        &user.token,
        "sched-no-elicit",
        NO_ELICIT_WORKFLOW_YAML,
    )
    .await;
    let ok_wf_id = ok_wf["id"].as_str().unwrap();
    let resp = create_task(
        &server,
        &user.token,
        &workflow_task_body(ok_wf_id, model_id, "no-input"),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "a no-elicit workflow is schedulable"
    );
}

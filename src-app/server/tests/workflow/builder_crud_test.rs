//! Visual-builder definition-CRUD endpoints (TEST-1 / TEST-2 / TEST-3):
//!   GET  /api/workflows/{id}/definition   — load the editable WorkflowDef
//!   POST /api/workflows                    — create from a posted WorkflowDef
//!   PUT  /api/workflows/{id}/definition    — replace steps/inputs in place
//!
//! These exercise the builder's load / save-new / edit-in-place surface. All run
//! against the real backend (per-test DB + spawned server) with NO external API
//! keys — the defs are never executed, only parsed / validated / compiled.

use serde_json::{json, Value as Json};
use uuid::Uuid;

use super::{import_dev_workflow, plain_server, workflow_user, SIMPLE_OK_YAML};

/// A minimal, valid 1-step llm `WorkflowDef` JSON body (the shape `POST
/// /workflows` + `PUT /workflows/{id}/definition` accept). Distinct step id so a
/// listing can be matched.
fn simple_def() -> Json {
    json!({
        "inputs": [{ "name": "topic", "required": true }],
        "steps": [{
            "id": "gen",
            "kind": "llm",
            "prompt": "say something about {{ inputs.topic }}"
        }],
        "outputs": [{ "name": "result", "from": "{{ gen.output }}" }]
    })
}

/// The builder create body = `{ name, ...WorkflowDef }` — the api-client
/// serializes a POST's `name` into the JSON BODY (flattened onto the def),
/// NOT a query param. Sending it as a query param silently dropped it.
fn named_body(mut def: Json, name: &str) -> Json {
    def["name"] = json!(name);
    def
}

// ── TEST-1 — GET /workflows/{id}/definition ───────────────────────────────────

#[tokio::test]
async fn get_definition_owner_ok_foreign_404_unauth_401() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_getdef_owner").await;
    let wf = import_dev_workflow(&server, &owner.token, "getdef", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().expect("workflow id").to_string();
    let client = reqwest::Client::new();

    // Owner → 200 with the editable WorkflowDef (steps + inputs).
    let resp = client
        .get(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("get definition");
    assert_eq!(resp.status(), 200, "owner reads the definition");
    let def: Json = resp.json().await.expect("parse definition");
    let steps = def["steps"].as_array().expect("steps array");
    assert_eq!(steps.len(), 1, "SIMPLE_OK_YAML has one step: {def}");
    assert_eq!(steps[0]["id"], "gen", "step id parsed into the def: {def}");
    assert_eq!(steps[0]["kind"], "llm", "step kind parsed: {def}");
    assert_eq!(
        def["inputs"][0]["name"], "topic",
        "inputs carried in the editable def: {def}"
    );

    // A DIFFERENT user (with full workflow perms) must not see it → 404.
    let other = workflow_user(&server, "wf_getdef_other").await;
    let resp = client
        .get(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .expect("get definition as other");
    assert_eq!(
        resp.status(),
        404,
        "a non-owner's id lookup 404s (no cross-user leak)"
    );

    // Unauthenticated → 401.
    let resp = client
        .get(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .send()
        .await
        .expect("get definition unauth");
    assert_eq!(resp.status(), 401, "no token → 401");
}

// ── TEST-2 — POST /workflows (create from a WorkflowDef) ───────────────────────

#[tokio::test]
async fn create_from_def_lists_dupe_409_invalid_rejected_no_row() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_create_def").await;
    let client = reqwest::Client::new();

    // Create a user-scope workflow from a posted def → 201, and it lists.
    let resp = client
        .post(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&named_body(simple_def(), "builder-create"))
        .send()
        .await
        .expect("create from def");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse create body");
    assert_eq!(status, 201, "create-from-def should 201: {body}");
    assert_eq!(body["scope"], "user", "builder create forces user scope: {body}");
    // Regression guard: the builder-sent `name` (in the BODY, not a query param)
    // must round-trip. Reading it from a query dropped it → every builder workflow
    // showed as the default "imported-workflow" and a 2nd save 409'd.
    assert_eq!(
        body["display_name"], "builder-create",
        "builder name must round-trip to display_name: {body}"
    );
    let created_id = body["id"].as_str().expect("created id").to_string();

    let list: Json = client
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("parse list");
    let listed = list["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .any(|w| w["id"].as_str() == Some(created_id.as_str()));
    assert!(listed, "created workflow is listable: {list}");

    // Same name again → 409 WORKFLOW_NAME_EXISTS (never a silent overwrite).
    let resp = client
        .post(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&named_body(simple_def(), "builder-create"))
        .send()
        .await
        .expect("create dupe");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse dupe body");
    assert_eq!(status, 409, "duplicate name → 409: {body}");
    assert_eq!(
        body["error_code"], "WORKFLOW_NAME_EXISTS",
        "409 carries the WORKFLOW_NAME_EXISTS code: {body}"
    );

    // A def that FAILS validation (dead `tools:` on an llm step —
    // WORKFLOW_DEAD_TOOLS_FIELD) must be rejected non-2xx, creating NO row.
    let bad_def = json!({
        "inputs": [{ "name": "topic", "required": true }],
        "steps": [{
            "id": "gen",
            "kind": "llm",
            "prompt": "hi {{ inputs.topic }}",
            "tools": ["web_search"]
        }],
        "outputs": [{ "name": "result", "from": "{{ gen.output }}" }]
    });
    let resp = client
        .post(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user.token))
        .json(&named_body(bad_def, "builder-invalid"))
        .send()
        .await
        .expect("create invalid");
    let status = resp.status();
    assert!(
        !status.is_success(),
        "an invalid def is rejected non-2xx; got {status}"
    );

    // No row was created for the rejected name.
    let list: Json = client
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .expect("list after invalid")
        .json()
        .await
        .expect("parse list");
    let leaked = list["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .any(|w| w["name"].as_str().unwrap_or("").contains("builder-invalid"));
    assert!(!leaked, "a rejected create leaves no row: {list}");
}

// ── TEST-3 — PUT /workflows/{id}/definition (edit in place) ────────────────────

#[tokio::test]
async fn put_definition_edits_in_place_preserving_id() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "wf_putdef_owner").await;
    let client = reqwest::Client::new();

    // Seed via the create path so it's a user-scope, owner-editable row.
    let created: Json = client
        .post(server.api_url("/workflows?name=builder-edit"))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&simple_def())
        .send()
        .await
        .expect("create for edit")
        .json()
        .await
        .expect("parse create");
    let wf_id = created["id"].as_str().expect("id").to_string();
    let ir_before = created["compiled_ir_json"]["step_count"].clone();
    assert_eq!(ir_before, 1, "starts as a 1-step IR: {created}");

    // Edit: replace the steps with a 2-step def (adds a `summarize` step).
    let edited_def = json!({
        "inputs": [{ "name": "topic", "required": true }],
        "steps": [
            { "id": "gen", "kind": "llm", "prompt": "about {{ inputs.topic }}" },
            {
                "id": "summarize",
                "kind": "llm",
                "prompt": "summarize {{ gen.output }}",
                "depends_on": ["gen"]
            }
        ],
        "outputs": [{ "name": "result", "from": "{{ summarize.output }}" }]
    });
    let resp = client
        .put(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&edited_def)
        .send()
        .await
        .expect("put definition");
    let status = resp.status();
    let updated: Json = resp.json().await.expect("parse updated");
    assert_eq!(status, 200, "edit-in-place should 200: {updated}");

    // The id is UNCHANGED (run-history FKs survive the edit).
    assert_eq!(
        updated["id"].as_str(),
        Some(wf_id.as_str()),
        "id preserved across an in-place edit: {updated}"
    );
    // The recompiled IR changed (1 → 2 steps).
    assert_eq!(
        updated["compiled_ir_json"]["step_count"], 2,
        "recompiled IR reflects the new step count: {updated}"
    );

    // Refetched definition reflects the change.
    let def: Json = client
        .get(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .send()
        .await
        .expect("get updated definition")
        .json()
        .await
        .expect("parse def");
    let step_ids: Vec<&str> = def["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["id"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        step_ids,
        vec!["gen", "summarize"],
        "refetched def carries the edited steps: {def}"
    );

    // Non-owner (with workflows::manage) → 403 (not 404): the row exists but
    // isn't theirs.
    let other = workflow_user(&server, "wf_putdef_other").await;
    let resp = client
        .put(server.api_url(&format!("/workflows/{wf_id}/definition")))
        .header("Authorization", format!("Bearer {}", other.token))
        .json(&edited_def)
        .send()
        .await
        .expect("put as non-owner");
    assert_eq!(resp.status(), 403, "a non-owner edit is forbidden (403)");

    // Missing id → 404.
    let missing = Uuid::new_v4();
    let resp = client
        .put(server.api_url(&format!("/workflows/{missing}/definition")))
        .header("Authorization", format!("Bearer {}", owner.token))
        .json(&edited_def)
        .send()
        .await
        .expect("put missing id");
    assert_eq!(resp.status(), 404, "editing a missing workflow → 404");
}

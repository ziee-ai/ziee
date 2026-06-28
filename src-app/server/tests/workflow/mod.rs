//! Workflow consumer integration tests (plan §7 — consumer integration tier).
//!
//! - `install_from_hub` — install a workflow bundle from the mock hub →
//!   DB row + workflow.yaml on disk + Layer 1+2+3 validate (cycle-check)
//!   passed.
//! - `run_mocked` — dev-import a 3-step llm workflow, run it with mocks
//!   for every step → run reaches `completed`, output files written,
//!   final_output_json populated. Mocks are dev-only (is_dev=true), so
//!   this uses the import path; a stub model satisfies the runner's
//!   pre-flight model snapshot without spending tokens.
//! - `validate_and_dry_run` — POST /validate (valid + invalid) and
//!   /dry-run structured responses.
//! - `elicit` — dev-import a single-step `kind: elicit` workflow, run it,
//!   exercise the /elicit endpoint validation paths (ownership 403,
//!   staleness 410, schema 422) + the schema-valid resume → completed.

mod access_and_durability;
mod elicit;
mod elicit_data_seeding;
mod install_from_hub;
mod permissions_gating;
mod real_llm;
mod real_stack;
mod resume;
mod run_cost_test;
mod run_history_and_delete;
mod run_mocked;
mod run_model;
mod sandbox_progress;
mod sandbox_run;
mod sse_ordering;
mod sr_real_llm;
mod sr_workflow;
mod status_machine;
mod stream_access;
mod sync_emit_test;
mod system_endpoints;
mod tool_step;
mod validate_and_dry_run;

use std::io::Write;
use std::time::Duration;

use serde_json::Value as Json;
use uuid::Uuid;

use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;
use crate::hub::mock_release_server::{MockHub, MockItem, MockVersion, spawn_mock_hub};

/// Reverse-DNS name of the fixture workflow the mock catalog serves.
pub const FIXTURE_WORKFLOW_NAME: &str = "io.github.test/research-summarize-write";

/// A valid 3-step sequential llm workflow (research → summarize → write),
/// mirroring the seed `research-summarize-write`. Used by the
/// install-from-hub test (the cycle-check must pass).
pub const FIXTURE_WORKFLOW_YAML: &str = r#"$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
expose_logs: on_error
inputs:
  - name: topic
    description: "What to research and summarize"
    required: true
steps:
  - id: research
    kind: llm
    message: "Researching {{ inputs.topic }}"
    prompt: |
      Produce sources on "{{ inputs.topic }}". Return a JSON array.
    output_format: json
  - id: summarize
    kind: llm
    message: "Summarizing"
    prompt: |
      Summarize: {{ research.output | json }}
    output_format: json
    depends_on: [research]
  - id: write
    kind: llm
    message: "Writing memo"
    prompt: |
      Write a memo on "{{ inputs.topic }}" using {{ summarize.output | json }}
    depends_on: [summarize]
outputs:
  - name: memo
    from: "{{ write.output }}"
    expose: full
"#;

/// One mock catalog version carrying a single workflow item that ships a
/// real workflow.yaml bundle.
pub fn workflow_catalog() -> Vec<MockVersion> {
    vec![MockVersion {
        version: "9.9.1-test",
        prerelease: true,
        items: vec![MockItem::bundle(
            "workflow",
            FIXTURE_WORKFLOW_NAME,
            vec![("workflow.yaml", FIXTURE_WORKFLOW_YAML)],
        )],
    }]
}

/// Boot a TestServer wired to a mock Pages server serving the workflow
/// catalog + bundle.
pub async fn server_with_workflow_catalog() -> (TestServer, MockHub) {
    let mock = spawn_mock_hub(workflow_catalog()).await;
    let server = TestServer::start_with_options(crate::common::TestServerOptions {
        extra_env: mock.test_env(),
        ..Default::default()
    })
    .await;
    (server, mock)
}

/// Admin who can refresh the catalog + manage/execute workflows, with the
/// mock catalog already pulled active.
pub async fn admin_and_refresh(server: &TestServer) -> crate::common::test_helpers::TestUser {
    let admin = create_user_with_permissions(
        server,
        "wf_admin",
        &[
            "hub::catalog::read",
            "hub::catalog::manage",
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::execute",
        ],
    )
    .await;
    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/refresh"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .send()
        .await
        .expect("refresh");
    assert_eq!(
        resp.status(),
        200,
        "/hub/refresh must 200; got {}: {}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    admin
}

/// Install the fixture workflow from the mock hub (user scope). The
/// install pipeline lives in the hub module — `POST /hub/workflows/create`
/// body `{hub_id}`. Returns `{workflow, hub_tracking}` (status 201).
pub async fn install_fixture_workflow(server: &TestServer, token: &str) -> Json {
    let resp = reqwest::Client::new()
        .post(server.api_url("/hub/workflows/create"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "hub_id": FIXTURE_WORKFLOW_NAME }))
        .send()
        .await
        .expect("install workflow");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse install body");
    assert_eq!(
        status, 201,
        "workflow install-from-hub should 201; got {status}: {body}"
    );
    body
}

/// A plain TestServer (no mock catalog) — for tests that only use the
/// dev-import path (run_mocked, elicit, validate).
pub async fn plain_server() -> TestServer {
    TestServer::start().await
}

/// A minimal valid 1-step llm workflow (no sandbox flavor reqs). Shared
/// across the access/durability + system-endpoint tests.
pub const SIMPLE_OK_YAML: &str = r#"inputs:
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

/// Dev-import a workflow on the SYSTEM scope (admin multipart endpoint).
/// Returns the created `Workflow` JSON. `slug` becomes
/// `local.dev.system/<slug>`.
pub async fn system_import_workflow(
    server: &TestServer,
    token: &str,
    slug: &str,
    yaml: &str,
) -> Json {
    let tarball = workflow_tarball(yaml);
    let part = reqwest::multipart::Part::bytes(tarball)
        .file_name("bundle.tar.gz")
        .mime_str("application/gzip")
        .unwrap();
    let form = reqwest::multipart::Form::new().part("bundle", part);
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/system/import?name={slug}")))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(form)
        .send()
        .await
        .expect("system import workflow");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse system import body");
    assert_eq!(status, 201, "system import should 201; got {status}: {body}");
    body
}

/// A user with the workflow permissions needed for dev import + run.
pub async fn workflow_user(server: &TestServer, name: &str) -> crate::common::test_helpers::TestUser {
    create_user_with_permissions(
        server,
        name,
        &["workflows::read", "workflows::install", "workflows::manage", "workflows::execute"],
    )
    .await
}

/// Build a tar.gz containing a single `workflow.yaml` with the given
/// contents (for the dev `/import` multipart upload).
pub fn workflow_tarball(yaml: &str) -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::{Builder, Header};
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = Builder::new(enc);
    let mut header = Header::new_gnu();
    header.set_size(yaml.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder
        .append_data(&mut header, "workflow.yaml", yaml.as_bytes())
        .expect("append workflow.yaml");
    let enc = builder.into_inner().expect("tar into_inner");
    let mut bytes = enc.finish().expect("gz finish");
    bytes.flush().ok();
    bytes
}

/// Dev-import a workflow (is_dev=true → mocks allowed). Returns the
/// created `Workflow` JSON. `slug` becomes `local.dev/<slug>`.
pub async fn import_dev_workflow(
    server: &TestServer,
    token: &str,
    slug: &str,
    yaml: &str,
) -> Json {
    let tarball = workflow_tarball(yaml);
    let part = reqwest::multipart::Part::bytes(tarball)
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
        .expect("import workflow");
    let status = resp.status();
    let body: Json = resp.json().await.expect("parse import body");
    // `import_workflow` returns 201 CREATED (see workflow/handlers/dev.rs).
    assert_eq!(
        status, 201,
        "dev import should 201; got {status}: {body}"
    );
    body
}

/// Kick a `POST /workflows/{id}/run`. Returns the run JSON
/// `{run_id, status}` (status 202 ACCEPTED). `mocks` is honored only for
/// is_dev workflows.
pub async fn run_workflow(
    server: &TestServer,
    token: &str,
    workflow_id: &str,
    body: Json,
) -> Json {
    let resp = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{workflow_id}/run")))
        .header("Authorization", format!("Bearer {token}"))
        .json(&body)
        .send()
        .await
        .expect("run workflow");
    let status = resp.status();
    let parsed: Json = resp.json().await.expect("parse run body");
    assert_eq!(
        status, 202,
        "run should 202 ACCEPTED; got {status}: {parsed}"
    );
    parsed
}

/// Create a stub model + a conversation bound to it, so the runner's
/// model snapshot succeeds without an API key. Returns
/// `(stub_guard, conversation_id)`. KEEP the guard alive.
pub async fn stub_conversation(
    server: &TestServer,
    user_id: &str,
    token: &str,
) -> (crate::common::stub_engine::StubEngine, Uuid) {
    let (stub, model) = crate::chat::helpers::create_stub_model(server, user_id).await;
    let conv = crate::chat::helpers::create_conversation(
        server,
        token,
        Some(Uuid::parse_str(model["id"].as_str().unwrap()).unwrap()),
        Some("workflow run conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().unwrap()).unwrap();
    (stub, conv_id)
}

/// A user with the workflow permissions PLUS `mcp_servers::{create,read}` so
/// the same user can both register the in-process `MockMcpServer` as a user MCP
/// server AND run a workflow whose `tool` step calls it. Mirrors
/// `workflow_user` with the extra MCP grants the A6 tool-step tests need.
pub async fn workflow_tool_user(
    server: &TestServer,
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
        ],
    )
    .await
}

/// Register the in-process `MockMcpServer` (`tests/mcp/fixtures`) as a
/// user-owned HTTP MCP server. Returns `(server_id, server_name)` — the name is
/// what a `tool` step's `server:` field references (resolved at run time against
/// the user's accessible set). 201 expected.
pub async fn register_mock_as_user_server(
    server: &TestServer,
    token: &str,
    name: &str,
    url: &str,
) -> (String, String) {
    let res = reqwest::Client::new()
        .post(server.api_url("/mcp/servers"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "name": name,
            "display_name": "Workflow tool-step mock",
            "transport_type": "http",
            "url": url,
            "enabled": true,
        }))
        .send()
        .await
        .expect("register mock server");
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(status, 201, "register mock server: {status}: {body}");
    let row: Json = serde_json::from_str(&body).expect("parse mcp server row");
    (
        row["id"].as_str().expect("server id").to_string(),
        name.to_string(),
    )
}

/// Create a stub model + grant `user_id` access, returning
/// `(stub_guard, model_id)` WITHOUT a conversation. Keep the guard alive for
/// the run's duration (the runner snapshots the model at run start). Used by the
/// A1 standalone-run + A6 tool-step tests that run with an explicit `model_id`
/// and `conversation_id = None`.
pub async fn stub_model_for(
    server: &TestServer,
    user_id: &str,
) -> (crate::common::stub_engine::StubEngine, Uuid) {
    let (stub, model) = crate::chat::helpers::create_stub_model(server, user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().expect("model id")).unwrap();
    (stub, model_id)
}

/// Open a small pool on the per-test DB for direct-SQL assertions.
pub async fn db_pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

/// Count `files` rows linked to a run (`workflow_run_id = run_id`) with a given
/// `created_by`. The A3/A6/A5 durable-artifact + delete tests assert on this.
pub async fn count_files_for_run(
    pool: &sqlx::PgPool,
    run_id: Uuid,
    created_by: &str,
) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM files WHERE workflow_run_id = $1 AND created_by = $2",
    )
    .bind(run_id)
    .bind(created_by)
    .fetch_one(pool)
    .await
    .expect("count files for run")
}

/// Poll GET /workflow-runs/{run_id} until terminal (completed / failed /
/// cancelled) or timeout. Returns the final run JSON.
pub async fn poll_run(server: &TestServer, token: &str, run_id: Uuid) -> Json {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let run: Json = reqwest::Client::new()
            .get(server.api_url(&format!("/workflow-runs/{run_id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .expect("get run")
            .json()
            .await
            .expect("parse run");
        let status = run["status"].as_str().unwrap_or("");
        if matches!(status, "completed" | "failed" | "cancelled") {
            return run;
        }
        if std::time::Instant::now() >= deadline {
            panic!("workflow run {run_id} did not terminate in time: {run}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

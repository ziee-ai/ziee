//! `spawn_background` ARGUMENT-CONTRACT tests — the integration half of the
//! `tool-argument-contracts` fix.
//!
//! The defect these reproduce: `spawn_background` advertises `kind` as a
//! top-level sibling of `spec`, but `spec` is a permissive open object whose
//! visible properties are the per-kind fields (`command`, `flavor`, `task`,
//! `system`) — which invites a model to nest `kind` beside them. The server read
//! `kind` from the top level ONLY, so a nested `kind` was DROPPED, the
//! `subagent` default substituted, and the call refused with
//! `spec.task must be a non-empty string` — naming a field the model
//! deliberately did not send. When the nested-`kind` spec ALSO carried `task`
//! there was no error at all: a sub-agent ran instead of the requested shell
//! command.
//!
//! Every refusal test here carries its happy-path counterpart in the SAME test,
//! so a refusal can never pass because the whole path is broken.
//!
//! These drive the REAL `/api/background/mcp` route (JSON-RPC over HTTP, the
//! `x-conversation-id` header the chat MCP client sends, a real `background::use`
//! user). `code_sandbox` is DISABLED in the default `TestServer`, so a spawned
//! `sandbox_exec` run fails fast in its detached driver with
//! `SANDBOX_NOT_INITIALIZED` and never touches the network — the spawn RESULT,
//! which is what these tests assert, is produced before the driver runs.

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{background_user, jsonrpc, structured};
use crate::common::TestServer;
use crate::common::stub_engine::StubEngine;
use crate::common::test_helpers::TestUser;

/// A `background::use` user + a conversation backed by a stub model, which is
/// what `spawn_background{kind:'subagent'}` needs to resolve a model.
async fn user_with_conversation(server: &TestServer, name: &str) -> (TestUser, Uuid, StubEngine) {
    let user = background_user(server, name).await;
    let (stub, model) = crate::chat::helpers::create_stub_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().expect("model id")).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("spawn-contract conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().expect("conversation id")).unwrap();
    (user, conv_id, stub)
}

/// The model-facing refusal text from a JSON-RPC error envelope. Panics (with
/// the whole body) when the call SUCCEEDED — which is itself a meaningful
/// failure message for the silent-wrong-thing test.
fn error_message(body: &Json) -> String {
    body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("expected a JSON-RPC error refusal, but the call SUCCEEDED: {body}")
        })
        .to_string()
}

/// Count this user's `workflow_runs` rows — the proof that a refused spawn
/// created nothing.
async fn run_count(server: &TestServer, user_id: &str) -> i64 {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workflow_runs WHERE user_id = $1")
        .bind(Uuid::parse_str(user_id).unwrap())
        .fetch_one(&pool)
        .await
        .expect("count workflow_runs")
}

async fn spawn(server: &TestServer, user: &TestUser, conv: Uuid, arguments: Json) -> Json {
    jsonrpc(
        server,
        &user.token,
        Some(conv),
        "tools/call",
        json!({ "name": "spawn_background", "arguments": arguments }),
    )
    .await
}

// =====================================================================
// TEST-11 — the VERBATIM reported repro (ITEM-1, ITEM-4)
// =====================================================================

/// The exact payload from the live audit rig:
/// `{"spec":{"kind":"sandbox_exec","command":"python hello.py"}}`.
///
/// On `origin/main` this is refused with `spec.task must be a non-empty string`
/// — the model is told to add a field it deliberately did not want, and `kind`
/// is never mentioned. This was 445 failures of 948 calls, the single largest
/// error class in `mcp_tool_calls`.
///
/// The nested `kind` must be HONOURED: the spec is a complete, valid
/// `sandbox_exec` spec, so the call must SUCCEED as a sandbox run.
#[tokio::test]
async fn nested_kind_sandbox_exec_is_honoured_not_blamed_on_spec_task() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_nested_kind_cmd").await;

    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "kind": "sandbox_exec", "command": "python hello.py" } }),
    )
    .await;

    // The regression, stated as the thing that must never happen again: a
    // refusal that names `spec.task` for a call that supplied neither a task nor
    // a subagent kind.
    if let Some(msg) = body["error"]["message"].as_str() {
        assert!(
            !msg.contains("spec.task"),
            "a nested-`kind` sandbox_exec spec must never be refused for a MISSING \
             `spec.task` — that blames a field the model deliberately did not send. \
             Got: {msg}"
        );
        panic!("a complete nested-`kind` sandbox_exec spec must be accepted, got: {msg}");
    }

    let sc = structured(&body);
    assert_eq!(
        sc["kind"], "sandbox_exec",
        "the `kind` supplied inside `spec` must resolve the job kind, not be dropped \
         in favour of the `subagent` default: {sc}"
    );
    assert!(
        sc["run_id"].as_str().is_some(),
        "an accepted spawn returns an opaque run_id: {sc}"
    );

    // HAPPY-PATH COUNTERPART (same server, same user): a correctly-formed
    // top-level-`kind` sub-agent call still works, so the assertions above are
    // about the misplaced kind and not about spawning being broken.
    let ok = spawn(
        &server,
        &user,
        conv,
        json!({ "kind": "subagent", "spec": { "task": "Say a one-line hello." } }),
    )
    .await;
    let ok_sc = structured(&ok);
    assert_eq!(
        ok_sc["kind"], "subagent",
        "a well-formed sub-agent spawn still works: {ok_sc}"
    );
    assert!(
        ok_sc["run_id"].as_str().is_some(),
        "…and returns a run_id: {ok_sc}"
    );
}

// =====================================================================
// TEST-12 — the SILENT-WRONG-THING case (ITEM-1) [acceptance] [INV-2]
// =====================================================================

/// `{"spec":{"kind":"sandbox_exec","task":"…"}}`.
///
/// On `origin/main` this SUCCEEDS and runs a **sub-agent** — the requested kind
/// is discarded, the default substituted, and the caller is told nothing. Silent
/// wrong-thing is worse than a loud failure, and this is the invariant
/// (`a supplied argument is never silently replaced by the default`) stated
/// executably.
///
/// After the fix the supplied `kind` is honoured, so the call is refused for the
/// `command` a `sandbox_exec` spec actually needs — never quietly downgraded.
#[tokio::test]
async fn nested_kind_never_silently_runs_the_other_job_kind() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_nested_kind_silent").await;

    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "kind": "sandbox_exec", "task": "Say a one-line hello." } }),
    )
    .await;

    // The load-bearing assertion: it must NOT have quietly run a sub-agent.
    if body["error"].is_null() {
        let sc = structured(&body);
        panic!(
            "a spec that asked for `kind: sandbox_exec` must NEVER silently run a \
             `{}` job instead — the supplied kind was discarded and the default \
             substituted, with no error at all: {sc}",
            sc["kind"]
        );
    }

    let msg = error_message(&body);
    assert!(
        msg.contains("command"),
        "the refusal must be about the field a sandbox_exec spec is MISSING \
         (`spec.command`), which is what honouring the supplied kind implies: {msg}"
    );

    // HAPPY-PATH COUNTERPART / POSITIVE CONTROL: an explicit sub-agent spawn on
    // the SAME server does still produce a sub-agent run. Without this, the
    // refusal above is indistinguishable from "sub-agents are broken".
    let ok = spawn(
        &server,
        &user,
        conv,
        json!({ "kind": "subagent", "spec": { "task": "Say a one-line hello." } }),
    )
    .await;
    let ok_sc = structured(&ok);
    assert_eq!(
        ok_sc["kind"], "subagent",
        "the positive control: an explicitly-requested sub-agent DOES still run: {ok_sc}"
    );
}

// =====================================================================
// TEST-13 — the advertised `flavor` enum is enforced (ITEM-6) [acceptance] [INV-3]
// =====================================================================

/// `flavor` is advertised as `"enum": ["minimal","full"]` and was enforced
/// nowhere: any non-empty string flowed into
/// `format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` and became a live
/// GitHub Releases request. A model invented `"zee-workflow"` and it reached the
/// network.
#[tokio::test]
async fn invented_sandbox_flavor_is_refused_before_any_run_row_exists() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_bad_flavor").await;

    let before = run_count(&server, &user.user_id).await;

    let body = spawn(
        &server,
        &user,
        conv,
        json!({
            "kind": "sandbox_exec",
            "spec": { "command": "echo hi", "flavor": "zee-workflow" }
        }),
    )
    .await;

    let msg = error_message(&body);
    assert!(
        msg.contains("flavor"),
        "the refusal must name the `flavor` argument: {msg}"
    );
    assert!(
        msg.contains("minimal") && msg.contains("full"),
        "the refusal must list the flavors the schema actually advertises: {msg}"
    );

    assert_eq!(
        run_count(&server, &user.user_id).await,
        before,
        "a refused flavor must create NO workflow_runs row — the check has to land \
         before the run exists, and therefore before any URL is constructed"
    );

    // HAPPY-PATH COUNTERPART: both advertised flavors are still accepted.
    for flavor in ["minimal", "full"] {
        let ok = spawn(
            &server,
            &user,
            conv,
            json!({
                "kind": "sandbox_exec",
                "spec": { "command": "echo hi", "flavor": flavor }
            }),
        )
        .await;
        let sc = structured(&ok);
        assert_eq!(
            sc["kind"], "sandbox_exec",
            "the advertised flavor `{flavor}` must still be accepted: {sc}"
        );
    }
}

// =====================================================================
// TEST-14 — unknown `spec` key + unknown `kind` value (ITEM-3, ITEM-2)
// =====================================================================

#[tokio::test]
async fn unadvertised_spec_key_and_unknown_kind_are_refused_actionably() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_unknown_keys").await;

    // (a) A key the schema never advertised — the typo population
    // (`cmd`/`prompt`/`script`) that silently did nothing.
    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "kind": "subagent", "spec": { "task": "x", "priority": "high" } }),
    )
    .await;
    let msg = error_message(&body);
    assert!(
        msg.contains("priority"),
        "the refusal must name the offending key: {msg}"
    );
    assert!(
        msg.contains("task") && msg.contains("command"),
        "…and list the keys `spec` actually accepts: {msg}"
    );

    // (b) An unknown `kind` value must list the valid kinds.
    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "kind": "zee-workflow", "spec": { "task": "x" } }),
    )
    .await;
    let msg = error_message(&body);
    assert!(msg.contains("kind"), "the refusal must name `kind`: {msg}");
    assert!(
        msg.contains("subagent") && msg.contains("sandbox_exec"),
        "…and list BOTH valid kinds: {msg}"
    );

    // HAPPY-PATH COUNTERPART: a spec using only advertised keys still spawns.
    let ok = spawn(
        &server,
        &user,
        conv,
        json!({
            "kind": "subagent",
            "spec": { "task": "Say a one-line hello.", "system": "Be terse." }
        }),
    )
    .await;
    let sc = structured(&ok);
    assert_eq!(
        sc["kind"], "subagent",
        "every advertised key is still accepted: {sc}"
    );
}

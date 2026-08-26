//! `spawn_background` DEDUP + per-conversation CAP guard — the acceptance
//! integration tests for `background-spawn-loop-guard` (INV-1/INV-2/INV-3).
//!
//! The defect these reproduce: a confused model (esp. a weaker local one) spawned
//! the SAME background task repeatedly — the DB showed 9 `job_kind=subagent` runs,
//! 6–7 the IDENTICAL spec — because each `[Background task complete]` completion
//! re-injected into the conversation re-triggered the model, and there was NO
//! spawn de-duplication and NO per-conversation cap. The fix refuses a
//! duplicate / over-cap spawn at the backend boundary, which is what breaks the
//! loop.
//!
//! These drive the REAL `/api/background/mcp` route (JSON-RPC over HTTP, the
//! `x-conversation-id` header the chat MCP client sends). Duplicate/cap decisions
//! are made BEFORE any detached task runs, so no real LLM / rootfs is needed:
//! non-terminal / recent runs are SEEDED directly into `workflow_runs` and the
//! guard reads that live state. `code_sandbox` is disabled in the default
//! TestServer, so only the `subagent` kind is exercised here (the guard applies to
//! both kinds; the sandbox path shares the identical seam).

use serde_json::{Value as Json, json};
use uuid::Uuid;

use super::{background_user, jsonrpc, structured};
use crate::common::TestServer;
use crate::common::stub_engine::StubEngine;
use crate::common::test_helpers::TestUser;

/// A `background::use` user + a stub-model conversation — what
/// `spawn_background{kind:'subagent'}` needs to resolve a model. The stub is
/// returned so it stays alive for the whole test.
async fn user_with_conversation(server: &TestServer, name: &str) -> (TestUser, Uuid, StubEngine) {
    let user = background_user(server, name).await;
    let (stub, model) = crate::chat::helpers::create_stub_model(server, &user.user_id).await;
    let model_id = Uuid::parse_str(model["id"].as_str().expect("model id")).unwrap();
    let conv = crate::chat::helpers::create_conversation(
        server,
        &user.token,
        Some(model_id),
        Some("spawn-guard conv"),
    )
    .await;
    let conv_id = Uuid::parse_str(conv["id"].as_str().expect("conversation id")).unwrap();
    (user, conv_id, stub)
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

/// The model-facing refusal text from a JSON-RPC error envelope; panics (with the
/// whole body) when the call SUCCEEDED — itself a meaningful failure message.
fn error_message(body: &Json) -> String {
    body["error"]["message"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("expected a JSON-RPC error refusal, but the call SUCCEEDED: {body}")
        })
        .to_string()
}

/// Count the conversation's background `workflow_runs` rows — the proof that a
/// refused / deduped spawn created nothing.
async fn run_count(server: &TestServer, conv: Uuid) -> i64 {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let n = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM workflow_runs WHERE conversation_id = $1 AND job_kind <> 'workflow'",
    )
    .bind(conv)
    .fetch_one(&pool)
    .await
    .expect("count workflow_runs");
    pool.close().await;
    n
}

/// Seed a background `workflow_runs` row directly (bypassing the spawn boundary),
/// so the guard has live state to read. Controls BOTH timestamps independently:
/// `created_age_secs` sets `created_at` (SPAWN time) and `updated_age_secs` sets
/// `updated_at` (the run's last-transition time — for a terminal run, its
/// COMPLETION time, which is what the dedup window keys off). Keeping them separate
/// is what lets a test model a LONG run: created long ago, completed just now.
async fn seed_bg_run(
    server: &TestServer,
    conv: Uuid,
    user_id: &str,
    inputs: Json,
    status: &str,
    created_age_secs: i64,
    updated_age_secs: i64,
) -> Uuid {
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let owner = Uuid::parse_str(user_id).unwrap();
    let id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO workflow_runs \
            (job_kind, conversation_id, user_id, run_kind, invocation_source, inputs_json, status, created_at, updated_at) \
         VALUES ('subagent', $1, $2, 'normal', 'conversation', $3, $4, \
                 now() - (interval '1 second' * $5), now() - (interval '1 second' * $6)) \
         RETURNING id",
    )
    .bind(conv)
    .bind(owner)
    .bind(inputs)
    .bind(status)
    .bind(created_age_secs)
    .bind(updated_age_secs)
    .fetch_one(&pool)
    .await
    .expect("seed background run");
    pool.close().await;
    id
}

// =====================================================================
// TEST-5 — positive control: a first spawn (no prior run) still works
// =====================================================================

#[tokio::test]
async fn first_spawn_of_a_spec_still_succeeds() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_guard_first").await;

    assert_eq!(run_count(&server, conv).await, 0, "no runs yet");

    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "task": "Say a one-line hello." } }),
    )
    .await;
    let sc = structured(&body);
    assert_eq!(
        sc["status"], "pending",
        "a first spawn returns a pending handle: {sc}"
    );
    assert!(sc["run_id"].as_str().is_some(), "…and a run_id: {sc}");

    assert_eq!(
        run_count(&server, conv).await,
        1,
        "the first spawn creates exactly one run — the guard does not block legitimate spawns"
    );
}

// =====================================================================
// TEST-1 — INV-1: an identical in-flight spec is deduped, not re-spawned
// =====================================================================

/// [acceptance] [invariant: INV-1] With an identical NON-TERMINAL run already
/// present, a second `spawn_background` of the same spec returns a clear "already
/// running/queued" result carrying the EXISTING run_id and creates NO second row.
#[tokio::test]
async fn identical_inflight_spec_is_deduped_to_one_run() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_guard_dedup").await;

    // A running sub-agent with this exact spec already exists (inputs_json is the
    // spec with `kind` removed, i.e. what the spawn path stores).
    let existing = seed_bg_run(
        &server,
        conv,
        &user.user_id,
        json!({ "task": "Generate the Mandelbrot set" }),
        "running",
        0,
        0,
    )
    .await;
    assert_eq!(run_count(&server, conv).await, 1, "one seeded run");

    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "task": "Generate the Mandelbrot set" } }),
    )
    .await;
    let sc = structured(&body);
    assert_eq!(
        sc["status"], "already_running",
        "an identical in-flight spec must be reported as already running, not re-spawned: {sc}"
    );
    assert_eq!(
        sc["run_id"].as_str().unwrap(),
        existing.to_string(),
        "the dedup result carries the EXISTING run_id (so the model can wait/collect on it): {sc}"
    );

    assert_eq!(
        run_count(&server, conv).await,
        1,
        "the duplicate spawn created NO second run — spawn is idempotent (INV-1)"
    );
}

// =====================================================================
// TEST-2 — INV-2: over-cap spawn is refused and creates no run
// =====================================================================

/// [acceptance] [invariant: INV-2] With the conversation already at the cap of
/// non-terminal background runs (default `fan_out_max_threads` = 6), a further
/// distinct spawn is refused with a clear over-cap error and creates NO run; a
/// spawn below the cap still succeeds (positive control that the cap reads live
/// state).
#[tokio::test]
async fn over_cap_spawn_is_refused_and_creates_no_run() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_guard_cap").await;

    // Seed exactly the default cap (6) of DISTINCT-spec running runs, so dedup
    // never fires and only the CAP can.
    let mut seeded = Vec::new();
    for i in 0..6 {
        seeded.push(
            seed_bg_run(
                &server,
                conv,
                &user.user_id,
                json!({ "task": format!("distinct task {i}") }),
                "running",
                0,
                0,
            )
            .await,
        );
    }
    assert_eq!(
        run_count(&server, conv).await,
        6,
        "six in-flight runs (at the cap)"
    );

    // The 7th distinct spawn is over the cap.
    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "task": "the seventh, distinct task" } }),
    )
    .await;
    let msg = error_message(&body).to_lowercase();
    assert!(
        msg.contains("cap") && msg.contains("in flight"),
        "the over-cap refusal must clearly name the cap: {msg}"
    );
    assert_eq!(
        run_count(&server, conv).await,
        6,
        "an over-cap spawn creates NO new run (INV-2)"
    );

    // POSITIVE CONTROL: free a slot (mark one seeded run completed but OLD, so it
    // neither counts as active nor dedups), then a distinct spawn succeeds — the
    // cap reads LIVE state, it is not a blanket block.
    {
        let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
        sqlx::query("UPDATE workflow_runs SET status = 'completed', created_at = now() - interval '1 hour' WHERE id = $1")
            .bind(seeded[0])
            .execute(&pool)
            .await
            .expect("free a slot");
        pool.close().await;
    }
    let ok = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "task": "an eighth, distinct task" } }),
    )
    .await;
    let sc = structured(&ok);
    assert_eq!(
        sc["status"], "pending",
        "with a slot freed (5 active < cap 6), a distinct spawn succeeds again: {sc}"
    );
    assert_eq!(
        run_count(&server, conv).await,
        7,
        "the accepted below-cap spawn created a new row (6 seeded + 1)"
    );
}

// =====================================================================
// TEST-3 — INV-3: a completion re-injection does not yield a new run
// =====================================================================

/// [acceptance] [invariant: INV-3] A `[Background task complete]` re-injection is
/// the state where a RECENTLY-COMPLETED identical run exists; the re-engaged
/// model's re-spawn of that same spec must be refused as a duplicate (creating no
/// new run), so completion feedback is not a spawn trigger. Positive control: an
/// OLD completed run (outside the dedup window) does NOT block a legitimate re-run.
#[tokio::test]
async fn completed_reinjection_does_not_respawn() {
    let server = TestServer::start().await;
    let (user, conv, _stub) = user_with_conversation(&server, "bg_guard_reinject").await;

    // A sub-agent that just COMPLETED (its completion is what re-injected the
    // conversation) with this exact spec. Model a LONG run: created well before the
    // dedup window (1h ago) but COMPLETED just now (updated_at=now). The dedup must
    // key off COMPLETION time (updated_at), NOT spawn time (created_at) — else a
    // task that ran longer than the window has its re-inject escape dedup, which is
    // the exact loop this feature must break. (With the created_at bug this seed
    // would NOT dedup and the assertion below would fail.)
    let done = seed_bg_run(
        &server,
        conv,
        &user.user_id,
        json!({ "task": "Generate the Mandelbrot set" }),
        "completed",
        3600, // created 1h ago (a long run) — OUTSIDE the window by created_at
        0,    // but completed just now — INSIDE the window by updated_at
    )
    .await;
    assert_eq!(
        run_count(&server, conv).await,
        1,
        "one recently-completed run"
    );

    // What the re-engaged model does: spawn the identical task again.
    let body = spawn(
        &server,
        &user,
        conv,
        json!({ "spec": { "task": "Generate the Mandelbrot set" } }),
    )
    .await;
    let sc = structured(&body);
    assert_eq!(
        sc["status"], "already_running",
        "a recently-completed identical spec is refused as a duplicate — completion \
         feedback must not re-spawn (INV-3): {sc}"
    );
    assert_eq!(sc["run_id"].as_str().unwrap(), done.to_string());
    assert_eq!(
        run_count(&server, conv).await,
        1,
        "the completion re-injection created NO new run (INV-3)"
    );

    // POSITIVE CONTROL: an OLD completed run (COMPLETED well outside the 300s dedup
    // window — updated_at 1h ago) must NOT block a deliberate later re-run — in a
    // fresh conversation to isolate the count. Both timestamps old = genuinely
    // finished long ago (distinct from the long-run case above where updated_at is
    // recent), so keying on updated_at still allows this deliberate re-run.
    let (user2, conv2, _stub2) = user_with_conversation(&server, "bg_guard_oldrun").await;
    seed_bg_run(
        &server,
        conv2,
        &user2.user_id,
        json!({ "task": "Generate the Mandelbrot set" }),
        "completed",
        3600, // created an hour ago
        3600, // AND completed an hour ago — past the window by updated_at
    )
    .await;
    let ok = spawn(
        &server,
        &user2,
        conv2,
        json!({ "spec": { "task": "Generate the Mandelbrot set" } }),
    )
    .await;
    let sc = structured(&ok);
    assert_eq!(
        sc["status"], "pending",
        "an identical spec whose only prior run is OLD-completed still spawns (the window \
         allows a deliberate re-run): {sc}"
    );
    assert_eq!(
        run_count(&server, conv2).await,
        2,
        "the deliberate re-run created a new row"
    );
}

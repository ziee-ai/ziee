//! ITEM-6/7/8/10 + INV-2/INV-4 — fan-out CHILD sub-agent transcripts.
//!
//! Drives the REAL server-side fan-out persistence (`ChatChildSinkFactory` +
//! `insert_subagent_child_run`/`set_run_status` + the `GET /api/subagent-runs`
//! endpoints) deterministically, without a live LLM. The agent-core → factory
//! wiring itself is proven separately by the `agent-core` fanout tests (TEST-3 /
//! TEST-5); here we prove the server half end-to-end: a child row is created,
//! its transcript persists, it is read back owner-scoped via the endpoint, it
//! cascade-deletes with its conversation, and its terminal transition emits sync.

use agent_core::{AgentEvent, ChildSink};
use ai_providers::{ChatMessage, ContentBlock, Role};
use serde_json::{Value, json};
use uuid::Uuid;
use ziee::workflow::{ChatChildSinkFactory, insert_subagent_child_run, set_run_status};

use super::{db_pool, plain_server, workflow_user};

/// Insert a real conversation owned by `user` (the `parent_conversation_id`
/// cascade FK target).
async fn make_conversation(pool: &sqlx::PgPool, user_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO conversations (id, user_id) VALUES ($1, $2)")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert conversation");
    id
}

/// A realistic child transcript: thinking → tool call → final message.
fn child_turn_event() -> AgentEvent {
    AgentEvent::Message(ChatMessage::with_blocks(
        Role::Assistant,
        vec![
            ContentBlock::Thinking {
                thinking: "child planning".into(),
                signature: None,
            },
            ContentBlock::ToolUse {
                id: "t1".into(),
                name: "search".into(),
                input: json!({ "q": "subtopic" }),
            },
            ContentBlock::Text {
                text: "child subtopic done".into(),
            },
        ],
    ))
}

/// TEST-2 (acceptance, INV-2): a fan-out child persists a `subagent` run row whose
/// activity timeline carries thinking + tool + message, read back via the list +
/// detail endpoints.
#[tokio::test]
async fn fan_out_child_transcript_persists_and_reads_back() {
    let server = plain_server().await;
    let user = workflow_user(&server, "sub_inv2").await;
    let owner = Uuid::parse_str(&user.user_id).unwrap();
    let pool = db_pool(&server).await;

    let conv = make_conversation(&pool, owner).await;
    let parent_msg = Uuid::new_v4();
    let factory = ChatChildSinkFactory::new(pool.clone(), owner, Some(conv), parent_msg, None);

    // The real fan-out server path: create the child row + get its persisting sink,
    // stream the child's own transcript, then settle it terminal.
    let child_id = Uuid::new_v4();
    let sink = factory.for_child(child_id, "Research subtopic A").await;
    sink.emit(child_turn_event()).await;
    factory.settle_child(child_id, true).await;

    let client = reqwest::Client::new();

    // LIST by parent message id.
    let list: Value = client
        .get(server.api_url(&format!("/subagent-runs?parent_message_id={parent_msg}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let children = list["children"].as_array().expect("children[]");
    assert_eq!(children.len(), 1, "the one spawned child is listed");
    assert_eq!(children[0]["id"].as_str().unwrap(), child_id.to_string());
    assert_eq!(
        children[0]["label"].as_str().unwrap(),
        "Research subtopic A"
    );
    assert_eq!(children[0]["status"].as_str().unwrap(), "completed");

    // DETAIL with the full transcript.
    let detail: Value = client
        .get(server.api_url(&format!("/subagent-runs/{child_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let kinds: Vec<&str> = detail["activity"]
        .as_array()
        .expect("activity[]")
        .iter()
        .map(|a| a["kind"].as_str().unwrap_or(""))
        .collect();
    assert!(
        kinds.contains(&"thinking"),
        "full per-child transcript: {kinds:?}"
    );
    assert!(
        kinds.contains(&"tool_call"),
        "full per-child transcript: {kinds:?}"
    );
    assert!(
        kinds.contains(&"message"),
        "full per-child transcript: {kinds:?}"
    );
}

/// TEST-4 (acceptance, INV-4): a foreign user cannot read another user's child —
/// list is empty, detail is 404.
#[tokio::test]
async fn subagent_reads_are_owner_scoped() {
    let server = plain_server().await;
    let owner = workflow_user(&server, "sub_owner").await;
    let other = workflow_user(&server, "sub_other").await;
    let owner_uuid = Uuid::parse_str(&owner.user_id).unwrap();
    let pool = db_pool(&server).await;

    let conv = make_conversation(&pool, owner_uuid).await;
    let parent_msg = Uuid::new_v4();
    let factory = ChatChildSinkFactory::new(pool.clone(), owner_uuid, Some(conv), parent_msg, None);
    let child_id = Uuid::new_v4();
    let _ = factory.for_child(child_id, "owned child").await;

    let client = reqwest::Client::new();

    // The other user's DETAIL request → 404 (never the row).
    let detail = client
        .get(server.api_url(&format!("/subagent-runs/{child_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        detail.status(),
        404,
        "a foreign child id is 404, never leaked"
    );

    // The other user's LIST for the same parent message → empty.
    let list: Value = client
        .get(server.api_url(&format!("/subagent-runs?parent_message_id={parent_msg}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        list["children"].as_array().unwrap().is_empty(),
        "a foreign user sees none of the owner's children"
    );
}

/// TEST-8 (ITEM-6): deleting the parent CONVERSATION cascade-deletes its fan-out
/// child runs (the `parent_conversation_id` FK ON DELETE CASCADE — the DEC-3
/// lifecycle guarantee).
#[tokio::test]
async fn deleting_conversation_cascades_child_runs() {
    let server = plain_server().await;
    let user = workflow_user(&server, "sub_cascade").await;
    let owner = Uuid::parse_str(&user.user_id).unwrap();
    let pool = db_pool(&server).await;

    let conv = make_conversation(&pool, owner).await;
    let parent_msg = Uuid::new_v4();
    let factory = ChatChildSinkFactory::new(pool.clone(), owner, Some(conv), parent_msg, None);
    let child_id = Uuid::new_v4();
    let _ = factory.for_child(child_id, "child to cascade").await;

    let count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_runs WHERE id = $1")
        .bind(child_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count_before, 1,
        "child row exists before the conversation delete"
    );

    sqlx::query("DELETE FROM conversations WHERE id = $1")
        .bind(conv)
        .execute(&pool)
        .await
        .unwrap();

    let count_after: i64 = sqlx::query_scalar("SELECT count(*) FROM workflow_runs WHERE id = $1")
        .bind(child_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count_after, 0,
        "the child run cascade-deletes with its conversation"
    );
}

/// TEST-9 (ITEM-7): `insert_subagent_child_run` writes a `subagent` row linked to
/// the parent message; `set_run_status` flips it terminal.
#[tokio::test]
async fn insert_and_settle_child_run() {
    let server = plain_server().await;
    let user = workflow_user(&server, "sub_insert").await;
    let owner = Uuid::parse_str(&user.user_id).unwrap();
    let pool = db_pool(&server).await;

    let conv = make_conversation(&pool, owner).await;
    let parent_msg = Uuid::new_v4();
    let child_id = Uuid::new_v4();

    insert_subagent_child_run(
        &pool,
        child_id,
        parent_msg,
        Some(conv),
        owner,
        None,
        "labelled child",
    )
    .await
    .expect("insert child run");

    let (kind, status, pmsg): (String, String, Option<Uuid>) = sqlx::query_as(
        "SELECT job_kind, status, parent_message_id FROM workflow_runs WHERE id = $1",
    )
    .bind(child_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "subagent");
    assert_eq!(status, "running", "a fresh child starts running");
    assert_eq!(pmsg, Some(parent_msg), "linked to the parent message");

    set_run_status(&pool, child_id, "completed").await.unwrap();
    let status2: String = sqlx::query_scalar("SELECT status FROM workflow_runs WHERE id = $1")
        .bind(child_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status2, "completed", "settle flips the child terminal");
}

/// TEST-10 (ITEM-10): the notify-and-refetch contract for a child settle.
/// `settle_child` marks the child terminal AND emits an owner-scoped `WorkflowRun`
/// sync (`SyncAction::Update`, via the SAME `emit_workflow_run` the background-run
/// terminal path uses). The sync payload is notify-only, so the SUBSTANTIVE,
/// deterministically-testable half is the refetch TARGET: after settle, the OWNER's
/// refetch endpoint shows the child terminal, and a foreign user — never in the
/// owner audience, so never notified — sees nothing. (Live SSE frame delivery is
/// exercised by the real-fan-out e2e; capturing an in-process axum SSE `Event`
/// frame here is not feasible — see DRIFT-1.)
#[tokio::test]
async fn child_settle_is_owner_scoped_and_refetchable() {
    let server = plain_server().await;
    let user = workflow_user(&server, "sub_sync_owner").await;
    let other = workflow_user(&server, "sub_sync_other").await;
    let owner = Uuid::parse_str(&user.user_id).unwrap();
    let pool = db_pool(&server).await;

    let conv = make_conversation(&pool, owner).await;
    let parent_msg = Uuid::new_v4();
    let factory = ChatChildSinkFactory::new(pool.clone(), owner, Some(conv), parent_msg, None);
    let child_id = Uuid::new_v4();

    let _ = factory.for_child(child_id, "sync child").await;
    factory.settle_child(child_id, true).await; // emits the owner-scoped WorkflowRun/Update

    let client = reqwest::Client::new();

    // OWNER refetches the sync target → the child is terminal (completed).
    let detail: Value = client
        .get(server.api_url(&format!("/subagent-runs/{child_id}")))
        .header("Authorization", format!("Bearer {}", user.token))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        detail["status"].as_str().unwrap(),
        "completed",
        "settle marked the child terminal on the owner-visible refetch endpoint"
    );

    // A foreign user (never in the owner audience, so never notified) cannot refetch it.
    let foreign = client
        .get(server.api_url(&format!("/subagent-runs/{child_id}")))
        .header("Authorization", format!("Bearer {}", other.token))
        .send()
        .await
        .unwrap();
    assert_eq!(
        foreign.status(),
        404,
        "the sync + its refetch are strictly owner-scoped"
    );
}

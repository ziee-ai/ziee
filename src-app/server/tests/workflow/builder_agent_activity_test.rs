//! TEST-8 — `repository::append_agent_activity` is a seq-based bounded ring.
//!
//! A `kind: agent` step can emit thousands of activity entries; each is durably
//! appended to `workflow_runs.step_logs_json["<step_id>::agent_activity"]` via one
//! atomic UPDATE that trims to the `AGENT_ACTIVITY_MAX_ENTRIES` (500) HIGHEST-`seq`
//! entries and re-emits them in ascending `seq`. This drives the real repo fn
//! N > 500 times and asserts the cap + the highest-seq retention + ascending order.
//! No LLM / API key — a minimal run row is inserted directly, then the fn is called.

use serde_json::{json, Value as Json};
use uuid::Uuid;

use ziee::workflow::{append_agent_activity, AGENT_ACTIVITY_MAX_ENTRIES};

use super::{db_pool, import_dev_workflow, plain_server, workflow_user, SIMPLE_OK_YAML};

#[tokio::test]
async fn append_agent_activity_caps_at_500_keeping_highest_seq_ascending() {
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_agent_activity").await;
    // A real workflow row (for the run's workflow_id FK).
    let wf = import_dev_workflow(&server, &user.token, "agent-activity", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().expect("workflow id")).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let pool = db_pool(&server).await;

    // Minimal run fixture (mirrors status_machine.rs's direct-SQL insert).
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, 'running') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&pool)
    .await
    .expect("insert run row");

    let step_id = "think";
    let key = format!("{step_id}::agent_activity");

    // Append N > 500 entries with strictly-increasing seq 1..=N.
    let n: u64 = (AGENT_ACTIVITY_MAX_ENTRIES as u64) + 20; // 520
    for seq in 1..=n {
        let entry = json!({
            "seq": seq,
            "kind": "message",
            "title": format!("entry {seq}"),
            "status": "ok"
        });
        append_agent_activity(&pool, run_id, step_id, &entry)
            .await
            .expect("append agent activity");
    }

    // Read back the persisted ring.
    let logs: Json = sqlx::query_scalar::<_, Json>(
        "SELECT step_logs_json FROM workflow_runs WHERE id = $1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .expect("read step_logs_json");

    let arr = logs[&key].as_array().expect("agent_activity array present");

    // Capped at AGENT_ACTIVITY_MAX_ENTRIES.
    assert_eq!(
        arr.len() as i64,
        AGENT_ACTIVITY_MAX_ENTRIES,
        "ring is capped at {AGENT_ACTIVITY_MAX_ENTRIES}: got {}",
        arr.len()
    );

    // Retains the HIGHEST-seq window: with seq 1..=520 kept-500 = 21..=520.
    let first_seq = arr.first().expect("first")["seq"].as_u64().expect("first seq");
    let last_seq = arr.last().expect("last")["seq"].as_u64().expect("last seq");
    let expected_first = n - (AGENT_ACTIVITY_MAX_ENTRIES as u64) + 1; // 21
    assert_eq!(first_seq, expected_first, "lowest retained seq is {expected_first}");
    assert_eq!(last_seq, n, "highest retained seq is {n}");

    // Ascending, contiguous seq order.
    let seqs: Vec<u64> = arr
        .iter()
        .map(|e| e["seq"].as_u64().expect("seq"))
        .collect();
    let mut expected = seqs.clone();
    expected.sort_unstable();
    assert_eq!(seqs, expected, "entries are in ascending seq order");
    assert!(
        seqs.windows(2).all(|w| w[1] == w[0] + 1),
        "retained window is contiguous (no dropped-then-kept gaps): {seqs:?}"
    );
}

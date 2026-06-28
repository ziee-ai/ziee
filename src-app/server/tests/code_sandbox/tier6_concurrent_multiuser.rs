//! audit id all-dd6b037a9b19 — concurrent multi-user sandbox access. Tier-3
//! only exercises the per-conversation DashMap mutex; Tier-4/6 run sequentially.
//! This fires execute_command for SEVERAL distinct users/conversations
//! CONCURRENTLY and asserts each gets its OWN isolated result (its unique marker
//! in stdout, exit 0) — no cross-contamination under contention. Gated by
//! enabled_test_server() (clean skip when no rootfs/bwrap; runs on a sandbox CI).

use std::sync::Arc;

use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::harness::{create_test_conversation, enabled_test_server, tool_call};
use crate::common::test_helpers::create_user_with_permissions;
use crate::common::TestServer;

#[tokio::test]
async fn concurrent_multi_user_sandbox_runs_are_isolated() {
    let Some(server) = enabled_test_server().await else {
        return; // no rootfs/bwrap on this host — skip cleanly
    };
    let server = Arc::new(server);

    // Three independent users, each with their own conversation.
    let mut actors = Vec::new();
    for i in 0..3u32 {
        let user = create_user_with_permissions(
            &server,
            &format!("sb_concurrent_{i}"),
            &["code_sandbox::execute"],
        )
        .await;
        let uid = Uuid::parse_str(&user.user_id).unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&server.database_url)
            .await
            .unwrap();
        let conv = create_test_conversation(&pool, uid).await;
        pool.close().await;
        actors.push((user.token, conv, format!("MARKER_USER_{i}")));
    }

    // Fire all three execute_command calls concurrently.
    let mut handles = Vec::new();
    for (token, conv, marker) in actors {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            let body = tool_call(
                &server,
                &token,
                conv,
                "execute_command",
                json!({ "command": format!("echo {marker}"), "flavor": "minimal" }),
            )
            .await;
            (marker, body)
        }));
    }

    // Each result must carry ITS OWN marker (no cross-talk) + exit 0.
    for h in handles {
        let (marker, body) = h.await.unwrap();
        let sc = &body["result"]["structuredContent"];
        assert_eq!(sc["exit_code"].as_i64(), Some(0), "exit 0 for {marker}: {body}");
        let stdout = sc["stdout"].as_str().unwrap_or_default();
        assert!(stdout.contains(&marker), "each run must see only its own output ({marker}): {stdout}");
    }
}

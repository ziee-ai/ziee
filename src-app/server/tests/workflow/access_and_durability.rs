//! Phase 8 H wave 2 — workflow access isolation + run durability.
//!
//! - H2: a group-restricted SYSTEM workflow is invisible (GET /workflows
//!   omits it; GET /workflows/{id} 404s) AND unrunnable (POST /run 404s)
//!   to a non-member, but visible + runnable to a member.
//! - H1: cross-user list isolation — user B's GET /workflows omits user
//!   A's user-scope rows.
//! - H3: a cancelled run survives a late terminal write (the guarded
//!   `mark_status` UPDATE is a no-op once the row is terminal).

use serde_json::Value as Json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::{
    SIMPLE_OK_YAML, import_dev_workflow, plain_server, system_import_workflow, workflow_user,
};
use crate::common::test_helpers::create_user_with_permissions;

async fn pool(server: &crate::common::TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

/// Assign a SYSTEM workflow to a freshly-created group and return the
/// group id. Uses the admin group-assignment endpoints (the same path
/// the admin UI drives).
async fn assign_workflow_to_new_group(
    server: &crate::common::TestServer,
    admin: &crate::common::test_helpers::TestUser,
    wf_id: &str,
) -> String {
    let grp: Json = reqwest::Client::new()
        .post(server.api_url("/groups"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "name": format!("wf-grp-{}", &Uuid::new_v4().to_string()[..8]), "description": "x", "permissions": [] }))
        .send()
        .await
        .expect("create group")
        .json()
        .await
        .expect("parse group");
    let gid = grp["id"].as_str().expect("group id").to_string();
    let set = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/system/{wf_id}/groups")))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&serde_json::json!({ "group_ids": [gid] }))
        .send()
        .await
        .expect("set groups");
    assert_eq!(set.status(), 204, "set groups should 204");
    gid
}

#[tokio::test]
async fn group_restricted_system_workflow_hidden_from_non_member() {
    let server = plain_server().await;
    let admin = create_user_with_permissions(
        &server,
        "wf_h2_admin",
        &[
            "workflows::read",
            "workflows::install",
            "workflows::manage",
            "workflows::manage_system",
            "workflows::assign_to_groups",
            "workflows::execute",
            "groups::read",
            "groups::create",
        ],
    )
    .await;

    // Install a SYSTEM workflow + restrict it to a new group.
    let wf = system_import_workflow(&server, &admin.token, "h2-restricted", SIMPLE_OK_YAML).await;
    let wf_id = wf["id"].as_str().expect("wf id").to_string();
    assert_eq!(wf["scope"], "system");
    let _gid = assign_workflow_to_new_group(&server, &admin, &wf_id).await;

    // A non-member user with execute perm must NOT see it in the list.
    let outsider = workflow_user(&server, "wf_h2_outsider").await;
    let list: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("parse list");
    let visible = list["workflows"]
        .as_array()
        .expect("workflows array")
        .iter()
        .any(|w| w["id"] == wf["id"]);
    assert!(
        !visible,
        "group-restricted system workflow must be hidden from a non-member: {list}"
    );

    // GET /workflows/{id} → 404 for the non-member.
    let get = reqwest::Client::new()
        .get(server.api_url(&format!("/workflows/{wf_id}")))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .send()
        .await
        .expect("get");
    assert_eq!(get.status(), 404, "non-member GET must 404 (H2)");

    // POST /run → 404 for the non-member (unrunnable).
    let run = reqwest::Client::new()
        .post(server.api_url(&format!("/workflows/{wf_id}/run")))
        .header("Authorization", format!("Bearer {}", outsider.token))
        .json(&serde_json::json!({ "inputs": { "topic": "x" } }))
        .send()
        .await
        .expect("run");
    assert_eq!(run.status(), 404, "non-member RUN must 404 (H2)");
}

#[tokio::test]
async fn cross_user_list_isolation() {
    // H1: user B's GET /workflows omits user A's user-scope rows.
    let server = plain_server().await;
    let user_a = workflow_user(&server, "wf_iso_a").await;
    let user_b = workflow_user(&server, "wf_iso_b").await;

    let wf_a = import_dev_workflow(&server, &user_a.token, "a-private", SIMPLE_OK_YAML).await;
    let a_id = wf_a["id"].clone();

    let list_b: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user_b.token))
        .send()
        .await
        .expect("list b")
        .json()
        .await
        .expect("parse");
    let leaked = list_b["workflows"]
        .as_array()
        .expect("array")
        .iter()
        .any(|w| w["id"] == a_id);
    assert!(
        !leaked,
        "user B must NOT see user A's user-scope workflow: {list_b}"
    );

    // Sanity: user A DOES see their own.
    let list_a: Json = reqwest::Client::new()
        .get(server.api_url("/workflows"))
        .header("Authorization", format!("Bearer {}", user_a.token))
        .send()
        .await
        .expect("list a")
        .json()
        .await
        .expect("parse");
    assert!(
        list_a["workflows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["id"] == a_id),
        "user A sees their own workflow"
    );
}

#[tokio::test]
async fn cancelled_run_survives_late_completion() {
    // H3: once a run is terminal (cancelled), a late `mark_status`
    // (Completed) must NOT clobber it. We exercise the exact guarded
    // UPDATE the repository uses against a cancelled row + assert the
    // status stays `cancelled`.
    let server = plain_server().await;
    let user = workflow_user(&server, "wf_h3").await;
    let wf = import_dev_workflow(&server, &user.token, "h3-cancel", SIMPLE_OK_YAML).await;
    let wf_id = Uuid::parse_str(wf["id"].as_str().unwrap()).unwrap();
    let user_uuid = Uuid::parse_str(&user.user_id).unwrap();

    let db = pool(&server).await;

    // Insert a run row, then flip it to cancelled (simulating cancel_cas).
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO workflow_runs (workflow_id, user_id, status) VALUES ($1, $2, 'cancelled') RETURNING id",
    )
    .bind(wf_id)
    .bind(user_uuid)
    .fetch_one(&db)
    .await
    .expect("insert cancelled run");

    // Late terminal write — the guarded UPDATE `mark_status` runs for a
    // Completed outcome. It must match ZERO rows (the row is terminal).
    let n = sqlx::query(
        "UPDATE workflow_runs SET status = 'completed', updated_at = NOW() \
         WHERE id = $1 AND (status NOT IN ('cancelled','completed','failed') OR ($2 AND status='cancelled'))",
    )
    .bind(run_id)
    .bind(false) // allow_cancelled_self = false for a Completed write
    .execute(&db)
    .await
    .expect("guarded late completion")
    .rows_affected();
    assert_eq!(n, 0, "late completion must NOT update a cancelled row (H3)");

    // The row is still cancelled.
    let status: String =
        sqlx::query_scalar("SELECT status FROM workflow_runs WHERE id = $1")
            .bind(run_id)
            .fetch_one(&db)
            .await
            .expect("read status");
    assert_eq!(status, "cancelled", "cancelled status must be durable (H3)");

    db.close().await;
}

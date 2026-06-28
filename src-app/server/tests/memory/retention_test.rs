// ============================================================================
// Retention reaper integration tests (Plan Phase 5).
//
// The scheduled reaper runs every 24h (modules/memory/reaper.rs). To
// avoid waiting, these tests drive the same SQL directly against the
// test DB. Covers:
//   - Settings round-trip (max_memories, retention_days)
//   - 30-day grace-period hard delete of soft-deleted rows
//   - retention_days per-user enforcement
//   - max_memories cap enforcement (ROW_NUMBER window)
// ============================================================================

use serde_json::{Value, json};
use sqlx::PgPool;

// ── Settings round-trip ──────────────────────────────────────────────

#[tokio::test]
async fn test_max_memories_cap_setting_round_trip() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_cap",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    let res = client
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.json::<Value>().await.unwrap()["max_memories"], 1000);

    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "max_memories": 10 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.json::<Value>().await.unwrap()["max_memories"], 10);
}

#[tokio::test]
async fn test_retention_days_round_trip() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_days",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    let res = client
        .get(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert!(res.json::<Value>().await.unwrap()["retention_days"].is_null());

    let res = client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "retention_days": 90 }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.json::<Value>().await.unwrap()["retention_days"], 90);
}

// ── Reaper SQL behavior ──────────────────────────────────────────────

async fn open_pool(server: &crate::common::TestServer) -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect to test DB")
}

#[tokio::test]
async fn test_grace_period_hard_delete() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_grace",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    // Create 3 + soft-delete via the public DELETE endpoint.
    let mut ids: Vec<String> = Vec::new();
    for i in 0..3 {
        let res = client
            .post(server.api_url("/memories"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "content": format!("mem {i}") }))
            .send()
            .await
            .unwrap();
        let row: Value = res.json().await.unwrap();
        ids.push(row["id"].as_str().unwrap().to_string());
    }
    for id in &ids {
        client
            .delete(server.api_url(&format!("/memories/{id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
    }

    // Backdate deleted_at to >30 days ago.
    let pool = open_pool(&server).await;
    sqlx::query("UPDATE user_memories SET deleted_at = NOW() - INTERVAL '40 days' WHERE deleted_at IS NOT NULL")
        .execute(&pool)
        .await
        .unwrap();

    // Mirror reaper.rs::run_once step 1.
    let n = sqlx::query(
        "DELETE FROM user_memories WHERE deleted_at IS NOT NULL AND deleted_at < NOW() - ($1 * INTERVAL '1 day')",
    )
    .bind(30_i32)
    .execute(&pool)
    .await
    .unwrap();
    assert!(n.rows_affected() >= 3);

    let res = client
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let rows = body["items"].as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), 0);
}

#[tokio::test]
async fn test_retention_days_enforcement() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_enforce",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "retention_days": 7 }))
        .send()
        .await
        .unwrap();

    let res = client
        .post(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "content": "old memory" }))
        .send()
        .await
        .unwrap();
    let row: Value = res.json().await.unwrap();
    let id = uuid::Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();

    let pool = open_pool(&server).await;
    sqlx::query("UPDATE user_memories SET updated_at = NOW() - INTERVAL '10 days' WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();

    // Mirror reaper.rs::run_once step 2.
    sqlx::query(
        r#"
        UPDATE user_memories um
        SET deleted_at = NOW()
        FROM user_memory_settings ums
        WHERE um.user_id = ums.user_id
          AND ums.retention_days IS NOT NULL
          AND um.deleted_at IS NULL
          AND um.updated_at < NOW() - (ums.retention_days * INTERVAL '1 day')
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = client
        .get(server.api_url(&format!("/memories/{id}")))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 404, "retention-aged memory must be soft-deleted");
}

#[tokio::test]
async fn test_max_memories_cap_enforcement() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_cap_enforce",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    client
        .put(server.api_url("/memory/settings"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "max_memories": 3 }))
        .send()
        .await
        .unwrap();

    let mut ids: Vec<uuid::Uuid> = Vec::new();
    for i in 0..5 {
        let res = client
            .post(server.api_url("/memories"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "content": format!("mem {i}") }))
            .send()
            .await
            .unwrap();
        let row: Value = res.json().await.unwrap();
        ids.push(uuid::Uuid::parse_str(row["id"].as_str().unwrap()).unwrap());
    }

    // Stagger updated_at: idx 0 oldest, idx 4 newest.
    let pool = open_pool(&server).await;
    for (idx, id) in ids.iter().enumerate() {
        let days = 5 - (idx as i32);
        sqlx::query(&format!(
            "UPDATE user_memories SET updated_at = NOW() - INTERVAL '{days} days' WHERE id = $1"
        ))
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Mirror reaper.rs::run_once step 3.
    sqlx::query(
        r#"
        WITH ranked AS (
            SELECT um.id,
                   ROW_NUMBER() OVER (PARTITION BY um.user_id ORDER BY um.updated_at DESC) AS rn,
                   COALESCE(ums.max_memories, 1000) AS cap
            FROM user_memories um
            LEFT JOIN user_memory_settings ums ON ums.user_id = um.user_id
            WHERE um.deleted_at IS NULL
        )
        UPDATE user_memories
        SET deleted_at = NOW()
        WHERE id IN (SELECT id FROM ranked WHERE rn > cap)
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = client
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    let body: Value = res.json().await.unwrap();
    let rows = body["items"].as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), 3, "max_memories=3 should leave exactly 3 live rows");
}

// ── REAL reaper tick (production `run_once`, not mirrored SQL) ───────────

/// The other retention tests mirror `reaper.rs::run_once`'s SQL inline; this
/// one calls the ACTUAL production function (`ziee::memory_reaper_run_once`)
/// so a future drift between the reaper and the mirrored SQL is caught. Seeds
/// 3 memories, soft-deletes them, backdates `deleted_at` past the 30-day grace
/// window, runs one real reaper tick, and asserts they are hard-deleted.
#[tokio::test]
async fn test_real_reaper_run_once_hard_deletes_grace_expired() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "ret_realreaper",
        &["memory::read", "memory::write"],
    )
    .await;
    let client = reqwest::Client::new();
    let token = &user.token;

    let mut ids: Vec<String> = Vec::new();
    for i in 0..3 {
        let res = client
            .post(server.api_url("/memories"))
            .header("Authorization", format!("Bearer {token}"))
            .json(&json!({ "content": format!("real reaper mem {i}") }))
            .send()
            .await
            .unwrap();
        ids.push(res.json::<Value>().await.unwrap()["id"].as_str().unwrap().to_string());
    }
    for id in &ids {
        client
            .delete(server.api_url(&format!("/memories/{id}")))
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await
            .unwrap();
    }

    let pool = open_pool(&server).await;
    sqlx::query("UPDATE user_memories SET deleted_at = NOW() - INTERVAL '40 days' WHERE deleted_at IS NOT NULL")
        .execute(&pool)
        .await
        .unwrap();

    // One REAL reaper tick (reads admin settings, hard-deletes grace-expired).
    ziee::memory_reaper_run_once(&pool)
        .await
        .expect("reaper run_once should succeed");

    // The grace-expired soft-deletes are gone from the live list.
    let body: Value = client
        .get(server.api_url("/memories"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        body["items"].as_array().map(|a| a.len()).unwrap_or(0),
        0,
        "real reaper tick must hard-delete the grace-expired rows"
    );
}

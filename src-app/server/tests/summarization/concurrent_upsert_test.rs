use uuid::Uuid;
use crate::common::TestServer;
use serde_json::Value;
use serde_json::json;
use sqlx::PgPool;

#[tokio::test]
async fn concurrent_summary_upserts_converge_to_one_row() {
    let server = TestServer::start().await;
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "summ_race", &[]).await;
    let uid = Uuid::parse_str(&user.user_id).unwrap();

    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let conv_id = Uuid::new_v4();
    let branch_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO conversations (id, user_id, title, active_branch_id, created_at, updated_at)
           VALUES ($1, $2, 'race', NULL, NOW(), NOW())"#,
    )
    .bind(conv_id)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO branches (id, conversation_id, parent_branch_id, created_from_message_id, created_at)
           VALUES ($1, $2, NULL, NULL, NOW())"#,
    )
    .bind(branch_id)
    .bind(conv_id)
    .execute(&pool)
    .await
    .unwrap();

    // Many concurrent upserts on the SAME branch, each with distinct text.
    let mut handles = Vec::new();
    for i in 0..6 {
        let pool = pool.clone();
        let text = format!("summary-{i}");
        handles.push(tokio::spawn(async move {
            sqlx::query(
                r#"INSERT INTO conversation_summaries
                       (branch_id, summary_text, message_count, model_used)
                   VALUES ($1, $2, 3, 'm')
                   ON CONFLICT (branch_id) DO UPDATE
                   SET summary_text = EXCLUDED.summary_text, updated_at = NOW()"#,
            )
            .bind(branch_id)
            .bind(text)
            .execute(&pool)
            .await
        }));
    }
    for h in handles {
        h.await
            .unwrap()
            .expect("concurrent upsert must not error on the PK conflict");
    }

    // Exactly one row survives — last-write-wins, no duplicate from the race.
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM conversation_summaries WHERE branch_id = $1")
            .bind(branch_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count.0, 1, "concurrent upserts must converge to one row");

    let text: (String,) =
        sqlx::query_as("SELECT summary_text FROM conversation_summaries WHERE branch_id = $1")
            .bind(branch_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        text.0.starts_with("summary-"),
        "surviving row carries one writer's text: {}",
        text.0
    );
    pool.close().await;
}

async fn create_conversation(server: &crate::common::TestServer, token: &str) -> Uuid {
    let res = reqwest::Client::new()
        .post(server.api_url("/conversations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "title": "concurrent summary" }))
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success(), "create conversation: {}", res.status());
    let row: Value = res.json().await.unwrap();
    Uuid::parse_str(row["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn test_concurrent_summary_upsert_converges_to_one_row() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "summ_race",
        &["conversations::create", "conversations::read"],
    )
    .await;
    let conv_id = create_conversation(&server, &user.token).await;

    let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&server.database_url)
        .await
        .unwrap();
    let branch_id: Uuid = sqlx::query_scalar(
        "SELECT active_branch_id FROM conversations WHERE id = $1",
    )
    .bind(conv_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // N racers each upsert a DISTINCT summary for the same branch.
    let mut handles = Vec::new();
    for i in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            sqlx::query(
                "INSERT INTO conversation_summaries \
                   (branch_id, summary_text, summarized_up_to_id, message_count, model_used) \
                 VALUES ($1, $2, NULL, $3, $4) \
                 ON CONFLICT (branch_id) DO UPDATE SET \
                   summary_text = EXCLUDED.summary_text, \
                   message_count = EXCLUDED.message_count, \
                   model_used = EXCLUDED.model_used, \
                   updated_at = NOW()",
            )
            .bind(branch_id)
            .bind(format!("summary variant {i}"))
            .bind(i as i32)
            .bind(format!("model-{i}"))
            .execute(&pool)
            .await
        }));
    }
    for h in handles {
        // Every racer must succeed — the ON CONFLICT makes the upsert race-safe
        // (no duplicate-key violation).
        h.await.unwrap().expect("concurrent upsert must not error");
    }

    // Exactly one row survives, and it is one of the racers' values.
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_summaries WHERE branch_id = $1",
    )
    .bind(branch_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "concurrent upserts must converge to a single row");

    let surviving: String = sqlx::query_scalar(
        "SELECT summary_text FROM conversation_summaries WHERE branch_id = $1",
    )
    .bind(branch_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        surviving.starts_with("summary variant "),
        "surviving row holds a racer's value (last-write-wins): {surviving}"
    );
    pool.close().await;
}


//! Conversation sort integration tests (ITEM-5 / TEST-3).
//!
//! The list endpoint's `sort` param orders by recent/oldest/alpha/most_messages,
//! and an unknown value falls back to `recent`.

use reqwest::StatusCode;
use uuid::Uuid;

use super::helpers;

async fn set_updated_at(database_url: &str, conversation_id: Uuid, iso: &str) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .unwrap();
    sqlx::query("UPDATE conversations SET updated_at = $1::timestamptz WHERE id = $2")
        .bind(iso)
        .bind(conversation_id)
        .execute(&pool)
        .await
        .expect("set updated_at");
    pool.close().await;
}

async fn list_titles_sorted(
    server: &crate::common::TestServer,
    token: &str,
    sort: &str,
) -> Vec<String> {
    let response = reqwest::Client::new()
        .get(server.api_url("/conversations"))
        .query(&[("sort", sort)])
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    body["conversations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap_or("").to_string())
        .collect()
}

#[tokio::test]
async fn test_sort_orders_by_each_key() {
    let server = crate::common::TestServer::start().await;
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "user",
        &["conversations::create", "conversations::read"],
    )
    .await;

    // Three conversations with controlled updated_at + message counts.
    let apple = helpers::create_conversation(&server, &user.token, None, Some("Apple")).await;
    let banana = helpers::create_conversation(&server, &user.token, None, Some("Banana")).await;
    let cherry = helpers::create_conversation(&server, &user.token, None, Some("Cherry")).await;

    let apple_id = helpers::parse_uuid(&apple["id"]);
    let banana_id = helpers::parse_uuid(&banana["id"]);
    let cherry_id = helpers::parse_uuid(&cherry["id"]);

    // updated_at: Cherry oldest → Apple → Banana newest.
    set_updated_at(&server.database_url, cherry_id, "2020-01-01T00:00:00Z").await;
    set_updated_at(&server.database_url, apple_id, "2021-01-01T00:00:00Z").await;
    set_updated_at(&server.database_url, banana_id, "2022-01-01T00:00:00Z").await;

    // message counts: Apple 3, Cherry 2, Banana 1.
    let apple_branch = helpers::parse_uuid(&apple["active_branch_id"]);
    let banana_branch = helpers::parse_uuid(&banana["active_branch_id"]);
    let cherry_branch = helpers::parse_uuid(&cherry["active_branch_id"]);
    for i in 0..3 {
        helpers::seed_text_message(&server.database_url, apple_branch, "user", &format!("a{i}"))
            .await;
    }
    for i in 0..2 {
        helpers::seed_text_message(&server.database_url, cherry_branch, "user", &format!("c{i}"))
            .await;
    }
    helpers::seed_text_message(&server.database_url, banana_branch, "user", "b0").await;

    // recent (default): newest updated first.
    assert_eq!(
        list_titles_sorted(&server, &user.token, "recent").await,
        vec!["Banana", "Apple", "Cherry"],
    );
    // oldest: oldest updated first.
    assert_eq!(
        list_titles_sorted(&server, &user.token, "oldest").await,
        vec!["Cherry", "Apple", "Banana"],
    );
    // alpha: title A→Z.
    assert_eq!(
        list_titles_sorted(&server, &user.token, "alpha").await,
        vec!["Apple", "Banana", "Cherry"],
    );
    // most_messages: highest count first (Apple 3, Cherry 2, Banana 1).
    assert_eq!(
        list_titles_sorted(&server, &user.token, "most_messages").await,
        vec!["Apple", "Cherry", "Banana"],
    );
    // unknown sort falls back to recent.
    assert_eq!(
        list_titles_sorted(&server, &user.token, "not-a-real-sort").await,
        vec!["Banana", "Apple", "Cherry"],
        "unknown sort must behave like recent",
    );
}

//! web_search built-in MCP row consistency across restart / re-register
//! (gap e7e87730f310). `WebSearchModule::init` upserts the built-in server in a
//! `tokio::spawn` with `ON CONFLICT (id) DO UPDATE`. These assert the upsert is
//! idempotent (a re-register on the next boot leaves exactly one row) and that
//! a lost row is re-created — modeling repeated server starts.

use uuid::Uuid;

use crate::common::TestServer;
use ziee::web_search::{web_search_server_id, WebSearchRepository};

const LOOPBACK_URL: &str = "http://127.0.0.1:9999/api/web-search/mcp";

async fn pool(server: &TestServer) -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&server.database_url)
        .await
        .expect("connect test db")
}

async fn wait_for_row(pool: &sqlx::PgPool, id: Uuid) {
    for _ in 0..40 {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("count web_search built-in row");
        if count >= 1 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("web_search built-in row never registered at boot within ~10s");
}

#[tokio::test]
async fn web_search_re_register_is_idempotent_single_row() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = web_search_server_id();
    wait_for_row(&pool, id).await;

    // Simulate two more boots re-registering on top of the boot row.
    let repo = WebSearchRepository::new(pool.clone());
    repo.upsert_builtin_server(id, LOOPBACK_URL).await.expect("re-register 1");
    repo.upsert_builtin_server(id, LOOPBACK_URL).await.expect("re-register 2");

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "repeated re-registration must leave exactly one row");
    pool.close().await;
}

#[tokio::test]
async fn web_search_re_register_recreates_a_lost_row() {
    let server = TestServer::start().await;
    let pool = pool(&server).await;
    let id = web_search_server_id();
    wait_for_row(&pool, id).await;

    sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .expect("delete row");

    let repo = WebSearchRepository::new(pool.clone());
    repo.upsert_builtin_server(id, LOOPBACK_URL).await.expect("recovery upsert");

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "a lost built-in row is re-created on the next register");
    pool.close().await;
}

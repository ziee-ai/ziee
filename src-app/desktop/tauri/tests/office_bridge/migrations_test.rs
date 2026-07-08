//! TEST-6 [ITEM-5] — the office_bridge migrations, now in the desktop `1000…`
//! space, apply in the desktop build: `office_bridge_settings` exists and the
//! default Users group carries the `office_bridge::use` grant.

use sqlx::postgres::PgPoolOptions;

async fn pool_for(server: &crate::common::TestServer) -> sqlx::PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&server.database_url)
        .await
        .expect("connect test DB")
}

#[tokio::test]
async fn test6_desktop_migrations_create_office_bridge_schema_and_grant() {
    let server = crate::common::TestServer::start_desktop().await;
    let pool = pool_for(&server).await;

    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables \
         WHERE table_name = 'office_bridge_settings')",
    )
    .fetch_one(&pool)
    .await
    .expect("query office_bridge_settings existence");
    assert!(
        table_exists,
        "office_bridge_settings must exist after the desktop migrations run"
    );

    let granted: bool = sqlx::query_scalar(
        "SELECT 'office_bridge::use' = ANY(permissions) FROM groups \
         WHERE name = 'Users' AND is_system = TRUE AND is_default = TRUE",
    )
    .fetch_one(&pool)
    .await
    .expect("query Users group grant");
    assert!(
        granted,
        "the default Users group must carry the office_bridge::use grant"
    );
}

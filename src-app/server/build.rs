use std::env;

#[tokio::main]
async fn main() {
    // Get DATABASE_URL or use default
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@127.0.0.1:54321/postgres".to_string());

    println!("cargo:rerun-if-env-changed=DATABASE_URL");

    // Connect to the database
    let pool = match sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("\nERROR: Failed to connect to database: {}", e);
            eprintln!("DATABASE_URL: {}", database_url);
            panic!("Database connection failed");
        }
    };

    // Wipe the database
    sqlx::query("DROP SCHEMA IF EXISTS public CASCADE")
        .execute(&pool)
        .await
        .ok();

    sqlx::query("CREATE SCHEMA public")
        .execute(&pool)
        .await
        .expect("Failed to create schema");

    sqlx::query("GRANT ALL ON SCHEMA public TO PUBLIC")
        .execute(&pool)
        .await
        .ok();

    // Run migrations
    let migrations_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let migrator = sqlx::migrate::Migrator::new(migrations_path)
        .await
        .expect("Failed to create migrator");

    if let Err(e) = migrator.run(&pool).await {
        eprintln!("\nERROR: Migration failed: {}", e);
        panic!("Migration failed");
    }

    pool.close().await;

    // Set DATABASE_URL for SQLx compile-time verification
    println!("cargo:rustc-env=DATABASE_URL={}", database_url);
}

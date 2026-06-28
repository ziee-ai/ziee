// Integration tests for LLM Provider Files Module
//
// These tests verify the file caching and provider file mapping functionality.
// Since this is an integration test, we test the database operations directly
// without relying on internal module imports.

use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test file in the database
async fn create_test_file(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    filename: &str,
) -> Uuid {
    let file_id = Uuid::new_v4();

    // A `files` row needs a v1 `file_versions` head (current_version_id is NOT
    // NULL since the versioning migration). The two FKs are circular, so insert
    // both in one transaction — the current_version_id FK is DEFERRABLE INITIALLY
    // DEFERRED, checked at COMMIT once both rows exist. Mirrors Repos.file.create.
    let mut tx = pool.begin().await.expect("begin tx");
    sqlx::query!(
        r#"
        INSERT INTO files (
            id, user_id, filename, file_size, mime_type, checksum,
            has_thumbnail, preview_page_count, text_page_count, processing_metadata,
            current_version_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $1)
        "#,
        file_id,
        user_id,
        filename,
        1024i64,
        Some("text/plain"),
        "test_checksum",
        false,
        0i32,  // preview_page_count is NOT NULL
        1i32,  // text_page_count is NOT NULL
        json!({})
    )
    .execute(&mut *tx)
    .await
    .expect("Failed to create test file");
    sqlx::query!(
        r#"
        INSERT INTO file_versions (
            id, file_id, version, is_head, blob_version_id, file_size, mime_type,
            checksum, has_thumbnail, preview_page_count, text_page_count,
            processing_metadata, source_message_id, created_by
        )
        VALUES ($1, $1, 1, true, $1, $2, $3, $4, $5, $6, $7, $8, NULL, 'user')
        "#,
        file_id,
        1024i64,
        Some("text/plain"),
        "test_checksum",
        false,
        0i32,
        1i32,
        json!({})
    )
    .execute(&mut *tx)
    .await
    .expect("Failed to create test file v1 version");
    tx.commit().await.expect("commit tx");

    file_id
}

/// Create a test provider in the database
async fn create_test_provider(
    pool: &sqlx::PgPool,
    name: &str,
    provider_type: &str,
) -> Uuid {
    let provider_id = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO llm_providers (
            id, name, provider_type, enabled, api_key, base_url
        )
        VALUES ($1, $2, $3, true, $4, $5)
        "#,
        provider_id,
        name,
        provider_type,
        Some("test_api_key"),
        Some("https://api.test.com/v1")
    )
    .execute(pool)
    .await
    .expect("Failed to create test provider");

    provider_id
}

// ============================================================================
// Repository Tests - Basic CRUD
// ============================================================================

#[tokio::test]
async fn test_create_provider_file_mapping() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    // Create a test user
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");

    // Create test data
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "anthropic").await;

    // Create mapping
    let provider_file_id = "file_abc123".to_string();
    let metadata = json!({
        "uploaded_at": Utc::now().to_rfc3339(),
        "filename": "test.pdf"
    });

    let mapping = sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        ON CONFLICT (file_id, provider_id) DO UPDATE SET
            provider_file_id = EXCLUDED.provider_file_id,
            provider_metadata = EXCLUDED.provider_metadata,
            upload_status = 'completed',
            updated_at = NOW()
        RETURNING id, file_id, provider_id, provider_file_id, upload_status
        "#,
        file_id,
        provider_id,
        provider_file_id,
        metadata
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create mapping");

    assert_eq!(mapping.file_id, file_id);
    assert_eq!(mapping.provider_id, provider_id);
    assert_eq!(mapping.provider_file_id, Some(provider_file_id));
    assert_eq!(mapping.upload_status, "completed");
}

#[tokio::test]
async fn test_get_provider_file_mapping() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "gemini").await;

    // Create mapping
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_xyz789",
        json!({})
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Get mapping
    let mapping = sqlx::query!(
        r#"
        SELECT id, file_id, provider_id, provider_file_id, upload_status
        FROM llm_provider_files
        WHERE file_id = $1 AND provider_id = $2
        "#,
        file_id,
        provider_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to get mapping")
    .expect("Mapping not found");

    assert_eq!(mapping.file_id, file_id);
    assert_eq!(mapping.provider_id, provider_id);
    assert_eq!(mapping.provider_file_id, Some("file_xyz789".to_string()));
}

#[tokio::test]
async fn test_upsert_updates_existing_mapping() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "openai").await;

    // Create initial mapping
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_old123",
        json!({"version": 1})
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Update mapping with UPSERT
    let updated = sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        ON CONFLICT (file_id, provider_id) DO UPDATE SET
            provider_file_id = EXCLUDED.provider_file_id,
            provider_metadata = EXCLUDED.provider_metadata,
            updated_at = NOW()
        RETURNING id, provider_file_id, provider_metadata
        "#,
        file_id,
        provider_id,
        "file_new456",
        json!({"version": 2})
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to update mapping");

    assert_eq!(updated.provider_file_id, Some("file_new456".to_string()));
    assert_eq!(updated.provider_metadata["version"], 2);
}

#[tokio::test]
async fn test_concurrent_upserts_converge_to_single_row() {
    // The mapping upsert is documented "idempotent and safe for concurrent
    // calls" (ON CONFLICT (file_id, provider_id)). Fire many concurrent upserts
    // for the SAME (file_id, provider_id) and assert: none errors with a
    // duplicate-key violation, exactly ONE row survives, and its
    // provider_file_id is one of the racing writers' values (last-writer-wins).
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("connect test db");
    let user = crate::common::test_helpers::create_user_with_permissions(&server, "race_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();
    let file_id = create_test_file(&pool, user_id, "race.pdf").await;
    let provider_id = create_test_provider(&pool, "Race Provider", "openai").await;

    const N: usize = 12;
    let mut handles = Vec::with_capacity(N);
    for i in 0..N {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            sqlx::query(
                r#"
                INSERT INTO llm_provider_files (
                    file_id, provider_id, provider_file_id, provider_metadata, upload_status
                )
                VALUES ($1, $2, $3, $4, 'completed')
                ON CONFLICT (file_id, provider_id) DO UPDATE SET
                    provider_file_id = EXCLUDED.provider_file_id,
                    provider_metadata = EXCLUDED.provider_metadata,
                    upload_status = 'completed',
                    updated_at = NOW()
                "#,
            )
            .bind(file_id)
            .bind(provider_id)
            .bind(format!("file_race_{i}"))
            .bind(json!({ "writer": i }))
            .execute(&pool)
            .await
        }));
    }
    for h in handles {
        h.await.unwrap().expect("concurrent upsert must not error (no duplicate-key violation)");
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
    )
    .bind(file_id)
    .bind(provider_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "concurrent upserts must converge to exactly one row");

    let pfid: Option<String> = sqlx::query_scalar(
        "SELECT provider_file_id FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
    )
    .bind(file_id)
    .bind(provider_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let pfid = pfid.expect("surviving row has a provider_file_id");
    assert!(pfid.starts_with("file_race_"), "survivor is one of the writers: {pfid}");
}

#[tokio::test]
async fn test_delete_provider_file_mapping() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "anthropic").await;

    // Create mapping
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_abc123",
        json!({})
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Delete mapping
    let result = sqlx::query!(
        "DELETE FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
        file_id,
        provider_id
    )
    .execute(&pool)
    .await
    .expect("Failed to delete mapping");

    assert_eq!(result.rows_affected(), 1);

    // Verify it's gone
    let mapping = sqlx::query!(
        "SELECT id FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
        file_id,
        provider_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query mapping");

    assert!(mapping.is_none());
}

// ============================================================================
// Expiration Tests
// ============================================================================

#[tokio::test]
async fn test_file_expiration_check() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Gemini Provider", "gemini").await;

    // Create mapping with past expiration
    let expires_at = Utc::now() - Duration::hours(1);
    let metadata = json!({
        "expires_at": expires_at.to_rfc3339()
    });

    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_expired",
        metadata
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Query for expired files
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM llm_provider_files
        WHERE (provider_metadata->>'expires_at')::TIMESTAMPTZ < NOW()
          AND upload_status = 'completed'
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to query expired files");

    assert!(count >= 1, "Should find at least one expired file");
}

#[tokio::test]
async fn test_cascade_delete_on_file_deletion() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "anthropic").await;

    // Create mapping
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_abc123",
        json!({})
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Delete file
    sqlx::query!("DELETE FROM files WHERE id = $1", file_id)
        .execute(&pool)
        .await
        .expect("Failed to delete file");

    // Verify mapping is also deleted (CASCADE)
    let mapping = sqlx::query!(
        "SELECT id FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
        file_id,
        provider_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query mapping");

    assert!(mapping.is_none(), "Mapping should be cascade deleted");
}

#[tokio::test]
async fn test_cascade_delete_on_provider_deletion() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user = crate::common::test_helpers::create_user_with_permissions(&server, "test_user", &[])
        .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");
    let file_id = create_test_file(&pool, user_id, "test.pdf").await;
    let provider_id = create_test_provider(&pool, "Test Provider", "anthropic").await;

    // Create mapping
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "file_abc123",
        json!({})
    )
    .execute(&pool)
    .await
    .expect("Failed to create mapping");

    // Delete provider
    sqlx::query!("DELETE FROM llm_providers WHERE id = $1", provider_id)
        .execute(&pool)
        .await
        .expect("Failed to delete provider");

    // Verify mapping is also deleted (CASCADE)
    let mapping = sqlx::query!(
        "SELECT id FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
        file_id,
        provider_id
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to query mapping");

    assert!(mapping.is_none(), "Mapping should be cascade deleted");
}

// ============================================================================
// API-key rotation → cached provider_file_id invalidation
// ============================================================================

/// The cache-reuse path in `get_or_upload_provider_file` keys on an
/// `api_key_fingerprint` stored in `provider_metadata`: on a cache HIT it
/// reuses the mapping ONLY when the stored fingerprint equals the current
/// key's fingerprint, otherwise it treats the cached `provider_file_id` as
/// belonging to a different account and re-uploads. This pins the persisted
/// half of that decision end-to-end: the fingerprint round-trips through the
/// real `llm_provider_files` row, and the same comparison the service performs
/// (`stored != current ⇒ invalidate`) yields reuse for the same key and
/// invalidation after rotation.
#[tokio::test]
async fn test_api_key_fingerprint_persists_and_detects_rotation() {
    use sha2::{Digest, Sha256};

    fn fingerprint(key: &str) -> String {
        let mut h = Sha256::new();
        h.update(key.as_bytes());
        hex::encode(h.finalize())
    }

    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("connect");
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "rotation_user", &[])
            .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("uuid");
    let file_id = create_test_file(&pool, user_id, "doc.pdf").await;
    let provider_id = create_test_provider(&pool, "Rotation Provider", "gemini").await;

    let old_key = "sk-account-A-original";
    // Persist a completed mapping carrying the OLD key's fingerprint — exactly
    // what `save_upload_response` writes into provider_metadata.
    sqlx::query!(
        r#"
        INSERT INTO llm_provider_files (
            file_id, provider_id, provider_file_id,
            provider_metadata, upload_status
        )
        VALUES ($1, $2, $3, $4, 'completed')
        "#,
        file_id,
        provider_id,
        "provider-file-acctA",
        json!({ "api_key_fingerprint": fingerprint(old_key), "filename": "doc.pdf" })
    )
    .execute(&pool)
    .await
    .expect("insert mapping");

    // Read the mapping back as the service does (provider_metadata JSONB).
    let row = sqlx::query!(
        r#"
        SELECT provider_file_id, provider_metadata, upload_status
        FROM llm_provider_files
        WHERE file_id = $1 AND provider_id = $2
        "#,
        file_id,
        provider_id
    )
    .fetch_one(&pool)
    .await
    .expect("fetch mapping");

    let stored = row
        .provider_metadata
        .get("api_key_fingerprint")
        .and_then(|v| v.as_str())
        .expect("fingerprint persisted in provider_metadata");

    // Same key → NOT rotated → the cached provider_file_id is reused.
    let key_rotated_same = stored != fingerprint(old_key);
    assert!(!key_rotated_same, "same key must NOT be flagged as rotated");
    assert_eq!(row.upload_status, "completed");
    assert_eq!(row.provider_file_id.as_deref(), Some("provider-file-acctA"));

    // Rotated key → the stored fingerprint no longer matches → the cached id is
    // invalidated (the service re-uploads instead of returning the stale id).
    let new_key = "sk-account-B-rotated";
    let key_rotated_after = stored != fingerprint(new_key);
    assert!(
        key_rotated_after,
        "rotated key must invalidate the cached provider_file_id"
    );
}

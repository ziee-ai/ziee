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
// Service Tests — retry / re-upload after a provider upload failure
// ============================================================================
//
// Gap (audit id 880298cae9cb): the upload error path at
// `service.rs:118-122` (`upload_file(...).map_err(...)`) and the
// "test-and-validate" re-upload contract were untested. A failed provider
// upload must NOT persist a Completed mapping, and a subsequent call must
// retry the upload (rather than short-circuiting on a stale/half-written
// mapping). This drives the REAL service function `get_or_upload_provider_file`
// end-to-end against a real `FilesystemStorage` blob + a mock `AIProvider`
// whose first `upload_file` fails and second succeeds. Only the external
// provider boundary is mocked.

use std::sync::atomic::{AtomicUsize, Ordering};
use ai_providers::{
    AIProvider, ChatRequest, EmbeddingsRequest, EmbeddingsResponse, FileUpload,
    FileUploadResponse, ProviderError, StreamChatChunk,
};
use futures::Stream;
use std::pin::Pin;

/// Mock provider supporting the file API whose `upload_file` fails on the
/// first invocation and succeeds on every subsequent one.
struct FlakyUploadProvider {
    upload_calls: AtomicUsize,
    succeed_id: String,
}

impl FlakyUploadProvider {
    fn new(succeed_id: &str) -> Self {
        Self {
            upload_calls: AtomicUsize::new(0),
            succeed_id: succeed_id.to_string(),
        }
    }
}

#[ziee::async_trait]
impl AIProvider for FlakyUploadProvider {
    fn name(&self) -> &str {
        "flaky-upload"
    }

    async fn stream_chat(
        &self,
        _api_key: &str,
        _base_url: &str,
        _request: ChatRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        Err(ProviderError::NotSupported("mock: no chat".into()))
    }

    async fn embeddings(
        &self,
        _api_key: &str,
        _base_url: &str,
        _request: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        Err(ProviderError::NotSupported("mock: no embeddings".into()))
    }

    fn supports_file_api(&self) -> bool {
        true
    }

    async fn upload_file(
        &self,
        _api_key: &str,
        _base_url: &str,
        _upload: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        let n = self.upload_calls.fetch_add(1, Ordering::SeqCst);
        if n == 0 {
            // First attempt: simulate an upstream provider failure.
            Err(ProviderError::FileUpload(
                "simulated transient provider failure".into(),
            ))
        } else {
            Ok(Some(FileUploadResponse {
                provider_file_id: self.succeed_id.clone(),
                expires_at: None,
                metadata: None,
            }))
        }
    }
}

#[tokio::test]
async fn test_reupload_after_provider_failure() {
    use std::sync::Arc;
    use ziee::llm_provider_files_test_api::{
        get_or_upload_provider_file, FileRepository, FileStorage, FilesystemStorage, LlmProvider,
        ProxySettings,
    };

// Concurrency — race condition on the idempotent upsert
// ============================================================================

/// `upsert_provider_file_mapping` documents itself as "idempotent and safe for
/// concurrent calls" (repository.rs:46). The safety is delegated to Postgres:
/// the `UNIQUE(file_id, provider_id)` constraint (migration 15) plus the
/// `ON CONFLICT (file_id, provider_id) DO UPDATE` clause must collapse a burst
/// of simultaneous upserts on the SAME key into a single row — no duplicate-key
/// error leaking out, no second row, and a consistent terminal value (one of
/// the racers wins, last-writer-wins on the row).
///
/// This fires N upserts concurrently, each on the same (file_id, provider_id)
/// but with a DISTINCT provider_file_id, from independent pool connections so
/// they genuinely contend at the database (not serialized in one task). It runs
/// the EXACT SQL the repository issues (this integration tier tests the DB
/// operations directly, mirroring the other tests in this file).
#[tokio::test]
async fn test_concurrent_upsert_same_key_is_race_safe() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("Failed to connect to test database");

    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "reupload_user", &[])
            .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");

    // A real file row + its v1 blob on disk so `load_original` succeeds and
    // the upload path is genuinely reached.
    let filename = "retry.txt";
    let file_id = create_test_file(&pool, user_id, filename).await;

    let tmp = std::env::temp_dir().join(format!("ziee-reupload-test-{}", Uuid::new_v4()));
    let storage: Arc<dyn FileStorage> = Arc::new(FilesystemStorage::new(&tmp));
    storage
        .save_original(user_id, file_id, "txt", b"hello provider upload retry")
        .await
        .expect("save blob");

    let file_repo = FileRepository::new(pool.clone());

    let provider = LlmProvider {
        id: Uuid::new_v4(),
        name: "Anthropic Test".to_string(),
        provider_type: "anthropic".to_string(),
        enabled: true,
        api_key: Some("test-api-key-123".to_string()),
        base_url: Some("https://api.test.example/v1".to_string()),
        built_in: false,
        proxy_settings: ProxySettings::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        default_runtime_version_id: None,
    };
    // Persist the provider row so save_upload_response's FK to llm_providers
    // is satisfied on the successful (second) attempt.
    sqlx::query!(
        r#"INSERT INTO llm_providers (id, name, provider_type, enabled, api_key, base_url)
           VALUES ($1, $2, $3, true, $4, $5)"#,
        provider.id,
        provider.name,
        provider.provider_type,
        provider.api_key,
        provider.base_url,
    )
    .execute(&pool)
    .await
    .expect("insert provider");

    let ai = FlakyUploadProvider::new("provider-file-retry-ok");

    // First call: upload fails → the service must return an error and MUST NOT
    // leave a Completed mapping behind.
    let first = get_or_upload_provider_file(
        &pool, &file_repo, &storage, file_id, user_id, &provider, &ai,
    )
    .await;
    assert!(
        first.is_err(),
        "first upload should surface the provider failure as an error"
    );

    let completed_after_fail = sqlx::query!(
        "SELECT upload_status FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2 AND upload_status = 'completed'",
        file_id,
        provider.id,
    )
    .fetch_optional(&pool)
    .await
    .expect("query mapping after failure");
    assert!(
        completed_after_fail.is_none(),
        "a failed upload must not persist a completed provider-file mapping"
    );

    // Second call: retry — upload succeeds, mapping is saved, id returned.
    let second = get_or_upload_provider_file(
        &pool, &file_repo, &storage, file_id, user_id, &provider, &ai,
    )
    .await
    .expect("retry upload should succeed");
    assert_eq!(second, "provider-file-retry-ok");

    // Cleanup the temp blob dir.
    let _ = std::fs::remove_dir_all(&tmp);
}

// audit id all-088637dee7e6 — the API-key-rotation cache-invalidation edge in
// get_or_upload_provider_file (service.rs:82-86): a cached Completed mapping
// whose stored api_key_fingerprint no longer matches the provider's current key
// belongs to a different account and MUST be discarded → re-upload, not a
// stale-id return. Drives the REAL service fn; only the provider boundary mocked.
struct SequentialUploadProvider {
    upload_calls: AtomicUsize,
}

#[ziee::async_trait]
impl AIProvider for SequentialUploadProvider {
    fn name(&self) -> &str {
        "seq-upload"
    }
    async fn stream_chat(
        &self,
        _a: &str,
        _b: &str,
        _r: ChatRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<StreamChatChunk, ProviderError>> + Send>>,
        ProviderError,
    > {
        Err(ProviderError::NotSupported("mock".into()))
    }
    async fn embeddings(
        &self,
        _a: &str,
        _b: &str,
        _r: EmbeddingsRequest,
    ) -> Result<EmbeddingsResponse, ProviderError> {
        Err(ProviderError::NotSupported("mock".into()))
    }
    fn supports_file_api(&self) -> bool {
        true
    }
    async fn upload_file(
        &self,
        _a: &str,
        _b: &str,
        _u: FileUpload,
    ) -> Result<Option<FileUploadResponse>, ProviderError> {
        let n = self.upload_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(FileUploadResponse {
            provider_file_id: format!("upload-{n}"),
            expires_at: None,
            metadata: None,
        }))
    }
}

#[tokio::test]
async fn test_key_rotation_discards_cached_mapping_and_reuploads() {
    use std::sync::Arc;
    use ziee::llm_provider_files_test_api::{
        get_or_upload_provider_file, FileRepository, FileStorage, FilesystemStorage, LlmProvider,
        ProxySettings,
    };

    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();
    let user =
        crate::common::test_helpers::create_user_with_permissions(&server, "rotate_user", &[]).await;
    let user_id = Uuid::parse_str(&user.user_id).unwrap();

    let file_id = create_test_file(&pool, user_id, "rotate.txt").await;
    let tmp = std::env::temp_dir().join(format!("ziee-rotate-{}", Uuid::new_v4()));
    let storage: Arc<dyn FileStorage> = Arc::new(FilesystemStorage::new(&tmp));
    storage
        .save_original(user_id, file_id, "txt", b"rotate me")
        .await
        .unwrap();
    let file_repo = FileRepository::new(pool.clone());

    let mut provider = LlmProvider {
        id: Uuid::new_v4(),
        name: "Anthropic Rotate".to_string(),
        provider_type: "anthropic".to_string(),
        enabled: true,
        api_key: Some("key-AAAAAAAA".to_string()),
        base_url: Some("https://api.test.example/v1".to_string()),
        built_in: false,
        proxy_settings: ProxySettings::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        default_runtime_version_id: None,
    };
    sqlx::query!(
        r#"INSERT INTO llm_providers (id, name, provider_type, enabled, api_key, base_url)
           VALUES ($1, $2, $3, true, $4, $5)"#,
        provider.id,
        provider.name,
        provider.provider_type,
        provider.api_key,
        provider.base_url,
    )
    .execute(&pool)
    .await
    .unwrap();

    let ai = SequentialUploadProvider { upload_calls: AtomicUsize::new(0) };

    // First call with key-A → uploads → "upload-0", stores fingerprint(A).
    let first =
        get_or_upload_provider_file(&pool, &file_repo, &storage, file_id, user_id, &provider, &ai)
            .await
            .unwrap();
    assert_eq!(first, "upload-0");
    assert_eq!(ai.upload_calls.load(Ordering::SeqCst), 1);

    // Same key-A again → cache hit, NO new upload.
    let cached =
        get_or_upload_provider_file(&pool, &file_repo, &storage, file_id, user_id, &provider, &ai)
            .await
            .unwrap();
    assert_eq!(cached, "upload-0", "same key must return the cached id");
    assert_eq!(ai.upload_calls.load(Ordering::SeqCst), 1, "no re-upload on a cache hit");

    // Rotate the provider's key → the stored fingerprint no longer matches, so
    // the cached mapping is discarded and a fresh upload runs (service.rs:82-86).
    provider.api_key = Some("key-BBBBBBBB".to_string());
    let after_rotation =
        get_or_upload_provider_file(&pool, &file_repo, &storage, file_id, user_id, &provider, &ai)
            .await
            .unwrap();
    assert_eq!(after_rotation, "upload-1", "rotated key must trigger a re-upload, not return the stale id");
    assert_eq!(ai.upload_calls.load(Ordering::SeqCst), 2, "key rotation must re-upload");

    let _ = std::fs::remove_dir_all(&tmp);
}

// audit id all-e69ba2fb610d — cross-tenant isolation of provider-file mappings.
// get_provider_file_mapping joins to files and filters f.user_id = $3
// (repository.rs:31-33), so user B must NOT resolve a mapping for user A's file
// even with the (globally-unique) file_id + provider_id. Drive the REAL repo fn.
#[tokio::test]
async fn test_provider_file_mapping_is_user_scoped() {
    use ziee::llm_provider_files_test_api::get_provider_file_mapping;

    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url).await.unwrap();

    let user_a =
        crate::common::test_helpers::create_user_with_permissions(&server, "pf_owner_a", &[]).await;
    let user_b =
        crate::common::test_helpers::create_user_with_permissions(&server, "pf_other_b", &[]).await;
    let a_id = Uuid::parse_str(&user_a.user_id).unwrap();
    let b_id = Uuid::parse_str(&user_b.user_id).unwrap();

    // A's file + a completed mapping.
    let file_id = create_test_file(&pool, a_id, "secret.pdf").await;
    let provider_id = create_test_provider(&pool, "Shared Provider", "anthropic").await;
    sqlx::query!(
        r#"INSERT INTO llm_provider_files
               (file_id, provider_id, provider_file_id, provider_metadata, upload_status)
           VALUES ($1, $2, $3, $4, 'completed')"#,
        file_id,
        provider_id,
        "file_owned_by_a",
        json!({}),
    )
    .execute(&pool)
    .await
    .expect("insert mapping");

    // Owner A resolves the mapping.
    let owner = get_provider_file_mapping(&pool, file_id, provider_id, a_id)
/// Cross-tenant security (repository.rs:31-33, service.rs:54-58 — the JOIN to
/// `files` on `f.user_id = $3`). A provider-file mapping created for user A's
/// file must NOT be retrievable by user B via get_provider_file_mapping, even
/// though file_id is globally unique. The owner still gets it. Drives the REAL
/// repository function (re-exported as ziee::llm_provider_file_mapping_for_user),
/// not a mirrored query.
#[tokio::test]
async fn test_get_provider_file_mapping_is_cross_tenant_scoped() {
    let server = crate::common::TestServer::start().await;
    let pool = sqlx::PgPool::connect(&server.database_url)
        .await
        .expect("connect test db");

    let user_a =
        crate::common::test_helpers::create_user_with_permissions(&server, "tenant_a", &[]).await;
    let user_b =
        crate::common::test_helpers::create_user_with_permissions(&server, "tenant_b", &[]).await;
    let a_id = Uuid::parse_str(&user_a.user_id).unwrap();
    let b_id = Uuid::parse_str(&user_b.user_id).unwrap();

    let file_id = create_test_file(&pool, a_id, "secret.pdf").await;
    let provider_id = create_test_provider(&pool, "Tenant Provider", "gemini").await;

    sqlx::query!(
        r#"INSERT INTO llm_provider_files
            (file_id, provider_id, provider_file_id, provider_metadata, upload_status)
            VALUES ($1, $2, $3, $4, 'completed')"#,
        file_id,
        provider_id,
        "file_secret",
        json!({})
    )
    .execute(&pool)
    .await
    .expect("create mapping");

    // The OWNER (user A) resolves the mapping.
    let owner = ziee::llm_provider_file_mapping_for_user(&pool, file_id, provider_id, a_id)
        .await
        .expect("query ok");
    assert!(owner.is_some(), "owner must resolve their own provider-file mapping");

    // User B (same file_id + provider_id) must get NOTHING — the f.user_id
    // filter makes another tenant's mapping structurally invisible.
    let cross = get_provider_file_mapping(&pool, file_id, provider_id, b_id)
        .await
        .expect("query ok");
    assert!(cross.is_none(), "a different user must NOT resolve user A's provider-file mapping");
    let user = crate::common::test_helpers::create_user_with_permissions(
        &server,
        "concurrent_upsert_user",
        &[],
    )
    .await;
    let user_id = Uuid::parse_str(&user.user_id).expect("Invalid user ID");

    let file_id = create_test_file(&pool, user_id, "race.pdf").await;
    let provider_id = create_test_provider(&pool, "Race Provider", "openai").await;

    // Fire N concurrent upserts on the SAME key, each proposing a different
    // provider_file_id. Each task takes its own connection from the pool.
    const N: usize = 8;
    let candidates: Vec<String> = (0..N).map(|i| format!("file_race_{i}")).collect();

    let mut handles = Vec::with_capacity(N);
    for pfid in candidates.iter().cloned() {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            sqlx::query!(
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
                RETURNING id, provider_file_id
                "#,
                file_id,
                provider_id,
                pfid,
                json!({ "filename": "race.pdf" })
            )
            .fetch_one(&pool)
            .await
        }));
    }

    // Every concurrent upsert must SUCCEED — the idempotency claim means none
    // returns a unique-violation; the ON CONFLICT swallows the race.
    let mut returned_ids = std::collections::HashSet::new();
    for h in handles {
        let row = h
            .await
            .expect("upsert task panicked")
            .expect("concurrent upsert returned a DB error (race not handled)");
        returned_ids.insert(row.id);
    }

    // Exactly ONE row must exist for the key — the constraint collapsed the burst.
    let rows = sqlx::query!(
        "SELECT id, provider_file_id, upload_status
         FROM llm_provider_files WHERE file_id = $1 AND provider_id = $2",
        file_id,
        provider_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to query mappings");

    assert_eq!(
        rows.len(),
        1,
        "concurrent upserts must collapse to a single row, found {}",
        rows.len()
    );

    // All upserts returned the SAME row id (the one canonical row), and its
    // terminal value is one of the racers' proposals (a consistent winner, not
    // a corrupted/partial write).
    assert_eq!(
        returned_ids.len(),
        1,
        "all concurrent upserts must resolve to one row id, saw {:?}",
        returned_ids
    );
    let row = &rows[0];
    assert_eq!(row.id, *returned_ids.iter().next().unwrap());
    assert_eq!(row.upload_status, "completed");
    assert!(
        candidates.contains(row.provider_file_id.as_ref().expect("provider_file_id set")),
        "surviving provider_file_id {:?} must be one of the concurrent proposals",
        row.provider_file_id
    );
    // A DIFFERENT tenant (user B) must NOT — the files JOIN filters by user_id.
    let intruder = ziee::llm_provider_file_mapping_for_user(&pool, file_id, provider_id, b_id)
        .await
        .expect("query ok");
    assert!(intruder.is_none(), "cross-tenant access to another user's provider-file mapping must be blocked");
}

// File repository

use crate::common::AppError;
use crate::modules::file::models::{File, FileCreateData};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FileRepository {
    pool: PgPool,
}

impl FileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create new file
    pub async fn create(&self, data: FileCreateData) -> Result<File, AppError> {
        let file = sqlx::query_as!(
            File,
            r#"
            INSERT INTO files (
                id, user_id, filename, file_size, mime_type, checksum,
                has_thumbnail, preview_page_count, text_page_count, processing_metadata,
                created_by
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, user_id, filename, file_size, mime_type, checksum,
                      has_thumbnail, preview_page_count, text_page_count,
                      processing_metadata as "processing_metadata!: _",
                      created_by,
                      created_at as "created_at: _",
                      updated_at as "updated_at: _"
            "#,
            data.id,
            data.user_id,
            data.filename,
            data.file_size,
            data.mime_type,
            data.checksum,
            data.has_thumbnail,
            data.preview_page_count,
            data.text_page_count,
            data.processing_metadata,
            data.created_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(file)
    }

    /// Get file by ID (internal use)
    pub async fn get_by_id(&self, file_id: Uuid) -> Result<Option<File>, AppError> {
        let file = sqlx::query_as!(
            File,
            r#"
            SELECT id, user_id, filename, file_size, mime_type, checksum,
                   has_thumbnail, preview_page_count, text_page_count,
                   processing_metadata as "processing_metadata!: _",
                   created_by,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM files
            WHERE id = $1
            "#,
            file_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(file)
    }

    /// Get file by ID and verify user ownership
    pub async fn get_by_id_and_user(
        &self,
        file_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<File>, AppError> {
        let file = sqlx::query_as!(
            File,
            r#"
            SELECT id, user_id, filename, file_size, mime_type, checksum,
                   has_thumbnail, preview_page_count, text_page_count,
                   processing_metadata as "processing_metadata!: _",
                   created_by,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM files
            WHERE id = $1 AND user_id = $2
            "#,
            file_id,
            user_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(file)
    }

    /// Batch-fetch files by id, scoped to the owning user. Foreign or
    /// deleted ids simply don't appear in the result (the caller maps
    /// by id and drops misses), so this preserves the same ownership +
    /// existence filtering as `get_by_id_and_user` in a single round-trip.
    /// Order is unspecified — callers that need ordering re-impose it from
    /// their own id list.
    pub async fn get_by_ids_and_user(
        &self,
        file_ids: &[Uuid],
        user_id: Uuid,
    ) -> Result<Vec<File>, AppError> {
        let files = sqlx::query_as!(
            File,
            r#"
            SELECT id, user_id, filename, file_size, mime_type, checksum,
                   has_thumbnail, preview_page_count, text_page_count,
                   processing_metadata as "processing_metadata!: _",
                   created_by,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM files
            WHERE id = ANY($1) AND user_id = $2
            "#,
            file_ids,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(files)
    }

    /// List files for user with pagination. Defense-in-depth on the
    /// i32 multiplication: cast both factors to i64 BEFORE multiplying
    /// so a (theoretical) bypass of the PaginationQuery deserialize
    /// clamp can't overflow into a tiny / negative offset. Closes
    /// 05-file F-14 (Medium).
    pub async fn list_by_user(
        &self,
        user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<File>, i64), AppError> {
        let page64 = (page as i64).max(1);
        let per_page64 = (per_page as i64).max(1);
        let offset = (page64 - 1).saturating_mul(per_page64);

        // Get total count
        let total: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM files WHERE user_id = $1"#,
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        // Get paginated results
        let files = sqlx::query_as!(
            File,
            r#"
            SELECT id, user_id, filename, file_size, mime_type, checksum,
                   has_thumbnail, preview_page_count, text_page_count,
                   processing_metadata as "processing_metadata!: _",
                   created_by,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM files
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            per_page64,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok((files, total))
    }

    /// Sum the user's current uploaded file bytes. Drives the per-user
    /// storage quota enforced at upload time. Closes 05-file F-16
    /// (Medium). Returns 0 when the user has no files.
    pub async fn count_user_bytes(&self, user_id: Uuid) -> Result<i64, AppError> {
        let total: Option<i64> = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(file_size), 0)::BIGINT AS "total"
               FROM files
               WHERE user_id = $1"#,
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(total.unwrap_or(0))
    }

    /// Delete file
    pub async fn delete(&self, file_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query!(
            "DELETE FROM files WHERE id = $1 AND user_id = $2",
            file_id,
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found("File"));
        }

        Ok(())
    }

}

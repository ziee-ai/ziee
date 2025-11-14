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
                user_id, filename, file_size, mime_type, checksum,
                thumbnail_count, page_count, processing_metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, user_id, filename, file_size, mime_type, checksum,
                      thumbnail_count, page_count,
                      processing_metadata as "processing_metadata!: _",
                      created_at as "created_at: _",
                      updated_at as "updated_at: _"
            "#,
            data.user_id,
            data.filename,
            data.file_size,
            data.mime_type,
            data.checksum,
            data.thumbnail_count,
            data.page_count,
            data.processing_metadata
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(file)
    }

    /// Get file by ID
    pub async fn get_by_id(&self, file_id: Uuid) -> Result<Option<File>, AppError> {
        let file = sqlx::query_as!(
            File,
            r#"
            SELECT id, user_id, filename, file_size, mime_type, checksum,
                   thumbnail_count, page_count,
                   processing_metadata as "processing_metadata!: _",
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
                   thumbnail_count, page_count,
                   processing_metadata as "processing_metadata!: _",
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

    /// List files for user with pagination
    pub async fn list_by_user(
        &self,
        user_id: Uuid,
        page: i32,
        per_page: i32,
    ) -> Result<(Vec<File>, i64), AppError> {
        let offset = ((page - 1) * per_page) as i64;

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
                   thumbnail_count, page_count,
                   processing_metadata as "processing_metadata!: _",
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
            FROM files
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            per_page as i64,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok((files, total))
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

    /// Update processing metadata
    pub async fn update_processing_metadata(
        &self,
        file_id: Uuid,
        metadata: serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE files SET processing_metadata = $1, updated_at = NOW() WHERE id = $2",
            metadata,
            file_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(())
    }
}

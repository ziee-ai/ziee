// Project↔file relationship repository.
//
// Owns reads/writes against the `project_files` join table. Relocated
// from `modules/project/repository.rs` as part of the project↔file
// inversion. The project module no longer references this table.
//
// Exposed via the global `Repos.project_files` namespace (declared in
// `core/repository.rs`).

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::file::models::File as FileEntity;
use crate::modules::file::project_extension::models::ProjectFileListResponse;

/// Hard cap on project files (Tier-1 validator gate). Matches the v1
/// design in Plan 5 §8 ("File count cap — 100 files per project").
pub const PROJECT_MAX_FILES: i64 = 100;

pub struct ProjectFilesRepository {
    pool: PgPool,
}

impl ProjectFilesRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Race-free attach that enforces the file count cap atomically.
    /// Closes audit B1: two concurrent attaches at count=99 used to
    /// both pass a pre-check and result in count=101. Now we take a
    /// `FOR UPDATE` row lock on the project, count under the lock,
    /// reject if at cap, and insert in the same transaction. Returns
    /// `Ok(true)` if a new row was inserted, `Ok(false)` if the file
    /// was already attached (idempotent path — cap not consulted).
    pub async fn attach_file_capped(
        &self,
        project_id: Uuid,
        file_id: Uuid,
        cap: i64,
    ) -> Result<bool, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;

        // Lock the project row so no concurrent attach can race past
        // the count check. Cheap: one row per project.
        let project_locked = sqlx::query_scalar!(
            "SELECT 1 FROM projects WHERE id = $1 FOR UPDATE",
            project_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        if project_locked.is_none() {
            return Err(AppError::not_found("Project"));
        }

        // Already attached? Idempotent — don't count toward cap.
        let already: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1 AND file_id = $2",
            project_id,
            file_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if already > 0 {
            tx.commit().await.map_err(AppError::database_error)?;
            return Ok(false);
        }

        // Recount under the lock — this is the load-bearing step.
        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1",
            project_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if count >= cap {
            return Err(AppError::unprocessable_entity(
                "PROJECT_FILE_COUNT_CAP",
                format!("Project file count cap ({cap}) reached"),
            ));
        }

        sqlx::query!(
            r#"
            INSERT INTO project_files (project_id, file_id)
            VALUES ($1, $2)
            ON CONFLICT (project_id, file_id) DO NOTHING
            "#,
            project_id,
            file_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;

        tx.commit().await.map_err(AppError::database_error)?;
        Ok(true)
    }

    pub async fn detach_file(&self, project_id: Uuid, file_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM project_files WHERE project_id = $1 AND file_id = $2",
            project_id,
            file_id
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_files(&self, project_id: Uuid) -> Result<i64, AppError> {
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_files WHERE project_id = $1",
            project_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        Ok(count)
    }

    /// List file IDs only — fast path for chat-time knowledge injection.
    pub async fn list_file_ids(&self, project_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query!(
            "SELECT file_id FROM project_files WHERE project_id = $1 ORDER BY added_at ASC",
            project_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| r.file_id).collect())
    }

    /// List files with metadata (JOIN on files). Returns the same File
    /// entity the file module returns, for client convenience. Sorted
    /// newest-first (recent uploads at the top) — matches how the chat
    /// conversation list and other recency-driven UI surfaces order
    /// their rows.
    pub async fn list_files(
        &self,
        project_id: Uuid,
    ) -> Result<ProjectFileListResponse, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                f.id, f.user_id, f.filename, f.file_size,
                f.mime_type, f.checksum, f.has_thumbnail,
                f.preview_page_count, f.text_page_count,
                f.processing_metadata, f.created_by,
                f.created_at, f.updated_at,
                fv.version, f.current_version_id, fv.blob_version_id
            FROM project_files pf
            JOIN files f ON f.id = pf.file_id
            JOIN file_versions fv ON fv.id = f.current_version_id
            WHERE pf.project_id = $1
            ORDER BY pf.added_at DESC
            "#,
            project_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;

        let total = rows.len() as i64;
        let files: Vec<FileEntity> = rows
            .into_iter()
            .map(|r| FileEntity {
                id: r.id,
                user_id: r.user_id,
                filename: r.filename,
                file_size: r.file_size,
                mime_type: r.mime_type,
                checksum: r.checksum,
                has_thumbnail: r.has_thumbnail,
                preview_page_count: r.preview_page_count,
                text_page_count: r.text_page_count,
                processing_metadata: r.processing_metadata.unwrap_or_else(|| serde_json::json!({})),
                created_by: r.created_by,
                created_at: chrono::DateTime::from_timestamp(r.created_at.unix_timestamp(), 0)
                    .unwrap(),
                updated_at: chrono::DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0)
                    .unwrap(),
                version: r.version,
                current_version_id: r.current_version_id,
                blob_version_id: r.blob_version_id,
            })
            .collect();

        Ok(ProjectFileListResponse { files, total })
    }

    /// Clone all `project_files` rows from `src_project_id` to
    /// `dst_project_id` in the given transaction. Invoked by the file
    /// module's `ProjectExtension::on_project_duplicated` hook so
    /// duplicate-project carries forward the file attachments without
    /// project module needing file knowledge.
    ///
    /// Idempotent at the join level (`ON CONFLICT DO NOTHING`) — re-runs
    /// on a partially-populated destination are safe.
    pub async fn clone_for_project<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        src_project_id: Uuid,
        dst_project_id: Uuid,
    ) -> Result<u64, AppError> {
        let res = sqlx::query!(
            r#"
            INSERT INTO project_files (project_id, file_id)
            SELECT $2, file_id FROM project_files WHERE project_id = $1
            ON CONFLICT (project_id, file_id) DO NOTHING
            "#,
            src_project_id,
            dst_project_id
        )
        .execute(&mut **tx)
        .await
        .map_err(AppError::database_error)?;
        Ok(res.rows_affected())
    }
}

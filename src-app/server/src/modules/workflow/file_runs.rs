//! `file_workflow_runs` join-table repository (chunk `ziee-file`).
//!
//! The generic `ziee-file` store is domain-agnostic and carries no run column.
//! The file↔run association the workflow module needs — A3 run-artifact linking,
//! A5 run-delete cascade, run-history surfacing — lives here as an explicit join
//! (replacing the former `files.workflow_run_id` column + the two
//! `FileRepository` helpers). Registered as `Repos.file_workflow_runs`.

use crate::common::AppError;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct FileWorkflowRunsRepository {
    pool: PgPool,
}

impl FileWorkflowRunsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Link a file to the workflow run that produced it (A3). Idempotent — a
    /// re-link is a no-op. Lets the run-delete cascade (A5) find a run's files
    /// and the run history surface them.
    pub async fn link(&self, file_id: Uuid, run_id: Uuid) -> Result<(), AppError> {
        sqlx::query!(
            "INSERT INTO file_workflow_runs (file_id, workflow_run_id) VALUES ($1, $2) \
             ON CONFLICT (file_id, workflow_run_id) DO NOTHING",
            file_id,
            run_id,
        )
        .execute(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(())
    }

    /// A5: file ids produced by a workflow run (for the delete cascade + run
    /// history). Replaces `FileRepository::list_ids_by_workflow_run`.
    pub async fn list_file_ids(&self, run_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        let rows = sqlx::query!(
            "SELECT file_id FROM file_workflow_runs WHERE workflow_run_id = $1",
            run_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(|r| r.file_id).collect())
    }
}

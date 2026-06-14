//! Minimum repository surface for B2: insert/find/delete workflows +
//! create/mark/find workflow runs. B4 fleshes out the runner-side
//! query set (list_in_flight for startup sweep, mark_running, persist
//! step metadata, etc.).

#![allow(dead_code)]

use sqlx::PgPool;
use uuid::Uuid;

use super::models::{CreateWorkflow, CreateWorkflowRun, Workflow, WorkflowRun, WorkflowRunStatus};
use crate::common::AppError;

pub struct WorkflowRepository {
    pool: PgPool,
}

impl WorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, request: CreateWorkflow) -> Result<Workflow, AppError> {
        insert(&self.pool, request).await
    }

    pub async fn find_by_name_version(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<Workflow>, AppError> {
        find_by_name_version(&self.pool, name, version).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        delete(&self.pool, id).await
    }

    pub async fn insert_run(&self, request: CreateWorkflowRun) -> Result<WorkflowRun, AppError> {
        insert_run(&self.pool, request).await
    }

    pub async fn mark_status(
        &self,
        run_id: Uuid,
        status: WorkflowRunStatus,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        mark_status(&self.pool, run_id, status, error_message).await
    }

    pub async fn find_run(&self, run_id: Uuid) -> Result<Option<WorkflowRun>, AppError> {
        find_run(&self.pool, run_id).await
    }
}

pub async fn insert(pool: &PgPool, request: CreateWorkflow) -> Result<Workflow, AppError> {
    let row = sqlx::query_as!(
        Workflow,
        r#"
        INSERT INTO workflows (
            name, version, display_name, description,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point, tags,
            scope, owner_user_id, created_by, enabled, is_dev,
            compiled_ir_json
        )
        VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8,
            $9, $10,
            $11, $12, $13, $14, $15,
            $16
        )
        RETURNING
            id,
            name,
            version,
            display_name,
            description,
            extracted_path,
            bundle_sha256,
            bundle_size_bytes,
            file_count,
            entry_point,
            tags as "tags: _",
            scope,
            owner_user_id,
            created_by,
            enabled,
            is_dev,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        request.name,
        request.version,
        request.display_name,
        request.description,
        request.extracted_path,
        request.bundle_sha256,
        request.bundle_size_bytes,
        request.file_count,
        request.entry_point,
        request.tags,
        request.scope,
        request.owner_user_id,
        request.created_by,
        request.enabled,
        request.is_dev,
        request.compiled_ir_json,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn find_by_name_version(
    pool: &PgPool,
    name: &str,
    version: Option<&str>,
) -> Result<Option<Workflow>, AppError> {
    let row = sqlx::query_as!(
        Workflow,
        r#"
        SELECT
            id,
            name,
            version,
            display_name,
            description,
            extracted_path,
            bundle_sha256,
            bundle_size_bytes,
            file_count,
            entry_point,
            tags as "tags: _",
            scope,
            owner_user_id,
            created_by,
            enabled,
            is_dev,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflows
        WHERE name = $1
          AND ($2::text IS NULL AND version IS NULL OR version = $2)
        LIMIT 1
        "#,
        name,
        version,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let n = sqlx::query!("DELETE FROM workflows WHERE id = $1", id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?
        .rows_affected();
    if n == 0 {
        return Err(AppError::not_found("Workflow"));
    }
    Ok(())
}

// ============================================================
// workflow_runs (B4 expands this)
// ============================================================

pub async fn insert_run(
    pool: &PgPool,
    request: CreateWorkflowRun,
) -> Result<WorkflowRun, AppError> {
    let row = sqlx::query_as!(
        WorkflowRun,
        r#"
        INSERT INTO workflow_runs (
            workflow_id, conversation_id, user_id, model_id, sandbox_flavor,
            run_kind, inputs_json
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING
            id,
            workflow_id,
            conversation_id,
            user_id,
            model_id,
            sandbox_flavor,
            run_kind,
            inputs_json as "inputs_json: _",
            step_outputs_json as "step_outputs_json: _",
            step_item_progress_json as "step_item_progress_json: _",
            step_logs_json as "step_logs_json: _",
            step_artifacts_json as "step_artifacts_json: _",
            pending_elicitation_json as "pending_elicitation_json: _",
            final_output_json as "final_output_json: _",
            status,
            current_step,
            error_message,
            total_tokens,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        request.workflow_id,
        request.conversation_id,
        request.user_id,
        request.model_id,
        request.sandbox_flavor,
        request.run_kind,
        request.inputs_json,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn mark_status(
    pool: &PgPool,
    run_id: Uuid,
    status: WorkflowRunStatus,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET status = $2,
            error_message = $3,
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        status.as_str(),
        error_message,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn find_run(pool: &PgPool, run_id: Uuid) -> Result<Option<WorkflowRun>, AppError> {
    let row = sqlx::query_as!(
        WorkflowRun,
        r#"
        SELECT
            id,
            workflow_id,
            conversation_id,
            user_id,
            model_id,
            sandbox_flavor,
            run_kind,
            inputs_json as "inputs_json: _",
            step_outputs_json as "step_outputs_json: _",
            step_item_progress_json as "step_item_progress_json: _",
            step_logs_json as "step_logs_json: _",
            step_artifacts_json as "step_artifacts_json: _",
            pending_elicitation_json as "pending_elicitation_json: _",
            final_output_json as "final_output_json: _",
            status,
            current_step,
            error_message,
            total_tokens,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflow_runs
        WHERE id = $1
        "#,
        run_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

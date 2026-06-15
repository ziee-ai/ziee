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

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Workflow>, AppError> {
        find_by_id(&self.pool, id).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        request: super::models::UpdateWorkflow,
    ) -> Result<Workflow, AppError> {
        update(&self.pool, id, request).await
    }

    /// Group assignment management for system-scope workflows. Mirrors
    /// `SkillRepository`'s `get/assign/remove` group fns.
    pub async fn get_workflow_groups(&self, workflow_id: Uuid) -> Result<Vec<Uuid>, AppError> {
        get_workflow_groups(&self.pool, workflow_id).await
    }

    pub async fn set_workflow_groups(
        &self,
        workflow_id: Uuid,
        group_ids: &[Uuid],
    ) -> Result<(), AppError> {
        set_workflow_groups(&self.pool, workflow_id, group_ids).await
    }

    pub async fn remove_workflow_group(
        &self,
        workflow_id: Uuid,
        group_id: Uuid,
    ) -> Result<(), AppError> {
        remove_workflow_group(&self.pool, workflow_id, group_id).await
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

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Workflow>, AppError> {
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
        WHERE id = $1
        "#,
        id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Workflow>, AppError> {
    let rows = sqlx::query_as!(
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
        WHERE (scope = 'user' AND owner_user_id = $1)
           OR scope = 'system'
        ORDER BY name ASC
        "#,
        user_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
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

/// Persist the step's per-step output metadata into
/// `step_outputs_json[step_id]`. Idempotent — overwrites any prior
/// entry for the same step_id (re-run handling lives in the runner).
pub async fn persist_step_meta(
    pool: &PgPool,
    run_id: Uuid,
    step_id: &str,
    meta: &serde_json::Value,
    total_tokens_delta: u64,
    current_step: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET step_outputs_json = jsonb_set(
                coalesce(step_outputs_json, '{}'::jsonb),
                ARRAY[$2::text],
                $3,
                true
            ),
            total_tokens = total_tokens + $4,
            current_step = COALESCE($5, current_step),
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        step_id,
        meta,
        total_tokens_delta as i32,
        current_step,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn persist_step_item_progress(
    pool: &PgPool,
    run_id: Uuid,
    step_id: &str,
    progress: &serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET step_item_progress_json = jsonb_set(
                coalesce(step_item_progress_json, '{}'::jsonb),
                ARRAY[$2::text],
                $3,
                true
            ),
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        step_id,
        progress,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn persist_step_artifacts(
    pool: &PgPool,
    run_id: Uuid,
    step_id: &str,
    artifacts: &serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET step_artifacts_json = jsonb_set(
                coalesce(step_artifacts_json, '{}'::jsonb),
                ARRAY[$2::text],
                $3,
                true
            ),
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        step_id,
        artifacts,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn persist_step_logs(
    pool: &PgPool,
    run_id: Uuid,
    step_id: &str,
    logs: &serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET step_logs_json = jsonb_set(
                coalesce(step_logs_json, '{}'::jsonb),
                ARRAY[$2::text],
                $3,
                true
            ),
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        step_id,
        logs,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Set or clear the pending elicitation slot.
pub async fn set_pending_elicitation(
    pool: &PgPool,
    run_id: Uuid,
    value: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET pending_elicitation_json = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        value,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn set_final_output(
    pool: &PgPool,
    run_id: Uuid,
    value: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET final_output_json = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        value,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

pub async fn mark_running(pool: &PgPool, run_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE workflow_runs SET status = 'running', updated_at = NOW() WHERE id = $1"#,
        run_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Status-guarded cancel CAS (plan §4.3).
pub async fn cancel_cas(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<String>, AppError> {
    let row = sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET status = 'cancelled',
            error_message = 'cancelled by user',
            updated_at = NOW()
        WHERE id = $1 AND status IN ('pending', 'running')
        RETURNING status
        "#,
        run_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.map(|r| r.status))
}

/// Startup sweep: flip every still-pending/running row to `failed`.
pub async fn fail_orphaned_runs(pool: &PgPool) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET status = 'failed',
            error_message = 'server restart during execution',
            updated_at = NOW()
        WHERE status IN ('pending', 'running')
        "#,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
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

/// Look up an installed workflow by its reverse-DNS name (latest by
/// `updated_at`). Used by `workflow_mcp::tools::call_tool` to reverse
/// the `wf_<slug>` → workflow mapping. B5.
pub async fn find_by_name(pool: &PgPool, name: &str) -> Result<Option<Workflow>, AppError> {
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
        ORDER BY updated_at DESC
        LIMIT 1
        "#,
        name,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Edit the limited mutable metadata of a workflow (display_name /
/// description / enabled / tags). Mirrors `skill::repository::update`.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    request: super::models::UpdateWorkflow,
) -> Result<Workflow, AppError> {
    let row = sqlx::query_as!(
        Workflow,
        r#"
        UPDATE workflows SET
            display_name = COALESCE($2, display_name),
            description = COALESCE($3, description),
            enabled = COALESCE($4, enabled),
            tags = COALESCE($5, tags),
            updated_at = NOW()
        WHERE id = $1
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
        id,
        request.display_name,
        request.description,
        request.enabled,
        request.tags,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Workflow"))?;
    Ok(row)
}

// ============================================================
// group_workflows — system-scope group assignment (mirrors group_skills)
// ============================================================

pub async fn get_workflow_groups(
    pool: &PgPool,
    workflow_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let rows = sqlx::query_scalar!(
        r#"SELECT group_id FROM group_workflows WHERE workflow_id = $1"#,
        workflow_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

/// Replace the entire set of groups assigned to a workflow (diff-apply):
/// remove the groups no longer desired, insert the new ones. The
/// `group_workflows` table has a trigger rejecting non-system workflows,
/// so the caller MUST have already verified `scope == 'system'`.
pub async fn set_workflow_groups(
    pool: &PgPool,
    workflow_id: Uuid,
    group_ids: &[Uuid],
) -> Result<(), AppError> {
    use std::collections::HashSet;
    let current: HashSet<Uuid> = get_workflow_groups(pool, workflow_id)
        .await?
        .into_iter()
        .collect();
    let desired: HashSet<Uuid> = group_ids.iter().copied().collect();
    for gid in current.difference(&desired) {
        remove_workflow_group(pool, workflow_id, *gid).await?;
    }
    for gid in desired.difference(&current) {
        sqlx::query!(
            r#"
            INSERT INTO group_workflows (group_id, workflow_id)
            VALUES ($1, $2)
            ON CONFLICT DO NOTHING
            "#,
            gid,
            workflow_id,
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }
    Ok(())
}

pub async fn remove_workflow_group(
    pool: &PgPool,
    workflow_id: Uuid,
    group_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM group_workflows WHERE workflow_id = $1 AND group_id = $2"#,
        workflow_id,
        group_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Recent runs owned by `user_id`, newest first, capped at `limit`.
/// Backs `workflow_mcp::resources::list` (recency-bounded resource
/// listing). B5.
pub async fn list_runs_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<WorkflowRun>, AppError> {
    let rows = sqlx::query_as!(
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
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT $2
        "#,
        user_id,
        limit,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

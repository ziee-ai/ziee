//! Minimum repository surface for B2: insert/find/delete workflows +
//! create/mark/find workflow runs. B4 fleshes out the runner-side
//! query set (list_in_flight for startup sweep, mark_running, persist
//! step metadata, etc.).


use sqlx::PgPool;
use uuid::Uuid;

use super::models::{
    CreateBackgroundRun, CreateWorkflow, CreateWorkflowRun, JobKind, RunNote, Workflow,
    WorkflowRun, WorkflowRunStatus,
};
use super::types::{BackgroundRunSummary, WorkflowRunSummary};
use crate::common::AppError;

pub struct WorkflowRepository {
    pool: PgPool,
}

// Repository facade: several wrapper methods aren't called yet (callers use the
// free `repository::*` fns / owner-scoped variants directly). Kept as the B4
// query surface per the module doc above.
#[allow(dead_code)]
impl WorkflowRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, request: CreateWorkflow) -> Result<Workflow, AppError> {
        insert(&self.pool, request).await
    }

    /// H1: owner-scoped (name, version) lookup (see skill repo twin).
    pub async fn find_by_name_version_owner(
        &self,
        name: &str,
        version: Option<&str>,
        owner_user_id: Option<Uuid>,
    ) -> Result<Option<Workflow>, AppError> {
        find_by_name_version_owner(&self.pool, name, version, owner_user_id).await
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

    /// All system workflows assigned to a group (group → workflows direction,
    /// for the User Groups page assignment widget). Mirrors the MCP
    /// `get_system_servers_for_group`.
    pub async fn get_system_workflows_for_group(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<Workflow>, AppError> {
        get_system_workflows_for_group(&self.pool, group_id).await
    }

    /// How many of the given ids are existing `scope = 'system'` workflows.
    /// The group-assignment update handler compares this to the requested
    /// count to reject non-system / unknown ids with a 400 before writing.
    pub async fn count_system_workflows_in(&self, ids: &[Uuid]) -> Result<i64, AppError> {
        count_system_workflows_in(&self.pool, ids).await
    }

    /// Replace the full set of system workflows assigned to a group in ONE
    /// transaction (group → workflows direction). Removing-then-adding as N
    /// separate pool calls left a partial-write window that could strip a
    /// group's access on a mid-loop failure; the tx removes that window.
    /// Callers MUST have validated `desired` are all system-scope.
    pub async fn set_group_system_workflows(
        &self,
        group_id: Uuid,
        desired: &[Uuid],
    ) -> Result<(), AppError> {
        set_group_system_workflows(&self.pool, group_id, desired).await
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
            ephemeral, conversation_id,
            compiled_ir_json
        )
        VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8,
            $9, $10,
            $11, $12, $13, $14, $15,
            $16, $17,
            $18
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
            ephemeral,
            conversation_id,
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
        request.ephemeral,
        request.conversation_id,
        request.compiled_ir_json,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// H1: owner-scoped (name, version) lookup. NULL `owner_user_id` matches
/// the system row; a non-NULL value matches that user's row only.
pub async fn find_by_name_version_owner(
    pool: &PgPool,
    name: &str,
    version: Option<&str>,
    owner_user_id: Option<Uuid>,
) -> Result<Option<Workflow>, AppError> {
    let row = sqlx::query_as!(
        Workflow,
        r#"
        SELECT
            id, name, version, display_name, description,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point,
            tags as "tags: _",
            scope, owner_user_id, created_by, enabled, is_dev,
            ephemeral, conversation_id,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflows
        WHERE name = $1
          AND (($2::text IS NULL AND version IS NULL) OR version = $2)
          AND owner_user_id IS NOT DISTINCT FROM $3
        LIMIT 1
        "#,
        name,
        version,
        owner_user_id,
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
            ephemeral,
            conversation_id,
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

/// List workflows visible to `user_id`, bounded to `limit` rows starting at
/// `offset` so the listing can never return an unbounded set. Callers that
/// don't paginate pass `DEFAULT_PAGE_SIZE` / `0`.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Workflow>, AppError> {
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
            ephemeral,
            conversation_id,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflows w
        WHERE w.ephemeral = FALSE
          AND ((w.scope = 'user' AND w.owner_user_id = $1)
           OR (w.scope = 'system' AND (
                NOT EXISTS (SELECT 1 FROM group_workflows WHERE workflow_id = w.id)
                OR EXISTS (
                  SELECT 1 FROM group_workflows gw
                  JOIN user_groups ug ON gw.group_id = ug.group_id
                  WHERE gw.workflow_id = w.id AND ug.user_id = $1
                )
           )))
        ORDER BY name ASC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

/// Group-restriction access check for a single workflow (H2). Mirrors
/// `skill::repository::user_can_read`. A user can access a workflow iff
/// they own it (user-scope) OR it's a system workflow with NO group
/// restriction OR it's a system workflow assigned to one of their groups.
/// Used by the GET / RUN / cancel handlers so a group-restricted system
/// workflow is invisible + unrunnable to a non-member.
pub async fn user_can_access(
    pool: &PgPool,
    user_id: Uuid,
    workflow_id: Uuid,
) -> Result<bool, AppError> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM workflows w
        WHERE w.id = $1
          AND (
            (w.scope = 'user' AND w.owner_user_id = $2)
            OR (w.scope = 'system' AND (
              NOT EXISTS (SELECT 1 FROM group_workflows WHERE workflow_id = w.id)
              OR EXISTS (
                SELECT 1 FROM group_workflows gw
                JOIN user_groups ug ON gw.group_id = ug.group_id
                WHERE gw.workflow_id = w.id AND ug.user_id = $2
              )
            ))
          )
        "#,
        workflow_id,
        user_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(count > 0)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;
    // Drop the hub-install tracking row in the same transaction. There is no FK
    // cascade (`hub_entities.entity_id` is a plain UUID), so deleting a
    // hub-installed workflow through any path would otherwise orphan its
    // hub_entities row. Idempotent: a no-op for non-hub workflows.
    sqlx::query!(
        "DELETE FROM hub_entities WHERE entity_type = 'workflow' AND entity_id = $1",
        id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;
    let n = sqlx::query!("DELETE FROM workflows WHERE id = $1", id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?
        .rows_affected();
    if n == 0 {
        // Roll back the hub_entities delete — the workflow did not exist.
        return Err(AppError::not_found("Workflow"));
    }
    tx.commit().await.map_err(AppError::database_error)?;
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
            run_kind, invocation_source, inputs_json
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            id,
            workflow_id,
            job_kind,
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
            step_progress_json as "step_progress_json: _",
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
        request.invocation_source,
        request.inputs_json,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Insert a generalized BACKGROUND run (ITEM-14 / ITEM-17): a non-`workflow`
/// `JobKind` row with `workflow_id = NULL` — a detached sub-agent turn or a
/// fire-and-forget sandbox exec that reuses the runner's heartbeat +
/// `mark_running`/`mark_status` guards + `RunHandle` registry but has NO backing
/// `workflows` bundle. The row starts `pending`; the caller (typically
/// [`super::runner::spawn_background_run`]) registers a handle + drives it. We do
/// NOT synthesize a fake ephemeral workflow row (DEC-22).
///
/// `run_kind` is fixed to `'normal'` (a background run is never a workflow
/// test/dry-run). Rejects a `Workflow` kind: that path has a bundle and MUST go
/// through [`insert_run`] with a real `workflow_id` (the DB coherence CHECK
/// `workflow_runs_job_kind_workflow_id_check` is the backstop).
// Seam (ITEM-17): called by `spawn_background_run` / the background drivers in a
// later tranche; the backbone ships + tests the create path now.
#[allow(dead_code)]
pub async fn insert_background_run(
    pool: &PgPool,
    request: CreateBackgroundRun,
) -> Result<WorkflowRun, AppError> {
    if matches!(request.job_kind, JobKind::Workflow) {
        return Err(AppError::internal_error(
            "insert_background_run: 'workflow' kind requires a bundle — use insert_run",
        ));
    }
    let row = sqlx::query_as!(
        WorkflowRun,
        r#"
        INSERT INTO workflow_runs (
            workflow_id, job_kind, conversation_id, user_id, model_id,
            sandbox_flavor, run_kind, invocation_source, inputs_json
        )
        VALUES (NULL, $1, $2, $3, $4, $5, 'normal', $6, $7)
        RETURNING
            id,
            workflow_id,
            job_kind,
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
            step_progress_json as "step_progress_json: _",
            status,
            current_step,
            error_message,
            total_tokens,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        request.job_kind.as_str(),
        request.conversation_id,
        request.user_id,
        request.model_id,
        request.sandbox_flavor,
        request.invocation_source,
        request.inputs_json,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// Owner-scoped run fetch for the background read seam (`check_status` /
/// `collect_result` — the model-facing MCP trio is a later tranche). Returns the
/// row (status + `job_kind` + `final_output_json`) ONLY when it belongs to
/// `user_id`; a foreign or missing id yields `None` so the caller returns 404 —
/// never leaking another user's run (DEC-36 / CODING_GUIDELINES §1). Works for
/// every `job_kind`, including background runs with a NULL `workflow_id`.
// Seam (ITEM-17): the read side of the model-facing check_status/collect_result
// MCP trio (later tranche); the backbone ships + tests the owner-scoped read.
#[allow(dead_code)]
pub async fn find_run_for_owner(
    pool: &PgPool,
    run_id: Uuid,
    user_id: Uuid,
) -> Result<Option<WorkflowRun>, AppError> {
    Ok(find_run(pool, run_id).await?.filter(|r| r.user_id == user_id))
}

/// Terminal-status write (H3). Guards against clobbering an already
/// terminal row: an in-flight step that completes AFTER a cancel must NOT
/// overwrite `cancelled` back to `completed`/`failed`. The CAS predicate
/// `status NOT IN ('cancelled','completed','failed')` makes the first
/// terminal writer win; later writers are no-ops.
///
/// Writing `cancelled` is a special case — the runner's
/// `RunInnerOutcome::Cancelled` arm re-asserts cancellation after the
/// `cancel_cas` handler already flipped the row, and that must stay
/// idempotent (the row is already `cancelled`, so the guard would block
/// it). We therefore allow a `cancelled` write to also match an already
/// `cancelled` row — it's a no-op either way and never resurrects a
/// completed/failed run to cancelled.
pub async fn mark_status(
    pool: &PgPool,
    run_id: Uuid,
    status: WorkflowRunStatus,
    error_message: Option<&str>,
) -> Result<(), AppError> {
    let allow_cancelled_self = matches!(status, WorkflowRunStatus::Cancelled);
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET status = $2,
            error_message = $3,
            updated_at = NOW()
        WHERE id = $1
          AND (
            status NOT IN ('cancelled', 'completed', 'failed')
            OR ($4 AND status = 'cancelled')
          )
        "#,
        run_id,
        status.as_str(),
        error_message,
        allow_cancelled_self,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Record the step currently in progress so a FAILURE (incl. a first-step
/// failure) can name it. `build_error_result` reads `current_step`; without
/// this, `current_step` was only set on step COMPLETION, so a run that failed
/// on its first step reported a null `failed_step`.
pub async fn set_current_step(
    pool: &PgPool,
    run_id: Uuid,
    step_id: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE workflow_runs SET current_step = $2, updated_at = NOW() WHERE id = $1",
        run_id,
        step_id,
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
        // M4: BIGINT column — saturating cast so an absurd delta clamps
        // to i64::MAX rather than wrapping negative.
        i64::try_from(total_tokens_delta).unwrap_or(i64::MAX),
        current_step,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// P2.6: replace the running sandbox step's live progress track map on the run
/// row (the whole coalesced `{id->ProgressTrack}` object, written on the
/// dispatcher's throttle flush). The Snapshot reads it so a refresh rehydrates
/// in-flight bars. Cleared by [`clear_step_progress`] when the step ends.
pub async fn set_step_progress(
    pool: &PgPool,
    run_id: Uuid,
    tracks_json: &serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE workflow_runs SET step_progress_json = $2, updated_at = NOW() WHERE id = $1"#,
        run_id,
        tracks_json,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// P2.6: clear live progress when the running step terminates
/// (completed / failed / cancelled) — only the current step's tracks are ever
/// stored, so this resets the slot for the next step.
pub async fn clear_step_progress(pool: &PgPool, run_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE workflow_runs SET step_progress_json = NULL WHERE id = $1"#,
        run_id,
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

/// Durable resume: persist a user-submitted elicit response on the run row so
/// a freshly-spawned `resume_run` consumes it at the elicit step instead of
/// re-parking. Stored as `{ step_id, elicitation_id, response }`; cleared
/// (`None`) once consumed. Only ever set transiently on a cold `waiting` run.
pub async fn set_elicit_response(
    pool: &PgPool,
    run_id: Uuid,
    value: Option<serde_json::Value>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET elicit_response_json = $2,
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

/// Read the durable elicit response (if any) for a run.
pub async fn get_elicit_response(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<serde_json::Value>, AppError> {
    let row = sqlx::query!(
        r#"SELECT elicit_response_json as "elicit_response_json: serde_json::Value" FROM workflow_runs WHERE id = $1"#,
        run_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.and_then(|r| r.elicit_response_json))
}

/// Read the durable agent transcript (a JSON array of `ai_providers::ChatMessage`)
/// for a run — the resume source for the `kind: agent` step (DEC-8). NULL ⇒ the
/// agent has not started (an empty transcript).
pub async fn get_agent_transcript(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Option<serde_json::Value>, AppError> {
    let row = sqlx::query!(
        r#"SELECT agent_transcript_json as "agent_transcript_json: serde_json::Value" FROM workflow_runs WHERE id = $1"#,
        run_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.and_then(|r| r.agent_transcript_json))
}

/// Replace the durable agent transcript for a run (whole-array write — the agent
/// host's `TranscriptStore` load/append/replace_head all round-trip through this).
pub async fn set_agent_transcript(
    pool: &PgPool,
    run_id: Uuid,
    value: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET agent_transcript_json = $2,
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

/// Flag whether the runner is currently inside an agent step (ITEM-17). Set TRUE
/// on agent-step entry so the boot sweep SPARES a crashed `running` agent run
/// (marks it `resumable`) instead of failing it; cleared on step exit.
pub async fn set_resumable_agent(
    pool: &PgPool,
    run_id: Uuid,
    resumable: bool,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET resumable_agent = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
        run_id,
        resumable,
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

/// Flip `pending` → `running` (H3). Guarded with `WHERE status =
/// 'pending'` so a fast cancel that landed BEFORE the runner task got to
/// `mark_running` is not resurrected to `running`: if the row was already
/// flipped to `cancelled` (or any non-pending state) the UPDATE matches
/// zero rows and the runner observes the cancel on its next
/// `handle.is_cancelled()` / DB re-check.
pub async fn mark_running(pool: &PgPool, run_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE workflow_runs SET status = 'running', updated_at = NOW() WHERE id = $1 AND status = 'pending'"#,
        run_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Liveness heartbeat: bump `updated_at` on a still-running run without
/// changing any other state. The workflow_mcp tool path treats a stalled
/// `updated_at` as a crashed runner (M5 no-progress guard); a long-but-live
/// step (e.g. a 30-min elicit wait) produces no step transitions, so the
/// runner ticks this heartbeat to prove it's alive. The status guard means a
/// terminal run is never touched (can't resurrect a completed/cancelled run).
pub async fn heartbeat(pool: &PgPool, run_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE workflow_runs SET updated_at = NOW() WHERE id = $1 AND status IN ('pending', 'running')"#,
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
        WHERE id = $1 AND status IN ('pending', 'running', 'waiting', 'resumable')
        RETURNING status
        "#,
        run_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row.map(|r| r.status))
}

/// Startup sweep: flip every still-pending/running row created BEFORE
/// `cutoff` to `failed`, EXCEPT the kinds whose per-`JobKind` policy spares them
/// (DEC-76). M-3: the `created_at < cutoff` bound prevents the sweep (spawned
/// detached at module init) from racing — and clobbering — a run legitimately
/// started in the boot window after `cutoff` was captured.
///
/// Disposition order (each step only touches rows still `pending`/`running`):
///  1. `resumable_agent = TRUE` → `resumable` (a crash inside an `agent` step,
///     ANY kind — re-driven from `agent_transcript_json`).
///  2. any `JobKind` whose registered `orphan_sweep` is `Resumable` (subagent)
///     → `resumable` — decentralized: iterate the policy registry, no hardcoded
///     kind list, so a new replayable kind auto-participates (ITEM-17/DEC-76).
///  3. everything else (`workflow`, `sandbox_exec`, any `Fail`-policy or
///     unknown kind — fail-closed) → `failed`.
pub async fn fail_orphaned_runs(
    pool: &PgPool,
    cutoff: time::OffsetDateTime,
) -> Result<u64, AppError> {
    // Step 1 — ITEM-17: FIRST spare crashed `running` agent runs — a run that
    // crashed while inside an agent step (`resumable_agent = true`) becomes
    // `resumable` (NOT `failed`) so the boot path re-drives it from its persisted
    // `agent_transcript_json`. This runs before the fail-sweep below, so those
    // rows are no longer `running` and the fail-sweep skips them.
    sqlx::query!(
        r#"
        UPDATE workflow_runs
        SET status = 'resumable',
            updated_at = NOW()
        WHERE status = 'running'
          AND resumable_agent = TRUE
          AND created_at < $1
        "#,
        cutoff,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    // Step 2 — DEC-76 per-JobKind policy: spare every orphaned run whose kind
    // declares a `Resumable` orphan-sweep (a sub-agent's work replays from its
    // durable transcript). Decentralized: walk the registered policies rather
    // than naming a kind, so a future replayable kind is swept correctly with no
    // edit here. A `Fail`-policy kind (workflow / sandbox_exec) is left for the
    // fail-sweep below; an UNKNOWN kind is fail-closed (never spared here).
    for policy in super::job_kind::JOB_KIND_POLICIES.iter() {
        if policy.orphan_sweep == super::job_kind::OrphanSweepPolicy::Resumable {
            sqlx::query!(
                r#"
                UPDATE workflow_runs
                SET status = 'resumable',
                    updated_at = NOW()
                WHERE status IN ('pending', 'running')
                  AND job_kind = $1
                  AND created_at < $2
                "#,
                policy.job_kind,
                cutoff,
            )
            .execute(pool)
            .await
            .map_err(AppError::database_error)?;
        }
    }

    // Step 3 — sweep in bounded batches rather than one mass UPDATE: a single
    // statement would take row locks on every matching orphan at once,
    // which on a large backlog holds a long write-lock span and bloats the
    // WAL in one transaction. Each batch commits independently (LIMIT via a
    // CTE; `FOR UPDATE SKIP LOCKED` so we never wait on a row another
    // connection already holds). Loop until a batch flips zero rows.
    const BATCH: i64 = 1000;
    let mut total: u64 = 0;
    loop {
        let res = sqlx::query!(
            r#"
            WITH batch AS (
                SELECT id FROM workflow_runs
                WHERE status IN ('pending', 'running')
                  AND created_at < $1
                ORDER BY created_at
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            UPDATE workflow_runs r
            SET status = 'failed',
                error_message = 'server restart during execution',
                updated_at = NOW()
            FROM batch
            WHERE r.id = batch.id
            "#,
            cutoff,
            BATCH,
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
        let n = res.rows_affected();
        total += n;
        if n < BATCH as u64 {
            break;
        }
    }
    Ok(total)
}

/// ITEM-17: every run parked in the `resumable` crash-resume state, oldest
/// first. The boot path re-drives each via `resume_run` after the startup sweep.
pub async fn list_resumable_run_ids(pool: &PgPool) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_scalar!(
        r#"SELECT id FROM workflow_runs WHERE status = 'resumable' ORDER BY created_at ASC"#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(ids)
}

pub async fn find_run(pool: &PgPool, run_id: Uuid) -> Result<Option<WorkflowRun>, AppError> {
    let row = sqlx::query_as!(
        WorkflowRun,
        r#"
        SELECT
            id,
            workflow_id,
            job_kind,
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
            step_progress_json as "step_progress_json: _",
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
            ephemeral,
            conversation_id,
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

/// All `scope = 'system'` workflows, unconditionally (NOT access-filtered by
/// group membership — mirrors skill's `list_system`). This is the admin
/// moderation / assignment-picker surface: a system workflow already assigned
/// to a group must still appear here, which the group-filtered `list_for_user`
/// would hide.
pub async fn list_system(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<Workflow>, AppError> {
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
            ephemeral,
            conversation_id,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflows
        WHERE scope = 'system' AND ephemeral = FALSE
        ORDER BY name ASC
        LIMIT $1 OFFSET $2
        "#,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

pub async fn get_system_workflows_for_group(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<Workflow>, AppError> {
    let rows = sqlx::query_as!(
        Workflow,
        r#"
        SELECT
            w.id,
            w.name,
            w.version,
            w.display_name,
            w.description,
            w.extracted_path,
            w.bundle_sha256,
            w.bundle_size_bytes,
            w.file_count,
            w.entry_point,
            w.tags as "tags: _",
            w.scope,
            w.owner_user_id,
            w.created_by,
            w.enabled,
            w.is_dev,
            w.ephemeral,
            w.conversation_id,
            w.compiled_ir_json as "compiled_ir_json: _",
            w.created_at as "created_at: _",
            w.updated_at as "updated_at: _"
        FROM workflows w
        INNER JOIN group_workflows gw ON w.id = gw.workflow_id
        WHERE gw.group_id = $1 AND w.scope = 'system'
        ORDER BY w.name ASC
        "#,
        group_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

pub async fn set_group_system_workflows(
    pool: &PgPool,
    group_id: Uuid,
    desired: &[Uuid],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;
    // group_workflows only ever holds system workflows (scope trigger), so
    // dropping every row for this group not in `desired` is a safe replace.
    sqlx::query!(
        "DELETE FROM group_workflows WHERE group_id = $1 AND NOT (workflow_id = ANY($2))",
        group_id,
        desired,
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;
    for workflow_id in desired {
        sqlx::query!(
            "INSERT INTO group_workflows (group_id, workflow_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            group_id,
            workflow_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }
    tx.commit().await.map_err(AppError::database_error)?;
    Ok(())
}

pub async fn count_system_workflows_in(pool: &PgPool, ids: &[Uuid]) -> Result<i64, AppError> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM workflows
        WHERE scope = 'system' AND id = ANY($1)
        "#,
        ids,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(count)
}

/// Recent runs owned by `user_id`, newest first, capped at `limit`.
/// Backs `workflow_mcp::resources::list` (recency-bounded resource
/// listing). B5.
/// A4: per-workflow run history for the owner (newest first, capped).
pub async fn list_runs_for_workflow(
    pool: &PgPool,
    workflow_id: Uuid,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<WorkflowRunSummary>, AppError> {
    let rows = sqlx::query_as!(
        WorkflowRunSummary,
        r#"
        SELECT id,
               -- workflow_id is now nullable (background runs), but this query
               -- filters WHERE workflow_id = $1, so every returned row has it
               -- set — force non-null so the summary DTO stays a plain Uuid.
               workflow_id as "workflow_id!",
               status, invocation_source,
               conversation_id, model_id, total_tokens,
               created_at as "created_at: _"
        FROM workflow_runs
        WHERE workflow_id = $1 AND user_id = $2
        ORDER BY created_at DESC
        LIMIT $3
        "#,
        workflow_id,
        user_id,
        limit,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows)
}

/// A5: hard-delete a run row. The `file_workflow_runs` join rows CASCADE-delete
/// with the run (chunk `ziee-file`), so any still-linked files survive (they
/// only lose the link) — the handler's cascade removes the run-owned ones first
/// when there's no conversation.
/// (run_id, conversation_id) for every run of a workflow. Used by the
/// workflow-delete path to clean up each run's on-disk artifacts before the
/// `workflow_runs` rows cascade away (which would otherwise orphan run-created
/// file blobs + staged dirs).
pub async fn list_run_refs_for_workflow(
    pool: &PgPool,
    workflow_id: Uuid,
) -> Result<Vec<(Uuid, Option<Uuid>)>, AppError> {
    let rows = sqlx::query!(
        r#"SELECT id, conversation_id FROM workflow_runs WHERE workflow_id = $1"#,
        workflow_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(rows.into_iter().map(|r| (r.id, r.conversation_id)).collect())
}

pub async fn delete_run_row(pool: &PgPool, run_id: Uuid) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM workflow_runs WHERE id = $1", run_id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    Ok(())
}

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
            job_kind,
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
            step_progress_json as "step_progress_json: _",
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

/// ITEM-8: the acting user's BACKGROUND runs (`job_kind <> 'workflow'`),
/// newest-first, paginated, with optional `status` / `kind` filters — a COMPACT
/// projection (no heavy JSONB blobs; `has_result` merely flags whether a
/// `final_output_json` exists, read fully via `collect_result`). Owner-scoped
/// (`user_id = $1`; a foreign run is simply absent — never leaked).
///
/// Index-friendly: the existing `(user_id, created_at DESC)` index
/// (`idx_workflow_runs_user_created`) serves the scan + ordering; `job_kind` and
/// `status` (both separately indexed) are residual filters. Returns
/// `(rows, total)` for the paginated response. Pushes every filter to SQL (§4 —
/// no in-memory filtering / N+1).
pub async fn list_background_runs_for_user(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    per_page: i64,
    status: Option<&str>,
    kind: Option<&str>,
) -> Result<(Vec<BackgroundRunSummary>, i64), AppError> {
    let per_page = per_page.clamp(1, 500);
    let offset = (page - 1).max(0) * per_page;

    let rows = sqlx::query_as!(
        BackgroundRunSummary,
        r#"
        SELECT
            id,
            job_kind,
            status,
            conversation_id,
            model_id,
            NULLIF(left(inputs_json->>'task', 200), '') as "label?",
            (final_output_json IS NOT NULL) as "has_result!",
            error_message,
            total_tokens,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM workflow_runs
        WHERE user_id = $1
          AND job_kind <> 'workflow'
          AND ($2::text IS NULL OR status = $2)
          AND ($3::text IS NULL OR job_kind = $3)
        ORDER BY created_at DESC
        LIMIT $4 OFFSET $5
        "#,
        user_id,
        status,
        kind,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let total = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM workflow_runs
        WHERE user_id = $1
          AND job_kind <> 'workflow'
          AND ($2::text IS NULL OR status = $2)
          AND ($3::text IS NULL OR job_kind = $3)
        "#,
        user_id,
        status,
        kind,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?
    .count;

    Ok((rows, total))
}

// ── ITEM-25: durable steering-note queue (Group F) ──────────────────────────
//
// A user posts a note to a RUNNING background run; the detached agent-core loop
// consumes pending notes at its next iteration boundary. The REST enqueue/list
// live in `background_mcp` (owns `/api/background/` + `background::use`); the
// durable STORAGE + these repository fns are the backbone's (the run-notes table
// FKs `workflow_runs`). Ownership is the CALLER'S job — every REST path resolves
// the run via `find_run_for_owner` FIRST, so these fns are owner-agnostic
// (mirrors `insert_background_run`).

/// DEC-79 bounded depth: keep at most this many PENDING (unconsumed) notes per
/// run. `enqueue_run_note` drop-oldest-trims to this bound on insert.
pub const MAX_PENDING_RUN_NOTES: i64 = 8;

/// Enqueue one steering note against a background run (ITEM-25). Honors DEC-79's
/// bounded depth: within ONE txn it drop-oldest-trims the pending set to
/// `MAX_PENDING_RUN_NOTES - 1` BEFORE inserting, so at most 8 notes stay pending
/// (the newest win). Owner-agnostic — the REST handler resolves ownership first.
pub async fn enqueue_run_note(
    pool: &PgPool,
    run_id: Uuid,
    note: &str,
) -> Result<RunNote, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Drop-oldest: delete the oldest pending rows beyond the newest (MAX-1), so
    // that after the insert below the pending count is exactly <= MAX.
    sqlx::query!(
        r#"DELETE FROM background_run_notes
           WHERE id IN (
               SELECT id FROM background_run_notes
               WHERE run_id = $1 AND consumed_at IS NULL
               ORDER BY created_at DESC OFFSET $2
           )"#,
        run_id,
        (MAX_PENDING_RUN_NOTES - 1).max(0),
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    let row = sqlx::query_as!(
        RunNote,
        r#"INSERT INTO background_run_notes (run_id, note)
           VALUES ($1, $2)
           RETURNING id, run_id, note,
                     created_at as "created_at: _",
                     consumed_at as "consumed_at: _""#,
        run_id,
        note,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(row)
}

/// List a run's PENDING (unconsumed) steering notes, oldest-first. Owner-agnostic
/// (the REST GET resolves the run's owner first). Backs the `GET` list endpoint.
pub async fn list_pending_run_notes(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Vec<RunNote>, AppError> {
    sqlx::query_as!(
        RunNote,
        r#"SELECT id, run_id, note,
                  created_at as "created_at: _",
                  consumed_at as "consumed_at: _"
           FROM background_run_notes
           WHERE run_id = $1 AND consumed_at IS NULL
           ORDER BY created_at ASC"#,
        run_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// CONSUME a run's pending steering notes: atomically stamp them consumed and
/// return them oldest-first. Idempotent per note — a second call returns only
/// notes queued since (already-consumed rows are skipped).
///
/// THIS IS THE SEAM THE AGENT-CORE LOOP CALLS (ITEM-25 / DEC-79 — now WIRED).
/// `agent_dispatch::RunNoteSteerPort` backs `agent_core::SteerNotePort` with this
/// fn; `build_detached_agent_core` threads it into the `AgentCore` ONLY for the
/// background sub-agent driver (`background_mcp::execute_subagent_run`). At each
/// iteration boundary (after cancel/budget, before `run_contribute` /
/// `transcript.load`) `AgentCore::run` drains the pending notes and appends each
/// as a `[steering]` user message so it reaches the model on the next call.
pub async fn consume_pending_run_notes(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Vec<RunNote>, AppError> {
    let mut rows = sqlx::query_as!(
        RunNote,
        r#"UPDATE background_run_notes
           SET consumed_at = now()
           WHERE run_id = $1 AND consumed_at IS NULL
           RETURNING id, run_id, note,
                     created_at as "created_at: _",
                     consumed_at as "consumed_at: _""#,
        run_id,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;
    // RETURNING order isn't guaranteed — deliver oldest-first (turn order).
    rows.sort_by_key(|n| n.created_at);
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    // DB-gated in-source tests (backbone / ITEM-14/17/29). They SOFT-SKIP when
    // `DATABASE_URL` is unset / unreachable (mirroring the suite's env-gated
    // real-stack tests, e.g. `web_search::repository`), so `cargo test --lib`
    // without Postgres stays green; they run for real wherever `DATABASE_URL`
    // points at a migrated DB (which includes migration 202607190700).
    async fn connect() -> Option<PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = match PgPoolOptions::new().max_connections(2).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return None;
            }
        };
        // Probe: soft-skip if the ambient DB predates migration 202607190700
        // (an un-migrated `postgres` DB), so these never HARD-fail — they only
        // run against a DB where the backbone columns exist.
        let migrated: Option<String> = sqlx::query_scalar(
            "SELECT column_name FROM information_schema.columns \
             WHERE table_schema='public' AND table_name='workflow_runs' \
               AND column_name='job_kind'",
        )
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();
        if migrated.is_none() {
            eprintln!("skip: workflow_runs.job_kind absent — DB not migrated to 202607190700");
            return None;
        }
        Some(pool)
    }

    /// Insert a throwaway user (the `workflow_runs.user_id` FK target, SDK-owned
    /// `users` table). Uses a UUID-derived unique username/email so parallel
    /// in-source tests never collide. Deleting this user CASCADE-removes its runs
    /// (`workflow_runs_user_id_fkey ON DELETE CASCADE`) — the sole cleanup.
    async fn make_user(pool: &PgPool) -> Uuid {
        let uid = Uuid::new_v4();
        // Runtime (unchecked) query: keeps the test decoupled from the exact
        // SDK users schema (only username + email are required NOT-NULL columns).
        sqlx::query("INSERT INTO users (id, username, email) VALUES ($1, $2, $3)")
            .bind(uid)
            .bind(format!("bgtest_{}", uid.simple()))
            .bind(format!("bgtest_{}@example.invalid", uid.simple()))
            .execute(pool)
            .await
            .expect("insert throwaway user");
        uid
    }

    async fn cleanup_user(pool: &PgPool, user_id: Uuid) {
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    fn bg_req(user_id: Uuid, kind: JobKind) -> CreateBackgroundRun {
        CreateBackgroundRun {
            job_kind: kind,
            conversation_id: None,
            user_id,
            model_id: None,
            sandbox_flavor: None,
            invocation_source: "conversation".into(),
            inputs_json: serde_json::json!({}),
        }
    }

    // TEST-47 (ITEM-14): a `job_kind='sandbox_exec'` + NULL `workflow_id` row
    // persists / round-trips, its heartbeat bumps `updated_at`, and it reaches a
    // terminal state — all WITHOUT ever loading a `workflow.yaml`. The late
    // terminal write is a CAS no-op (TEST-132 end-to-end).
    #[tokio::test]
    async fn background_run_round_trips_and_heartbeats_without_workflow_yaml() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;

        // Persist a bundle-less background run.
        let row = insert_background_run(&pool, bg_req(user_id, JobKind::SandboxExec))
            .await
            .expect("insert background run");
        assert!(row.workflow_id.is_none(), "background run has NULL workflow_id");
        assert_eq!(row.job_kind, "sandbox_exec");
        assert_eq!(JobKind::from_db_str(&row.job_kind), Some(JobKind::SandboxExec));
        assert_eq!(row.status, "pending");
        let run_id = row.id;

        // pending → running via the guarded transition.
        mark_running(&pool, run_id).await.expect("mark_running");
        let running = find_run(&pool, run_id).await.unwrap().unwrap();
        assert_eq!(running.status, "running");
        let before = running.updated_at;

        // Heartbeat bumps updated_at (the no-progress liveness signal) without
        // changing status.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        heartbeat(&pool, run_id).await.expect("heartbeat");
        let beat = find_run(&pool, run_id).await.unwrap().unwrap();
        assert!(
            beat.updated_at > before,
            "heartbeat must advance updated_at ({:?} !> {:?})",
            beat.updated_at,
            before
        );
        assert_eq!(beat.status, "running", "heartbeat must not change status");

        // Reach a terminal state; no workflow bundle was ever loaded.
        set_final_output(&pool, run_id, serde_json::json!({"exit_code": 0}))
            .await
            .unwrap();
        mark_status(&pool, run_id, WorkflowRunStatus::Completed, None)
            .await
            .expect("mark completed");
        let done = find_run(&pool, run_id).await.unwrap().unwrap();
        assert_eq!(done.status, "completed");
        assert!(WorkflowRunStatus::from_db_str(&done.status).unwrap().is_terminal());
        assert_eq!(done.final_output_json, Some(serde_json::json!({"exit_code": 0})));

        // TEST-132: a LATE terminal write is a CAS no-op — the completed row is
        // never resurrected to failed.
        mark_status(&pool, run_id, WorkflowRunStatus::Failed, Some("late"))
            .await
            .unwrap();
        let still = find_run(&pool, run_id).await.unwrap().unwrap();
        assert_eq!(still.status, "completed", "terminal CAS must be a no-op");

        // Owner-scoped read seam: owner sees it, a stranger gets None (→ 404).
        assert!(find_run_for_owner(&pool, run_id, user_id).await.unwrap().is_some());
        assert!(
            find_run_for_owner(&pool, run_id, Uuid::new_v4())
                .await
                .unwrap()
                .is_none(),
            "cross-user fetch must be None (404), never leak the row"
        );

        cleanup_user(&pool, user_id).await;
    }

    // TEST-47 (spawn seam): `runner::spawn_background_run` fire-and-forgets a
    // background run reusing mark_running + the heartbeat + the guarded terminal
    // `mark_status`, reaching `completed` with the driver's final output — with no
    // workflow.yaml.
    #[tokio::test]
    async fn spawn_background_run_drives_to_terminal() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;

        let run_id = crate::modules::workflow::runner::spawn_background_run(
            &pool,
            bg_req(user_id, JobKind::SubAgent),
            |_pool, _run_id, _handle| async move {
                crate::modules::workflow::runner::BackgroundOutcome::Completed {
                    final_output: Some(serde_json::json!({"answer": 42})),
                }
            },
        )
        .await
        .expect("spawn background run");

        // Poll for terminal (the driver completes immediately; allow slack for
        // the detached task + terminal write).
        let mut terminal = None;
        for _ in 0..100 {
            let run = find_run(&pool, run_id).await.unwrap().unwrap();
            if WorkflowRunStatus::from_db_str(&run.status)
                .is_some_and(|s| s.is_terminal())
            {
                terminal = Some(run);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let run = terminal.expect("background run should reach terminal");
        assert_eq!(run.status, "completed");
        assert!(run.workflow_id.is_none());
        assert_eq!(run.job_kind, "subagent");
        assert_eq!(run.final_output_json, Some(serde_json::json!({"answer": 42})));

        cleanup_user(&pool, user_id).await;
    }

    // TEST-42 / TEST-133 (ITEM-29 / DEC-76): the boot orphan-sweep applies a
    // PER-JobKind policy — a crashed `subagent` orphan is SPARED as `resumable`
    // (replayable), a `sandbox_exec` orphan is `failed` (a killed subprocess
    // can't replay). Both rows are aged past the cutoff so ONLY they are swept
    // (the sweep is global; other tests' fresh rows are newer than the cutoff).
    #[tokio::test]
    async fn boot_sweep_is_per_job_kind() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;

        let sub = insert_background_run(&pool, bg_req(user_id, JobKind::SubAgent))
            .await
            .unwrap();
        let sand = insert_background_run(&pool, bg_req(user_id, JobKind::SandboxExec))
            .await
            .unwrap();
        // Simulate crashed in-flight orphans aged 2 days (older than the cutoff).
        for id in [sub.id, sand.id] {
            sqlx::query(
                "UPDATE workflow_runs SET status='running', created_at = now() - interval '2 days' WHERE id = $1",
            )
            .bind(id)
            .execute(&pool)
            .await
            .expect("age the orphan row");
        }

        // Cutoff 1 day ago: sweeps ONLY rows older than that (our 2-day rows),
        // never a concurrently-running test's ~now rows.
        let cutoff = time::OffsetDateTime::now_utc() - time::Duration::days(1);
        fail_orphaned_runs(&pool, cutoff).await.expect("sweep");

        let sub_after = find_run(&pool, sub.id).await.unwrap().unwrap();
        let sand_after = find_run(&pool, sand.id).await.unwrap().unwrap();
        assert_eq!(
            sub_after.status, "resumable",
            "subagent orphan must be spared as resumable (transcript replay)"
        );
        assert_eq!(
            sand_after.status, "failed",
            "sandbox_exec orphan must fail (subprocess gone)"
        );

        cleanup_user(&pool, user_id).await;
    }

    // ── ITEM-25: steering-note queue ────────────────────────────────────────

    /// enqueue → list-pending roundtrip, then consume marks them consumed
    /// (idempotent: a second consume yields nothing, and list-pending empties).
    #[tokio::test]
    async fn run_notes_enqueue_list_consume_roundtrip() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;
        let run = insert_background_run(&pool, bg_req(user_id, JobKind::SubAgent))
            .await
            .expect("insert subagent run");

        // Two notes queue and list oldest-first, both pending.
        let a = enqueue_run_note(&pool, run.id, "check the second table too")
            .await
            .expect("enqueue a");
        let b = enqueue_run_note(&pool, run.id, "prefer the 2024 revision")
            .await
            .expect("enqueue b");
        assert!(a.consumed_at.is_none() && b.consumed_at.is_none());

        let pending = list_pending_run_notes(&pool, run.id).await.unwrap();
        assert_eq!(pending.len(), 2, "both notes pending");
        assert_eq!(pending[0].note, "check the second table too", "oldest-first");
        assert_eq!(pending[1].note, "prefer the 2024 revision");

        // Consume returns both, oldest-first, and stamps consumed_at.
        let consumed = consume_pending_run_notes(&pool, run.id).await.unwrap();
        assert_eq!(consumed.len(), 2);
        assert_eq!(consumed[0].note, "check the second table too");
        assert!(consumed.iter().all(|n| n.consumed_at.is_some()));

        // Idempotent: pending is now empty; a second consume yields nothing.
        assert!(list_pending_run_notes(&pool, run.id).await.unwrap().is_empty());
        assert!(consume_pending_run_notes(&pool, run.id).await.unwrap().is_empty());

        cleanup_user(&pool, user_id).await;
    }

    /// DEC-79 bounded depth: enqueuing past `MAX_PENDING_RUN_NOTES` drops the
    /// OLDEST pending — only the newest 8 survive.
    #[tokio::test]
    async fn run_notes_enqueue_is_bounded_drop_oldest() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;
        let run = insert_background_run(&pool, bg_req(user_id, JobKind::SubAgent))
            .await
            .expect("insert subagent run");

        let over = MAX_PENDING_RUN_NOTES + 1; // 9
        for i in 0..over {
            enqueue_run_note(&pool, run.id, &format!("note-{i}"))
                .await
                .expect("enqueue");
            // Distinct created_at so the drop-oldest ORDER BY is deterministic.
            tokio::time::sleep(std::time::Duration::from_millis(3)).await;
        }

        let pending = list_pending_run_notes(&pool, run.id).await.unwrap();
        assert_eq!(
            pending.len() as i64,
            MAX_PENDING_RUN_NOTES,
            "pending is capped at MAX_PENDING_RUN_NOTES"
        );
        // note-0 (oldest) was dropped; note-1..note-8 remain, oldest-first.
        assert_eq!(pending[0].note, "note-1", "oldest surviving is note-1 (note-0 dropped)");
        assert_eq!(pending.last().unwrap().note, format!("note-{}", over - 1));

        cleanup_user(&pool, user_id).await;
    }

    /// Deleting the run CASCADES its notes away (FK ON DELETE CASCADE).
    #[tokio::test]
    async fn run_notes_cascade_on_run_delete() {
        let Some(pool) = connect().await else {
            eprintln!("skip: DATABASE_URL unset");
            return;
        };
        let user_id = make_user(&pool).await;
        let run = insert_background_run(&pool, bg_req(user_id, JobKind::SubAgent))
            .await
            .expect("insert subagent run");
        enqueue_run_note(&pool, run.id, "will be cascaded")
            .await
            .expect("enqueue");
        assert_eq!(list_pending_run_notes(&pool, run.id).await.unwrap().len(), 1);

        delete_run_row(&pool, run.id).await.expect("delete run");

        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM background_run_notes WHERE run_id = $1")
                .bind(run.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0, "notes are deleted with their run (cascade)");

        cleanup_user(&pool, user_id).await;
    }
}

//! Singleton admin-settings repository for the agent module.
//!
//! One row keyed on `id = TRUE`. Uses runtime-checked `sqlx::query_as` (no
//! compile-time `query!` macros) — the DTO's `sqlx::FromRow` maps columns by
//! name. Mirrors `js_tool::settings` + `summarization::repository`.

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::common::AppError;

use super::models::{AgentAdminSettings, UpdateAgentAdminSettingsRequest};

#[derive(Clone, Debug)]
pub struct AgentRepository {
    pool: PgPool,
}

impl AgentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn missing() -> AppError {
        AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "AGENT_SETTINGS_MISSING",
            "agent_admin_settings singleton row is missing — run migrations",
        )
    }

    /// Read the singleton settings row. Always present after migration.
    pub async fn get_admin_settings(&self) -> Result<AgentAdminSettings, AppError> {
        let row: Option<AgentAdminSettings> = sqlx::query_as(
            r#"
            SELECT default_sandbox_mode, unattended_approval_policy, reviewer_enabled,
                   reviewer_model_id, reviewer_policy, reviewer_risk_thresholds,
                   per_run_token_cap, per_step_token_cap, default_max_steps,
                   fan_out_max_threads, fan_out_max_depth,
                   fan_out_max_children_per_call,
                   goal_eval_model_id, goal_seek_max_turns, updated_at
            FROM agent_admin_settings
            WHERE id = TRUE
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        row.ok_or_else(Self::missing)
    }

    /// PATCH-style update. Non-null columns use COALESCE (absent = unchanged);
    /// the two nullable columns use the CASE-WHEN tri-state (absent =
    /// unchanged, explicit null = clear). Validation happens at the call site
    /// (`UpdateAgentAdminSettingsRequest::validate`).
    pub async fn update_admin_settings(
        &self,
        patch: &UpdateAgentAdminSettingsRequest,
    ) -> Result<AgentAdminSettings, AppError> {
        // Tri-state split: outer Some ⇒ "client sent this key"; inner
        // Some/None ⇒ value-vs-null. The boolean drives the CASE WHEN.
        let model_set = patch.reviewer_model_id.is_some();
        let model_val = patch.reviewer_model_id.flatten();
        let policy_set = patch.reviewer_policy.is_some();
        let policy_val = patch.reviewer_policy.clone().flatten();
        let goal_model_set = patch.goal_eval_model_id.is_some();
        let goal_model_val = patch.goal_eval_model_id.flatten();

        let row: Option<AgentAdminSettings> = sqlx::query_as(
            r#"
            UPDATE agent_admin_settings SET
                default_sandbox_mode        = COALESCE($1, default_sandbox_mode),
                unattended_approval_policy  = COALESCE($2, unattended_approval_policy),
                reviewer_enabled            = COALESCE($3, reviewer_enabled),
                reviewer_model_id           = CASE WHEN $4::bool THEN $5 ELSE reviewer_model_id END,
                reviewer_policy             = CASE WHEN $6::bool THEN $7 ELSE reviewer_policy END,
                reviewer_risk_thresholds    = COALESCE($8, reviewer_risk_thresholds),
                per_run_token_cap           = COALESCE($9, per_run_token_cap),
                per_step_token_cap          = COALESCE($10, per_step_token_cap),
                default_max_steps           = COALESCE($11, default_max_steps),
                fan_out_max_threads         = COALESCE($12, fan_out_max_threads),
                fan_out_max_depth           = COALESCE($13, fan_out_max_depth),
                fan_out_max_children_per_call = COALESCE($14, fan_out_max_children_per_call),
                goal_eval_model_id          = CASE WHEN $15::bool THEN $16 ELSE goal_eval_model_id END,
                goal_seek_max_turns         = COALESCE($17, goal_seek_max_turns),
                updated_at                  = NOW()
            WHERE id = TRUE
            RETURNING default_sandbox_mode, unattended_approval_policy, reviewer_enabled,
                      reviewer_model_id, reviewer_policy, reviewer_risk_thresholds,
                      per_run_token_cap, per_step_token_cap, default_max_steps,
                      fan_out_max_threads, fan_out_max_depth,
                      fan_out_max_children_per_call,
                      goal_eval_model_id, goal_seek_max_turns, updated_at
            "#,
        )
        .bind(patch.default_sandbox_mode.as_deref())
        .bind(patch.unattended_approval_policy.as_deref())
        .bind(patch.reviewer_enabled)
        .bind(model_set)
        .bind(model_val)
        .bind(policy_set)
        .bind(policy_val)
        .bind(patch.reviewer_risk_thresholds.as_ref())
        .bind(patch.per_run_token_cap)
        .bind(patch.per_step_token_cap)
        .bind(patch.default_max_steps)
        .bind(patch.fan_out_max_threads)
        .bind(patch.fan_out_max_depth)
        .bind(patch.fan_out_max_children_per_call)
        .bind(goal_model_set)
        .bind(goal_model_val)
        .bind(patch.goal_seek_max_turns)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            // Surface a CHECK-constraint violation as 400 so the UI renders it
            // rather than an opaque 500 (validate() catches most cases first).
            if let sqlx::Error::Database(db) = &e
                && db.constraint().is_some()
            {
                return AppError::bad_request(
                    "VALIDATION_ERROR",
                    format!("value rejected by DB constraint {:?}", db.constraint()),
                );
            }
            AppError::database_error(e)
        })?;
        row.ok_or_else(Self::missing)
    }
}

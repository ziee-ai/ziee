// Database repository for local runtime management

use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::common::AppError;
use super::runtime_settings::models::{RuntimeSettings, UpdateRuntimeSettingsRequest};

type AppResult<T> = Result<T, AppError>;

// =====================================================
// Repository Struct
// =====================================================

#[derive(Clone)]
pub struct LocalRuntimeRepository {
    pool: PgPool,
}

impl LocalRuntimeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =====================================================
    // Runtime Instance Methods
    // =====================================================

    /// Create a new runtime instance record
    pub async fn create_instance(
        &self,
        model_id: Uuid,
        provider_id: Uuid,
        local_port: i32,
        base_url: &str,
        runtime_version_id: Option<Uuid>,
    ) -> AppResult<Uuid> {
        let record = sqlx::query!(
            r#"
            INSERT INTO llm_runtime_instances
                (model_id, provider_id, local_port, base_url, status, runtime_version_id)
            VALUES ($1, $2, $3, $4, 'starting', $5)
            RETURNING id
            "#,
            model_id,
            provider_id,
            local_port,
            base_url,
            runtime_version_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db_err) = &e
                && db_err.is_unique_violation() {
                    return AppError::conflict("Runtime instance");
                }
            AppError::internal_error(format!("Failed to create runtime instance: {}", e))
        })?;

        Ok(record.id)
    }

    /// Get instance by model ID
    pub async fn get_instance_by_model(
        &self,
        model_id: Uuid,
    ) -> AppResult<Option<RuntimeInstance>> {
        let instance = sqlx::query_as!(
            RuntimeInstance,
            r#"
            SELECT id, model_id, provider_id, local_port, base_url, status,
                   error_message, runtime_version_id,
                   state, state_changed_at as "state_changed_at: _",
                   restart_attempts, last_failure_reason,
                   last_used_at as "last_used_at: _",
                   started_at as "started_at: _",
                   last_health_check as "last_health_check: _",
                   stopped_at as "stopped_at: _"
            FROM llm_runtime_instances
            WHERE model_id = $1
            "#,
            model_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            AppError::internal_error(format!("Failed to get runtime instance: {}", e))
        })?;

        Ok(instance)
    }

    /// Update instance status
    pub async fn update_instance_status(
        &self,
        model_id: Uuid,
        status: &str,
        error_message: Option<&str>,
    ) -> AppResult<()> {
        // Update stopped_at separately if status is 'stopped'
        if status == "stopped" {
            let result = sqlx::query!(
                r#"
                UPDATE llm_runtime_instances
                SET status = $1,
                    error_message = $2,
                    last_health_check = CURRENT_TIMESTAMP,
                    stopped_at = CURRENT_TIMESTAMP
                WHERE model_id = $3
                "#,
                status,
                error_message,
                model_id
            )
            .execute(&self.pool)
            .await
            .map_err(|e| {
                AppError::internal_error(format!("Failed to update instance status: {}", e))
            })?;

            if result.rows_affected() == 0 {
                return Err(AppError::not_found("Runtime instance"));
            }

            return Ok(());
        }

        let result = sqlx::query!(
            r#"
            UPDATE llm_runtime_instances
            SET status = $1,
                error_message = $2,
                last_health_check = CURRENT_TIMESTAMP
            WHERE model_id = $3
            "#,
            status,
            error_message,
            model_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            AppError::internal_error(format!("Failed to update instance status: {}", e))
        })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found("Runtime instance"));
        }

        Ok(())
    }

    /// Delete instance record
    pub async fn delete_instance(
        &self,
        model_id: Uuid,
    ) -> AppResult<()> {
        let result = sqlx::query!(
            r#"
            DELETE FROM llm_runtime_instances
            WHERE model_id = $1
            "#,
            model_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            AppError::internal_error(format!("Failed to delete runtime instance: {}", e))
        })?;

        if result.rows_affected() == 0 {
            return Err(AppError::not_found("Runtime instance"));
        }

        Ok(())
    }

    /// Get all instances for a provider
    pub async fn get_instances_by_provider(
        &self,
        provider_id: Uuid,
    ) -> AppResult<Vec<RuntimeInstance>> {
        let instances = sqlx::query_as!(
            RuntimeInstance,
            r#"
            SELECT id, model_id, provider_id, local_port, base_url, status,
                   error_message, runtime_version_id,
                   state, state_changed_at as "state_changed_at: _",
                   restart_attempts, last_failure_reason,
                   last_used_at as "last_used_at: _",
                   started_at as "started_at: _",
                   last_health_check as "last_health_check: _",
                   stopped_at as "stopped_at: _"
            FROM llm_runtime_instances
            WHERE provider_id = $1
            ORDER BY started_at DESC
            "#,
            provider_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            AppError::internal_error(format!("Failed to get provider instances: {}", e))
        })?;

        Ok(instances)
    }

    // =====================================================
    // Runtime Settings (singleton) Methods
    // =====================================================

    /// Load the singleton runtime-settings row.
    pub async fn get_runtime_settings(&self) -> AppResult<RuntimeSettings> {
        // TIMESTAMPTZ decodes to time::OffsetDateTime by default in
        // this project; the `: chrono::DateTime<Utc>` override forces
        // the chrono type RuntimeSettings uses.
        sqlx::query_as!(
            RuntimeSettings,
            r#"SELECT idle_unload_secs, auto_start_timeout_secs, drain_timeout_secs,
                    created_at as "created_at: chrono::DateTime<chrono::Utc>",
                    updated_at as "updated_at: chrono::DateTime<chrono::Utc>"
             FROM llm_runtime_settings WHERE id = TRUE"#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal_error(format!("runtime_settings: get: {e}")))
    }

    /// Update the singleton runtime-settings row (PATCH-style COALESCE
    /// on each field). Validates ranges before touching the DB.
    pub async fn update_runtime_settings(
        &self,
        req: UpdateRuntimeSettingsRequest,
    ) -> AppResult<RuntimeSettings> {
        if let Some(v) = req.idle_unload_secs {
            if !(0..=86_400).contains(&v) {
                return Err(AppError::bad_request(
                    "RUNTIME_SETTINGS_OUT_OF_RANGE",
                    "idle_unload_secs must be 0 (disabled) or 1..86400 (≤24h)",
                ));
            }
        }
        if let Some(v) = req.auto_start_timeout_secs {
            if !(1..=600).contains(&v) {
                return Err(AppError::bad_request(
                    "RUNTIME_SETTINGS_OUT_OF_RANGE",
                    "auto_start_timeout_secs must be 1..600",
                ));
            }
        }
        if let Some(v) = req.drain_timeout_secs {
            if !(1..=600).contains(&v) {
                return Err(AppError::bad_request(
                    "RUNTIME_SETTINGS_OUT_OF_RANGE",
                    "drain_timeout_secs must be 1..600",
                ));
            }
        }

        sqlx::query_as!(
            RuntimeSettings,
            r#"UPDATE llm_runtime_settings SET
                idle_unload_secs        = COALESCE($1, idle_unload_secs),
                auto_start_timeout_secs = COALESCE($2, auto_start_timeout_secs),
                drain_timeout_secs      = COALESCE($3, drain_timeout_secs),
                updated_at = NOW()
             WHERE id = TRUE
             RETURNING idle_unload_secs, auto_start_timeout_secs, drain_timeout_secs,
                       created_at as "created_at: chrono::DateTime<chrono::Utc>",
                       updated_at as "updated_at: chrono::DateTime<chrono::Utc>""#,
            req.idle_unload_secs,
            req.auto_start_timeout_secs,
            req.drain_timeout_secs,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::internal_error(format!("runtime_settings: update: {e}")))
    }

}

// =====================================================
// Database Models
// =====================================================

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RuntimeInstance {
    pub id: Uuid,
    pub model_id: Uuid,
    pub provider_id: Uuid,
    pub local_port: i32,
    pub base_url: String,
    pub status: String,
    pub error_message: Option<String>,
    pub runtime_version_id: Option<Uuid>,
    pub started_at: DateTime<Utc>,
    pub last_health_check: Option<DateTime<Utc>>,
    pub stopped_at: Option<DateTime<Utc>>,
    // Migration 0066: health state machine columns
    pub state: String,
    pub state_changed_at: DateTime<Utc>,
    pub restart_attempts: i32,
    pub last_failure_reason: Option<String>,
    // Migration 0068: last-used timestamp for idle eviction
    pub last_used_at: DateTime<Utc>,
}

//! Persistence for the voice module: the settings singleton (+ version /
//! instance rows in the runtime_version / deployment layers).

use sqlx::PgPool;

use crate::common::AppError;

use super::models::VoiceSettings;

pub struct VoiceRepository {
    pool: PgPool,
}

impl VoiceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the singleton settings row.
    pub async fn get_settings(&self) -> Result<VoiceSettings, AppError> {
        let row = sqlx::query_as!(
            VoiceSettings,
            r#"
            SELECT
                enabled,
                model,
                language,
                idle_unload_secs,
                auto_start_timeout_secs,
                drain_timeout_secs,
                max_clip_seconds,
                max_upload_bytes,
                streaming_enabled,
                stream_interval_ms,
                updated_at as "updated_at: _"
            FROM voice_runtime_settings
            WHERE id = TRUE
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// COALESCE-patch update of the singleton settings row. Every argument is
    /// optional; `None` leaves the column unchanged.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_settings(
        &self,
        enabled: Option<bool>,
        model: Option<String>,
        language: Option<String>,
        idle_unload_secs: Option<i32>,
        auto_start_timeout_secs: Option<i32>,
        drain_timeout_secs: Option<i32>,
        max_clip_seconds: Option<i32>,
        max_upload_bytes: Option<i64>,
        streaming_enabled: Option<bool>,
        stream_interval_ms: Option<i32>,
    ) -> Result<VoiceSettings, AppError> {
        let row = sqlx::query_as!(
            VoiceSettings,
            r#"
            UPDATE voice_runtime_settings
            SET
                enabled = COALESCE($1, enabled),
                model = COALESCE($2, model),
                language = COALESCE($3, language),
                idle_unload_secs = COALESCE($4, idle_unload_secs),
                auto_start_timeout_secs = COALESCE($5, auto_start_timeout_secs),
                drain_timeout_secs = COALESCE($6, drain_timeout_secs),
                max_clip_seconds = COALESCE($7, max_clip_seconds),
                max_upload_bytes = COALESCE($8, max_upload_bytes),
                streaming_enabled = COALESCE($9, streaming_enabled),
                stream_interval_ms = COALESCE($10, stream_interval_ms),
                updated_at = NOW()
            WHERE id = TRUE
            RETURNING
                enabled,
                model,
                language,
                idle_unload_secs,
                auto_start_timeout_secs,
                drain_timeout_secs,
                max_clip_seconds,
                max_upload_bytes,
                streaming_enabled,
                stream_interval_ms,
                updated_at as "updated_at: _"
            "#,
            enabled,
            model,
            language,
            idle_unload_secs,
            auto_start_timeout_secs,
            drain_timeout_secs,
            max_clip_seconds,
            max_upload_bytes,
            streaming_enabled,
            stream_interval_ms,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}

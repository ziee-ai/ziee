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
                stream_max_decode_secs,
                model_source_repo,
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
        stream_max_decode_secs: Option<i32>,
        model_source_repo: Option<String>,
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
                stream_max_decode_secs = COALESCE($11, stream_max_decode_secs),
                model_source_repo = COALESCE($12, model_source_repo),
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
                stream_max_decode_secs,
                model_source_repo,
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
            stream_max_decode_secs,
            model_source_repo,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }
}

/// A raw `voice_models` row (DB projection; `is_active`/`update_available` are
/// derived at read time, not stored).
pub struct VoiceModelRow {
    pub id: uuid::Uuid,
    pub name: String,
    pub filename: String,
    pub source: super::models::VoiceModelSource,
    pub source_url: Option<String>,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub verified: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// CRUD for the installed whisper-model set. Mirrors the runtime_version repo.
pub struct VoiceModelRepository {
    pool: PgPool,
}

impl VoiceModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<VoiceModelRow>, AppError> {
        let rows = sqlx::query_as!(
            VoiceModelRow,
            r#"
            SELECT id, name, filename,
                   source as "source: super::models::VoiceModelSource",
                   source_url, size_bytes, sha256, verified,
                   created_at as "created_at: _"
            FROM voice_models
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows)
    }

    pub async fn get_by_id(&self, id: uuid::Uuid) -> Result<Option<VoiceModelRow>, AppError> {
        let row = sqlx::query_as!(
            VoiceModelRow,
            r#"
            SELECT id, name, filename,
                   source as "source: super::models::VoiceModelSource",
                   source_url, size_bytes, sha256, verified,
                   created_at as "created_at: _"
            FROM voice_models WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn get_by_name(&self, name: &str) -> Result<Option<VoiceModelRow>, AppError> {
        let row = sqlx::query_as!(
            VoiceModelRow,
            r#"
            SELECT id, name, filename,
                   source as "source: super::models::VoiceModelSource",
                   source_url, size_bytes, sha256, verified,
                   created_at as "created_at: _"
            FROM voice_models WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    /// Insert (or upsert on filename — a re-download of the same file updates the
    /// recorded size/sha/verified/source in place).
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert(
        &self,
        name: &str,
        filename: &str,
        source: super::models::VoiceModelSource,
        source_url: Option<&str>,
        size_bytes: i64,
        sha256: Option<&str>,
        verified: bool,
    ) -> Result<VoiceModelRow, AppError> {
        let row = sqlx::query_as!(
            VoiceModelRow,
            r#"
            INSERT INTO voice_models (name, filename, source, source_url, size_bytes, sha256, verified)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (filename) DO UPDATE SET
                name = EXCLUDED.name,
                source = EXCLUDED.source,
                source_url = EXCLUDED.source_url,
                size_bytes = EXCLUDED.size_bytes,
                sha256 = EXCLUDED.sha256,
                verified = EXCLUDED.verified
            RETURNING id, name, filename,
                   source as "source: super::models::VoiceModelSource",
                   source_url, size_bytes, sha256, verified,
                   created_at as "created_at: _"
            "#,
            name,
            filename,
            source as super::models::VoiceModelSource,
            source_url,
            size_bytes,
            sha256,
            verified,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row)
    }

    pub async fn delete(&self, id: uuid::Uuid) -> Result<bool, AppError> {
        let res = sqlx::query!("DELETE FROM voice_models WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(AppError::database_error)?;
        Ok(res.rows_affected() > 0)
    }
}

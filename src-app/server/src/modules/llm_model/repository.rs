// LLM model repository
#![allow(dead_code)]

// LLM Model database queries - copied from react-test and refactored for ziee-chat
// Source: react-test/src-tauri/src/database/queries/models.rs

use chrono::DateTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{
    DownloadInstance, DownloadPhase, DownloadProgressData, DownloadStatus, EngineType, FileFormat,
    LlmModel,
};
use super::types::{
    CreateDownloadInstanceRequest, CreateLlmModelRequest, DownloadInstanceListResponse,
    UpdateDownloadProgressRequest, UpdateDownloadStatusRequest, UpdateLlmModelRequest,
};

// Note: SQLx query_as! automatically handles type conversions including time crate types

// =====================================================
// LLM Model Repository (Public structs)
// =====================================================

/// Repository for LLM model database operations
#[derive(Clone, Debug)]
pub struct LlmModelRepository {
    pool: PgPool,
}

impl LlmModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new LLM model
    pub async fn create(&self, request: CreateLlmModelRequest) -> Result<LlmModel, AppError> {
        create_llm_model(&self.pool, request)
            .await
            .map_err(AppError::database_error)
    }

    /// Get LLM model by ID
    pub async fn get_by_id(&self, model_id: Uuid) -> Result<Option<LlmModel>, AppError> {
        get_llm_model_by_id(&self.pool, model_id)
            .await
            .map_err(AppError::database_error)
    }

    /// Set model validation status
    pub async fn set_validation_status(
        &self,
        model_id: Uuid,
        status: &str,
        issues: Option<Vec<String>>,
    ) -> Result<(), AppError> {
        set_model_validation_status(&self.pool, model_id, status, issues)
            .await
            .map_err(AppError::database_error)
    }

    /// List all LLM models across all providers
    pub async fn list_all(&self) -> Result<Vec<LlmModel>, AppError> {
        list_all_llm_models(&self.pool)
            .await
            .map_err(AppError::database_error)
    }

    /// List LLM models by provider ID
    pub async fn list_by_provider(&self, provider_id: Uuid) -> Result<Vec<LlmModel>, AppError> {
        list_llm_models_by_provider(&self.pool, provider_id)
            .await
            .map_err(AppError::database_error)
    }

    /// Update an existing LLM model
    pub async fn update(
        &self,
        model_id: Uuid,
        request: UpdateLlmModelRequest,
    ) -> Result<Option<LlmModel>, AppError> {
        update_llm_model(&self.pool, model_id, request)
            .await
            .map_err(AppError::database_error)
    }

    /// Delete an LLM model
    pub async fn delete(&self, model_id: Uuid) -> Result<bool, AppError> {
        delete_llm_model(&self.pool, model_id)
            .await
            .map_err(AppError::database_error)
    }
}

// =====================================================
// Download Instance Repository
// =====================================================

/// Repository for download instance database operations
#[derive(Clone, Debug)]
pub struct DownloadInstanceRepository {
    pool: PgPool,
}

impl DownloadInstanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new download instance
    pub async fn create(
        &self,
        request: CreateDownloadInstanceRequest,
    ) -> Result<DownloadInstance, AppError> {
        create_download_instance(&self.pool, request)
            .await
            .map_err(AppError::database_error)
    }

    /// Update download progress
    pub async fn update_progress(
        &self,
        download_id: Uuid,
        request: UpdateDownloadProgressRequest,
    ) -> Result<Option<DownloadInstance>, AppError> {
        update_download_progress(&self.pool, download_id, request)
            .await
            .map_err(AppError::database_error)
    }

    /// Update download status
    pub async fn update_status(
        &self,
        download_id: Uuid,
        request: UpdateDownloadStatusRequest,
    ) -> Result<Option<DownloadInstance>, AppError> {
        update_download_status(&self.pool, download_id, request)
            .await
            .map_err(AppError::database_error)
    }

    /// Get download instance by ID
    pub async fn get_by_id(&self, download_id: Uuid) -> Result<Option<DownloadInstance>, AppError> {
        get_download_instance_by_id(&self.pool, download_id)
            .await
            .map_err(AppError::database_error)
    }

    /// List download instances with pagination and optional status filter
    pub async fn list(
        &self,
        page: i32,
        per_page: i32,
        status_filter: Option<DownloadStatus>,
    ) -> Result<DownloadInstanceListResponse, AppError> {
        get_download_instances(&self.pool, page, per_page, status_filter)
            .await
            .map_err(AppError::database_error)
    }

    /// Delete a download instance
    pub async fn delete(&self, download_id: Uuid) -> Result<bool, AppError> {
        delete_download_instance(&self.pool, download_id)
            .await
            .map_err(AppError::database_error)
    }

    /// Get all active downloads (pending, downloading, failed, cancelled)
    pub async fn get_all_active(&self) -> Result<Vec<DownloadInstance>, AppError> {
        get_all_active_downloads(&self.pool)
            .await
            .map_err(AppError::database_error)
    }

    /// Find an existing in-progress download for the same model
    /// Returns the existing download if one exists with status Pending or Downloading
    pub async fn find_existing_in_progress(
        &self,
        repository_id: Uuid,
        provider_id: Uuid,
        repository_path: &str,
        main_filename: &str,
    ) -> Result<Option<DownloadInstance>, AppError> {
        find_existing_in_progress_download(
            &self.pool,
            repository_id,
            provider_id,
            repository_path,
            main_filename,
        )
        .await
        .map_err(AppError::database_error)
    }
}

pub async fn get_llm_model_by_id(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<Option<LlmModel>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, provider_id, name, display_name, description,
                enabled, is_deprecated, is_active,
                capabilities, parameters,
                created_at, updated_at,
                file_size_bytes, validation_status, validation_issues,
                port, pid, engine_type, engine_settings, file_format
         FROM llm_models
         WHERE id = $1"#,
        model_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| LlmModel {
        id: r.id,
        provider_id: r.provider_id,
        name: r.name,
        display_name: r.display_name,
        description: r.description,
        enabled: r.enabled,
        is_deprecated: r.is_deprecated,
        is_active: r.is_active,
        capabilities: r
            .capabilities
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        parameters: r
            .parameters
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
        file_size_bytes: r.file_size_bytes,
        validation_status: r.validation_status,
        validation_issues: r
            .validation_issues
            .and_then(|v| serde_json::from_value(v).ok()),
        port: r.port,
        pid: r.pid,
        engine_type: EngineType::from_str(&r.engine_type).unwrap(),
        engine_settings: r
            .engine_settings
            .and_then(|v| serde_json::from_value(v).ok()),
        file_format: FileFormat::from_str(&r.file_format).unwrap(),
    }))
}

/// List all LLM models across all providers
pub async fn list_all_llm_models(pool: &PgPool) -> Result<Vec<LlmModel>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, provider_id, name, display_name, description,
                enabled, is_deprecated, is_active,
                capabilities, parameters,
                created_at, updated_at,
                file_size_bytes, validation_status, validation_issues,
                port, pid, engine_type, engine_settings, file_format
         FROM llm_models
         ORDER BY created_at ASC"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmModel {
            id: r.id,
            provider_id: r.provider_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_deprecated: r.is_deprecated,
            is_active: r.is_active,
            capabilities: r
                .capabilities
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            parameters: r
                .parameters
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
            file_size_bytes: r.file_size_bytes,
            validation_status: r.validation_status,
            validation_issues: r
                .validation_issues
                .and_then(|v| serde_json::from_value(v).ok()),
            port: r.port,
            pid: r.pid,
            engine_type: EngineType::from_str(&r.engine_type).unwrap(),
            engine_settings: r
                .engine_settings
                .and_then(|v| serde_json::from_value(v).ok()),
            file_format: FileFormat::from_str(&r.file_format).unwrap(),
        })
        .collect())
}

/// List LLM models by provider
pub async fn list_llm_models_by_provider(
    pool: &PgPool,
    provider_id: Uuid,
) -> Result<Vec<LlmModel>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT id, provider_id, name, display_name, description,
                enabled, is_deprecated, is_active,
                capabilities, parameters,
                created_at, updated_at,
                file_size_bytes, validation_status, validation_issues,
                port, pid, engine_type, engine_settings, file_format
         FROM llm_models
         WHERE provider_id = $1
         ORDER BY created_at ASC"#,
        provider_id
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LlmModel {
            id: r.id,
            provider_id: r.provider_id,
            name: r.name,
            display_name: r.display_name,
            description: r.description,
            enabled: r.enabled,
            is_deprecated: r.is_deprecated,
            is_active: r.is_active,
            capabilities: r
                .capabilities
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            parameters: r
                .parameters
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
            created_at: DateTime::from_timestamp(r.created_at.unix_timestamp(), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.updated_at.unix_timestamp(), 0).unwrap(),
            file_size_bytes: r.file_size_bytes,
            validation_status: r.validation_status,
            validation_issues: r
                .validation_issues
                .and_then(|v| serde_json::from_value(v).ok()),
            port: r.port,
            pid: r.pid,
            engine_type: EngineType::from_str(&r.engine_type).unwrap(),
            engine_settings: r
                .engine_settings
                .and_then(|v| serde_json::from_value(v).ok()),
            file_format: FileFormat::from_str(&r.file_format).unwrap(),
        })
        .collect())
}

pub async fn create_llm_model(
    pool: &PgPool,
    request: CreateLlmModelRequest,
) -> Result<LlmModel, sqlx::Error> {
    let model_id = Uuid::new_v4();
    let capabilities_json = serde_json::to_value(&request.capabilities.unwrap_or_default())
        .unwrap_or(serde_json::json!({}));
    let parameters_json = serde_json::to_value(&request.parameters.unwrap_or_default())
        .unwrap_or(serde_json::json!({}));
    let engine_settings_json = request
        .engine_settings
        .as_ref()
        .map(|s| serde_json::to_value(s).unwrap());

    let row = sqlx::query!(
        r#"INSERT INTO llm_models (id, provider_id, name, display_name, description, enabled, capabilities, parameters, engine_type, engine_settings, file_format)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         RETURNING id, provider_id, name, display_name, description, enabled, is_deprecated, is_active,
                   capabilities, parameters,
                   created_at, updated_at, file_size_bytes, validation_status, validation_issues,
                   port, pid, engine_type, engine_settings, file_format"#,
        model_id,
        request.provider_id,
        &request.name,
        &request.display_name,
        request.description.as_deref(),
        request.enabled.unwrap_or(true),
        capabilities_json,
        parameters_json,
        request.engine_type.as_str(),
        engine_settings_json,
        request.file_format.as_str(),
    )
    .fetch_one(pool)
    .await?;

    Ok(LlmModel {
        id: row.id,
        provider_id: row.provider_id,
        name: row.name,
        display_name: row.display_name,
        description: row.description,
        enabled: row.enabled,
        is_deprecated: row.is_deprecated,
        is_active: row.is_active,
        capabilities: row
            .capabilities
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        parameters: row
            .parameters
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        created_at: DateTime::from_timestamp(row.created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(row.updated_at.unix_timestamp(), 0).unwrap(),
        file_size_bytes: row.file_size_bytes,
        validation_status: row.validation_status,
        validation_issues: row
            .validation_issues
            .and_then(|v| serde_json::from_value(v).ok()),
        port: row.port,
        pid: row.pid,
        engine_type: EngineType::from_str(&row.engine_type).unwrap(),
        engine_settings: row
            .engine_settings
            .and_then(|v| serde_json::from_value(v).ok()),
        file_format: FileFormat::from_str(&row.file_format).unwrap(),
    })
}

pub async fn update_llm_model(
    pool: &PgPool,
    model_id: Uuid,
    request: UpdateLlmModelRequest,
) -> Result<Option<LlmModel>, sqlx::Error> {
    // If no updates provided, return existing record
    if request.name.is_none()
        && request.display_name.is_none()
        && request.description.is_none()
        && request.enabled.is_none()
        && request.is_active.is_none()
        && request.capabilities.is_none()
        && request.parameters.is_none()
        && request.engine_type.is_none()
        && request.engine_settings.is_none()
        && request.file_format.is_none()
    {
        return get_llm_model_by_id(pool, model_id).await;
    }

    // Apply updates
    if let Some(name) = &request.name {
        sqlx::query!(
            "UPDATE llm_models SET name = $1, updated_at = NOW() WHERE id = $2",
            name,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(display_name) = &request.display_name {
        sqlx::query!(
            "UPDATE llm_models SET display_name = $1, updated_at = NOW() WHERE id = $2",
            display_name,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(description) = &request.description {
        sqlx::query!(
            "UPDATE llm_models SET description = $1, updated_at = NOW() WHERE id = $2",
            Some(description),
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(enabled) = request.enabled {
        sqlx::query!(
            "UPDATE llm_models SET enabled = $1, updated_at = NOW() WHERE id = $2",
            enabled,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(is_active) = request.is_active {
        sqlx::query!(
            "UPDATE llm_models SET is_active = $1, updated_at = NOW() WHERE id = $2",
            is_active,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(capabilities) = &request.capabilities {
        let capabilities_json = serde_json::to_value(capabilities).unwrap();
        sqlx::query!(
            "UPDATE llm_models SET capabilities = $1, updated_at = NOW() WHERE id = $2",
            capabilities_json,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(parameters) = &request.parameters {
        let parameters_json = serde_json::to_value(parameters).unwrap();
        sqlx::query!(
            "UPDATE llm_models SET parameters = $1, updated_at = NOW() WHERE id = $2",
            parameters_json,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(engine_type) = &request.engine_type {
        sqlx::query!(
            "UPDATE llm_models SET engine_type = $1, updated_at = NOW() WHERE id = $2",
            engine_type.as_str(),
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(engine_settings) = &request.engine_settings {
        let engine_settings_json = serde_json::to_value(engine_settings).unwrap();
        sqlx::query!(
            "UPDATE llm_models SET engine_settings = $1, updated_at = NOW() WHERE id = $2",
            engine_settings_json,
            model_id
        )
        .execute(pool)
        .await?;
    }

    if let Some(file_format) = &request.file_format {
        sqlx::query!(
            "UPDATE llm_models SET file_format = $1, updated_at = NOW() WHERE id = $2",
            file_format.as_str(),
            model_id
        )
        .execute(pool)
        .await?;
    }

    // Return updated model
    get_llm_model_by_id(pool, model_id).await
}

pub async fn delete_llm_model(pool: &PgPool, model_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM llm_models WHERE id = $1", model_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn set_model_validation_status(
    pool: &PgPool,
    model_id: Uuid,
    status: &str,
    issues: Option<Vec<String>>,
) -> Result<(), sqlx::Error> {
    let issues_json = issues.map(|i| serde_json::to_value(i).unwrap());

    sqlx::query!(
        "UPDATE llm_models SET validation_status = $1, validation_issues = $2, updated_at = NOW() WHERE id = $3",
        status,
        issues_json,
        model_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

// ==================== Download Instance Repository Functions ====================

/// Get a download instance by ID
pub async fn get_download_instance_by_id(
    pool: &PgPool,
    download_id: Uuid,
) -> Result<Option<DownloadInstance>, sqlx::Error> {
    sqlx::query_as!(
        DownloadInstance,
        r#"SELECT id, provider_id, repository_id,
                request_data,
                status,
                progress_data,
                error_message,
                started_at as "started_at: _",
                completed_at as "completed_at: _",
                model_id,
                created_at as "created_at: _",
                updated_at as "updated_at: _"
         FROM download_instances
         WHERE id = $1"#,
        download_id
    )
    .fetch_optional(pool)
    .await
}

/// Get all download instances with pagination and optional status filter
pub async fn get_download_instances(
    pool: &PgPool,
    page: i32,
    per_page: i32,
    status_filter: Option<DownloadStatus>,
) -> Result<DownloadInstanceListResponse, sqlx::Error> {
    let offset = (page - 1) * per_page;

    // Execute query based on whether we have a status filter
    let downloads = if let Some(ref status) = status_filter {
        sqlx::query_as!(
            DownloadInstance,
            r#"SELECT id, provider_id, repository_id,
                     request_data,
                     status,
                     progress_data,
                     error_message,
                     started_at as "started_at: _",
                     completed_at as "completed_at: _",
                     model_id,
                     created_at as "created_at: _",
                     updated_at as "updated_at: _"
             FROM download_instances
             WHERE status = $3
             ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
            per_page as i64,
            offset as i64,
            status.as_str()
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            DownloadInstance,
            r#"SELECT id, provider_id, repository_id,
                     request_data,
                     status,
                     progress_data,
                     error_message,
                     started_at as "started_at: _",
                     completed_at as "completed_at: _",
                     model_id,
                     created_at as "created_at: _",
                     updated_at as "updated_at: _"
             FROM download_instances
             ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
            per_page as i64,
            offset as i64
        )
        .fetch_all(pool)
        .await?
    };

    // Count total records
    let total: i64 = if let Some(ref status) = status_filter {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM download_instances WHERE status = $1",
            status.as_str()
        )
        .fetch_one(pool)
        .await?
        .unwrap_or(0)
    } else {
        sqlx::query_scalar!("SELECT COUNT(*) FROM download_instances")
            .fetch_one(pool)
            .await?
            .unwrap_or(0)
    };

    Ok(DownloadInstanceListResponse {
        downloads,
        total,
        page,
        per_page,
    })
}

/// Create a new download instance
pub async fn create_download_instance(
    pool: &PgPool,
    request: CreateDownloadInstanceRequest,
) -> Result<DownloadInstance, sqlx::Error> {
    let download_id = Uuid::new_v4();

    sqlx::query_as!(
        DownloadInstance,
        r#"INSERT INTO download_instances (id, provider_id, repository_id, request_data, status, progress_data)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, provider_id, repository_id,
                   request_data,
                   status,
                   progress_data,
                   error_message,
                   started_at as "started_at: _",
                   completed_at as "completed_at: _",
                   model_id,
                   created_at as "created_at: _",
                   updated_at as "updated_at: _"
         "#,
        download_id,
        request.provider_id,
        request.repository_id,
        serde_json::to_value(&request.request_data)
            .map_err(|e| sqlx::Error::Encode(Box::new(e)))?,
        DownloadStatus::Pending.as_str(),
        serde_json::to_value(&DownloadProgressData {
            phase: DownloadPhase::Created,
            current: 0,
            total: 0,
            message: "Download instance created".to_string(),
            speed_bps: 0,
            eta_seconds: 0,
        })
        .map_err(|e| sqlx::Error::Encode(Box::new(e)))?,
    )
    .fetch_one(pool)
    .await
}

/// Update download progress
pub async fn update_download_progress(
    pool: &PgPool,
    download_id: Uuid,
    request: UpdateDownloadProgressRequest,
) -> Result<Option<DownloadInstance>, sqlx::Error> {
    if let Some(status) = request.status {
        sqlx::query_as!(
            DownloadInstance,
            r#"UPDATE download_instances
             SET progress_data = $2,
                 status = $3,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = $1
             RETURNING id, provider_id, repository_id,
                       request_data,
                       status,
                       progress_data,
                       error_message,
                       started_at as "started_at: _",
                       completed_at as "completed_at: _",
                       model_id,
                       created_at as "created_at: _",
                       updated_at as "updated_at: _"
         "#,
            download_id,
            serde_json::to_value(&request.progress_data)
                .map_err(|e| sqlx::Error::Encode(Box::new(e)))?,
            status.as_str()
        )
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query_as!(
            DownloadInstance,
            r#"UPDATE download_instances
             SET progress_data = $2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = $1
             RETURNING id, provider_id, repository_id,
                       request_data,
                       status,
                       progress_data,
                       error_message,
                       started_at as "started_at: _",
                       completed_at as "completed_at: _",
                       model_id,
                       created_at as "created_at: _",
                       updated_at as "updated_at: _"
         "#,
            download_id,
            serde_json::to_value(&request.progress_data)
                .map_err(|e| sqlx::Error::Encode(Box::new(e)))?
        )
        .fetch_optional(pool)
        .await
    }
}

/// Update download status (for completion, failure, or cancellation)
pub async fn update_download_status(
    pool: &PgPool,
    download_id: Uuid,
    request: UpdateDownloadStatusRequest,
) -> Result<Option<DownloadInstance>, sqlx::Error> {
    // Build update query based on status
    match request.status {
        DownloadStatus::Completed => {
            sqlx::query_as!(
                DownloadInstance,
                r#"UPDATE download_instances
                 SET status = $2,
                     error_message = $3,
                     model_id = $4,
                     completed_at = CURRENT_TIMESTAMP,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = $1
                 RETURNING id, provider_id, repository_id,
                           request_data,
                           status,
                           progress_data,
                           error_message,
                           started_at as "started_at: _",
                           completed_at as "completed_at: _",
                           model_id,
                           created_at as "created_at: _",
                           updated_at as "updated_at: _"
         "#,
                download_id,
                request.status.as_str(),
                request.error_message,
                request.model_id
            )
            .fetch_optional(pool)
            .await
        }
        DownloadStatus::Failed | DownloadStatus::Cancelled => {
            sqlx::query_as!(
                DownloadInstance,
                r#"UPDATE download_instances
                 SET status = $2,
                     error_message = $3,
                     completed_at = CURRENT_TIMESTAMP,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = $1
                 RETURNING id, provider_id, repository_id,
                           request_data,
                           status,
                           progress_data,
                           error_message,
                           started_at as "started_at: _",
                           completed_at as "completed_at: _",
                           model_id,
                           created_at as "created_at: _",
                           updated_at as "updated_at: _"
         "#,
                download_id,
                request.status.as_str(),
                request.error_message
            )
            .fetch_optional(pool)
            .await
        }
        _ => {
            sqlx::query_as!(
                DownloadInstance,
                r#"UPDATE download_instances
                 SET status = $2,
                     error_message = $3,
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = $1
                 RETURNING id, provider_id, repository_id,
                           request_data,
                           status,
                           progress_data,
                           error_message,
                           started_at as "started_at: _",
                           completed_at as "completed_at: _",
                           model_id,
                           created_at as "created_at: _",
                           updated_at as "updated_at: _"
         "#,
                download_id,
                request.status.as_str(),
                request.error_message
            )
            .fetch_optional(pool)
            .await
        }
    }
}

/// Delete a download instance
pub async fn delete_download_instance(
    pool: &PgPool,
    download_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM download_instances WHERE id = $1", download_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Get all active downloads (pending, downloading, failed, cancelled, completed)
/// Includes completed downloads to ensure SSE sends final status update before closing stream
pub async fn get_all_active_downloads(pool: &PgPool) -> Result<Vec<DownloadInstance>, sqlx::Error> {
    sqlx::query_as!(
        DownloadInstance,
        r#"SELECT id, provider_id, repository_id,
                 request_data,
                 status,
                 progress_data,
                 error_message,
                 started_at as "started_at: _",
                 completed_at as "completed_at: _",
                 model_id,
                 created_at as "created_at: _",
                 updated_at as "updated_at: _"
         FROM download_instances
         WHERE status IN ('pending', 'downloading', 'failed', 'cancelled', 'completed')
         ORDER BY created_at ASC"#
    )
    .fetch_all(pool)
    .await
}

pub async fn find_existing_in_progress_download(
    pool: &PgPool,
    repository_id: Uuid,
    provider_id: Uuid,
    repository_path: &str,
    main_filename: &str,
) -> Result<Option<DownloadInstance>, sqlx::Error> {
    sqlx::query_as!(
        DownloadInstance,
        r#"SELECT id, provider_id, repository_id,
                 request_data,
                 status,
                 progress_data,
                 error_message,
                 started_at as "started_at: _",
                 completed_at as "completed_at: _",
                 model_id,
                 created_at as "created_at: _",
                 updated_at as "updated_at: _"
         FROM download_instances
         WHERE repository_id = $1
           AND provider_id = $2
           AND status IN ('pending', 'downloading')
           AND request_data->>'repository_path' = $3
           AND request_data->>'main_filename' = $4
         ORDER BY created_at DESC
         LIMIT 1"#,
        repository_id,
        provider_id,
        repository_path,
        main_filename
    )
    .fetch_optional(pool)
    .await
}

/// Delete all download instances
pub async fn delete_all_downloads(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query!("DELETE FROM download_instances")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

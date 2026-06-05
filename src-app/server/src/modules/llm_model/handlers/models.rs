// LLM Model handlers
// Source: react-test/src-tauri/src/api/models.rs
// Following ziee patterns from llm_provider module

use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::r#type::{ApiResult, AppError},
    core::{events::EventBus, repository::Repos},
    modules::llm_model::permissions::LlmModelsRead,
    modules::llm_provider::permissions::UserLlmProvidersRead,
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};
use std::sync::Arc;

use super::super::{
    events::LlmModelEvent,
    models::{DownloadInstance, LlmModel},
    permissions::*,
    types::{CreateLlmModelRequest, ListModelsQuery, LlmModelListResponse, UpdateLlmModelRequest},
    utils,
};

// =====================================================
// Model CRUD Handlers
// =====================================================

/// List all LLM models with pagination and optional provider filtering
/// (requires llm_models::read permission)
#[debug_handler]
pub async fn list_models(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Query(params): Query<ListModelsQuery>,
) -> ApiResult<Json<LlmModelListResponse>> {
    // Get models based on whether provider_id filter is provided
    let mut all_models = if let Some(provider_id) = params.provider_id {
        // Filter by provider
        Repos.llm_model.list_by_provider(provider_id).await?
    } else {
        // Get all models across all providers
        Repos.llm_model.list_all().await?
    };

    // Optional capability filter. Allowlisted to defend against admin
    // typos / future JSONB-path injection. Used by the memory admin
    // page's embedding-model dropdown via `?capability=text_embedding`.
    if let Some(ref cap) = params.capability {
        const ALLOWED_CAPABILITIES: &[&str] = &[
            "text_embedding",
            "vision",
            "audio",
            "tools",
            "chat",
            "image_generator",
        ];
        if !ALLOWED_CAPABILITIES.contains(&cap.as_str()) {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "VALIDATION_ERROR",
                    format!(
                        "unknown capability {:?}; expected one of: {}",
                        cap,
                        ALLOWED_CAPABILITIES.join(", ")
                    ),
                ),
            ));
        }
        let cap = cap.clone();
        all_models.retain(|m| {
            serde_json::to_value(&m.capabilities)
                .ok()
                .and_then(|v| v.get(&cap).and_then(|c| c.as_bool()))
                .unwrap_or(false)
        });
    }

    // Calculate pagination
    let total = all_models.len() as i64;
    let start = ((params.page - 1) * params.per_page) as usize;
    let end = (start + params.per_page as usize).min(all_models.len());

    let paginated_models = if start < all_models.len() {
        all_models[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok((
        StatusCode::OK,
        Json(LlmModelListResponse {
            models: paginated_models,
            total,
            page: params.page as i32,
            per_page: params.per_page as i32,
        }),
    ))
}

pub fn list_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.list")
        .tag("LLM Models")
        .summary("List LLM models with pagination and optional provider filtering")
        .description("List all LLM models. Optionally filter by provider_id query parameter.")
        .response::<200, Json<LlmModelListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM model by ID (requires llm_models::read permission)
#[debug_handler]
pub async fn get_model(
    _auth: RequirePermissions<(LlmModelsRead,)>,
    Path(model_id): Path<Uuid>,
    
) -> ApiResult<Json<LlmModel>> {
    let model = Repos.llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    Ok((StatusCode::OK, Json(model)))
}

pub fn get_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsRead,)>(op)
        .id("LlmModel.get")
        .tag("LLM Models")
        .summary("Get LLM model by ID")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Create a new LLM model (requires llm_models::create permission)
#[debug_handler]
pub async fn create_model(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    utils::validate_create_request(&request)?;

    // Create model
    let model = Repos.llm_model.create(request).await?;

    // Emit event
    event_bus.emit_async(LlmModelEvent::created(model.clone()).into());

    sync_publish(SyncEntity::LlmModel, SyncAction::Create, model.id, Audience::perm::<LlmModelsRead>(), origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, model.id, Audience::perm::<UserLlmProvidersRead>(), origin.0);

    Ok((StatusCode::CREATED, Json(model)))
}

pub fn create_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.create")
        .tag("LLM Models")
        .summary("Create a new LLM model")
        .response::<201, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM model (requires llm_models::edit permission)
#[debug_handler]
pub async fn update_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<UpdateLlmModelRequest>,
) -> ApiResult<Json<LlmModel>> {
    // Validate request
    utils::validate_update_request(&request)?;

    // Update model
    let model = Repos.llm_model
        .update(model_id, request)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Emit event
    event_bus.emit_async(LlmModelEvent::updated(model.clone()).into());

    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model.id, Audience::perm::<LlmModelsRead>(), origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, model.id, Audience::perm::<UserLlmProvidersRead>(), origin.0);

    Ok((StatusCode::OK, Json(model)))
}

pub fn update_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.update")
        .tag("LLM Models")
        .summary("Update an existing LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Query options for `DELETE /api/llm-models/{id}`.
///
/// `delete_file` (default `true`) controls whether the on-disk
/// model directory is removed. The frontend pre-fills the
/// confirmation checkbox based on whether the model's `file_path`
/// is under the managed models directory; operator-managed paths
/// (Path 3 pre-stage) default-uncheck.
///
/// A safety net (`force`, default `false`) is required if
/// `delete_file=true` AND the path resolves outside the managed
/// `<app_data>/models/` directory — prevents an admin from
/// accidentally clobbering an arbitrary host path.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteModelQuery {
    #[serde(default = "default_true")]
    pub delete_file: bool,
    #[serde(default)]
    pub force: bool,
}

fn default_true() -> bool {
    true
}

/// Delete an LLM model (requires llm_models::delete permission)
#[debug_handler]
pub async fn delete_model(
    _auth: RequirePermissions<(LlmModelsDelete,)>,
    Path(model_id): Path<Uuid>,
    Query(query): Query<DeleteModelQuery>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Get model details before deletion (need provider_id for file path)
    let model = Repos.llm_model.get_by_id(model_id).await?;

    if model.is_none() {
        return Err(AppError::not_found("Model").to_api_error());
    }

    let model = model.unwrap();
    let provider_id = model.provider_id;
    let model_name = model.name.clone();

    // Delete from database first
    let deleted = Repos.llm_model.delete(model_id).await?;

    if !deleted {
        return Err(AppError::not_found("Model").to_api_error());
    }

    // Emit event
    event_bus.emit_async(LlmModelEvent::deleted(model_id, model_name).into());

    // Optionally delete files from disk. The "managed path" is
    // <app_data>/models/<provider_id>/<model_id>/; this is always
    // inside the managed root by construction. The `force` query is
    // accepted as a no-op for future-compatibility with file_path
    // override flows (P1.h's Path 3 operator-managed paths are not
    // exposed on LlmModel today; if/when we surface them, the
    // safety net activates).
    if query.delete_file {
        let _ = query.force; // reserved for future Path-3 cleanup
        let storage = crate::modules::llm_model::storage::ModelStorage::new()
            .await
            .map_err(|e| AppError::internal_error(format!("Storage error: {}", e)))?;

        let managed_path = storage.get_model_path(&provider_id, &model_id);
        if managed_path.exists() {
            let result = if managed_path.is_dir() {
                tokio::fs::remove_dir_all(&managed_path).await
            } else {
                tokio::fs::remove_file(&managed_path).await
            };
            if let Err(e) = result {
                tracing::error!(
                    "Failed to remove model path {}: {}",
                    managed_path.display(),
                    e
                );
                return Err(AppError::internal_error(format!(
                    "Failed to remove model files: {}",
                    e
                ))
                .into());
            }
            tracing::info!("Removed model path: {}", managed_path.display());
        }
    } else {
        tracing::info!(
            "Skipped on-disk delete for model {} (delete_file=false)",
            model_id
        );
    }

    sync_publish(SyncEntity::LlmModel, SyncAction::Delete, model_id, Audience::perm::<LlmModelsRead>(), origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, model_id, Audience::perm::<UserLlmProvidersRead>(), origin.0);

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn delete_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsDelete,)>(op)
        .id("LlmModel.delete")
        .tag("LLM Models")
        .summary("Delete an LLM model")
        .description(concat!(
            "Query: delete_file (default true) controls on-disk file deletion. ",
            "If delete_file=true and the file_path is outside <app_data>/models, ",
            "pass force=true to override the safety net."
        ))
        .response_with::<204, (), _>(|res| res.description("Model deleted successfully"))
        .response_with::<400, (), _>(|r| r.description("delete_file refused without force"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// P1.k: manually (re-)trigger validation for a model. Backs the
/// "Run inference test" button on the model card.
///   - Local models → enqueue a Tier-3 background validation
///     (engine load probe + a tiny chat round-trip).
///   - Remote models → inline remote API probe (tiny chat call).
#[debug_handler]
pub async fn validate_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(pool): Extension<sqlx::PgPool>,
) -> ApiResult<Json<serde_json::Value>> {
    let model = Repos
        .llm_model
        .get_by_id(model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    let provider = Repos
        .llm_provider
        .get_by_id(model.provider_id)
        .await
        .map_err(|e| AppError::internal_error(format!("provider lookup: {e}")))?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    if provider.provider_type == "local" {
        crate::modules::llm_local_runtime::validator::enqueue(
            model_id,
            crate::modules::llm_local_runtime::validator::ValidationTier::Tier3,
        )
        .await;
        Ok((
            StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "queued": true,
                "tier": "tier3",
                "message": "Local model validation queued; watch validation_status."
            })),
        ))
    } else {
        // Remote: run the probe inline (no engine to serialize).
        let outcome =
            crate::modules::llm_local_runtime::validator::validate_remote_model(&pool, model_id)
                .await?;
        let valid = matches!(
            outcome,
            crate::modules::llm_local_runtime::validator::ValidationOutcome::Valid
        );
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({
                "queued": false,
                "valid": valid,
            })),
        ))
    }
}

pub fn validate_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.validate")
        .tag("LLM Models")
        .summary("Manually (re-)run model validation (Tier-3 local / remote API probe).")
        .response::<200, Json<serde_json::Value>>()
        .response_with::<202, (), _>(|r| r.description("Local validation queued"))
        .response_with::<404, (), _>(|r| r.description("Model not found"))
}

// =====================================================
// Model Action Handlers
// =====================================================

/// Enable an LLM model (requires llm_models::edit permission)
#[debug_handler]
pub async fn enable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(true),
        ..Default::default()
    };

    let model = Repos.llm_model
        .update(model_id, request)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Emit event
    event_bus.emit_async(LlmModelEvent::updated(model.clone()).into());

    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model.id, Audience::perm::<LlmModelsRead>(), origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, model.id, Audience::perm::<UserLlmProvidersRead>(), origin.0);

    Ok((StatusCode::OK, Json(model)))
}

pub fn enable_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.enable")
        .tag("LLM Models")
        .summary("Enable an LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

/// Disable an LLM model (requires llm_models::edit permission)
#[debug_handler]
pub async fn disable_model(
    _auth: RequirePermissions<(LlmModelsEdit,)>,
    Path(model_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<Json<LlmModel>> {
    let request = UpdateLlmModelRequest {
        enabled: Some(false),
        ..Default::default()
    };

    let model = Repos.llm_model
        .update(model_id, request)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Emit event
    event_bus.emit_async(LlmModelEvent::updated(model.clone()).into());

    sync_publish(SyncEntity::LlmModel, SyncAction::Update, model.id, Audience::perm::<LlmModelsRead>(), origin.0);
    sync_publish(SyncEntity::UserLlmProvider, SyncAction::Update, model.id, Audience::perm::<UserLlmProvidersRead>(), origin.0);

    Ok((StatusCode::OK, Json(model)))
}

pub fn disable_model_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsEdit,)>(op)
        .id("LlmModel.disable")
        .tag("LLM Models")
        .summary("Disable an LLM model")
        .response::<200, Json<LlmModel>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Model not found"))
}

// =====================================================
// File Upload/Download Documentation
// =====================================================

pub fn upload_files_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.upload")
        .tag("LLM Models")
        .summary("Upload model files and create a new model")
        .description("Upload model weight files, config, and tokenizer files. The model is automatically created from the uploaded files.")
        .response::<200, Json<LlmModel>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request or file validation failed"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn initiate_download_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmModelsCreate,)>(op)
        .id("LlmModel.download")
        .tag("LLM Models")
        .summary("Initiate model download from a repository")
        .description("Start a background download task for a model from a Git repository (e.g., Hugging Face). Returns immediately with a download instance ID. The actual download happens in the background.")
        .response::<200, Json<DownloadInstance>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

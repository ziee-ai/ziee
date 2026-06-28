// LLM Model file upload and download handlers
// Adapted from react-test/src-tauri/src/api/model_uploads.rs for ziee

use crate::core::Repos;
use axum::{debug_handler, extract::Multipart, http::StatusCode, response::Json};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::common::r#type::{ApiResult, AppError};
use crate::modules::llm_model::permissions::LlmModelsRead;
use crate::modules::llm_provider::permissions::UserLlmProvidersRead;
use crate::modules::permissions::RequirePermissions;
use crate::modules::sync::{Audience, SyncAction, SyncEntity, publish as sync_publish};
use crate::utils::git::{GitError, GitPhase, GitProgress, GitService};

use super::super::{
    models::{
        DownloadInstance, DownloadPhase, DownloadProgressData, DownloadRequestData, DownloadStatus,
        EngineType, FileFormat, LlmModel, ModelCapabilities, ModelEngineSettings, ModelParameters,
    },
    permissions::*,
    repository,
    storage::ModelStorage,
    types::{self, CreateDownloadInstanceRequest, CreateLlmModelRequest},
};

/// Convert GitPhase to DownloadPhase
fn git_phase_to_download_phase(git_phase: GitPhase) -> DownloadPhase {
    match git_phase {
        GitPhase::Connecting => DownloadPhase::Connecting,
        GitPhase::Receiving => DownloadPhase::Receiving,
        GitPhase::Resolving => DownloadPhase::Resolving,
        GitPhase::CheckingOut => DownloadPhase::CheckingOut,
        GitPhase::Complete => DownloadPhase::Complete,
        GitPhase::Error => DownloadPhase::Error,
    }
}

/// Progress tracker for calculating speed and ETA
#[derive(Debug, Clone)]
struct ProgressTracker {
    start_time: std::time::Instant,
    last_update_time: std::time::Instant,
    last_bytes: u64,
}

impl ProgressTracker {
    fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            start_time: now,
            last_update_time: now,
            last_bytes: 0,
        }
    }

    fn update(&mut self, current_bytes: u64) -> (Option<f64>, Option<u64>) {
        let now = std::time::Instant::now();

        // Calculate overall speed (bytes per second)
        let total_elapsed = now.duration_since(self.start_time).as_secs_f64();
        let overall_speed = if total_elapsed > 0.0 {
            current_bytes as f64 / total_elapsed
        } else {
            0.0
        };

        // Calculate recent speed for more responsive updates
        let recent_elapsed = now.duration_since(self.last_update_time).as_secs_f64();
        let recent_speed = if recent_elapsed > 1.0 {
            let bytes_diff = current_bytes.saturating_sub(self.last_bytes) as f64;
            bytes_diff / recent_elapsed
        } else {
            overall_speed
        };

        // Use recent speed if it's reasonable, otherwise use overall speed
        let speed_bps = if recent_speed > 0.0 && recent_elapsed > 1.0 {
            recent_speed
        } else {
            overall_speed
        };

        // Update tracking state
        if recent_elapsed > 1.0 {
            self.last_update_time = now;
            self.last_bytes = current_bytes;
        }

        (Some(speed_bps), None)
    }

    fn calculate_eta(
        &self,
        current_bytes: u64,
        total_bytes: u64,
        speed_bps: Option<f64>,
    ) -> Option<u64> {
        if let Some(speed) = speed_bps {
            if speed > 0.0 && total_bytes > current_bytes {
                let remaining_bytes = total_bytes - current_bytes;
                let eta_seconds = remaining_bytes as f64 / speed;
                Some(eta_seconds as u64)
            } else {
                None
            }
        } else {
            None
        }
    }
}

/// Request struct for creating a model with files
#[derive(Debug)]
pub struct CreateModelWithFilesRequest {
    pub provider_id: Uuid,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub file_format: FileFormat,
    pub main_filename: String,
    pub source_dir: PathBuf,
    pub capabilities: Option<ModelCapabilities>,
    pub parameters: Option<ModelParameters>,
    pub engine_type: Option<EngineType>,
    pub engine_settings: Option<ModelEngineSettings>,
}

/// Shared model creation and file processing logic
async fn create_model_with_files(
    repo: &repository::LlmModelRepository,
    request: CreateModelWithFilesRequest,
) -> Result<LlmModel, AppError> {
    // Initialize storage
    let storage = ModelStorage::new()
        .await
        .map_err(|e| AppError::internal_error(format!("Failed to initialize storage: {}", e)))?;

    // Validate provider exists and is of type 'local'
    let provider =
        Repos.llm_provider.get_by_id(request.provider_id)
            .await
            .map_err(|e| AppError::internal_error(e.to_string()))?
            .ok_or_else(|| AppError::bad_request("NOT_FOUND", "Provider not found"))?;

    if provider.provider_type.as_str() != "local" {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Only Local providers support model uploads",
        ));
    }

    // Generate model ID first (but don't create in database yet)
    let model_id = Uuid::new_v4();
    let model_name = request.name.clone();

    tracing::info!(
        "Processing model with file format: {}",
        request.file_format.as_str()
    );

    // Create storage directory
    storage
        .create_model_directory(&request.provider_id, &model_id)
        .await
        .map_err(|e| {
            AppError::internal_error(format!("Failed to create storage directory: {}", e))
        })?;

    tracing::debug!(
        "Source directory for model files: {}",
        request.source_dir.display()
    );

    // List all files in the source directory
    let source_files = match tokio::fs::read_dir(&request.source_dir).await {
        Ok(mut entries) => {
            let mut files = Vec::new();
            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                AppError::internal_error(format!("Failed to read directory entry: {}", e))
            })? {
                if entry
                    .file_type()
                    .await
                    .map_err(|e| {
                        AppError::internal_error(format!("Failed to get file type: {}", e))
                    })?
                    .is_file()
                {
                    files.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            files
        }
        Err(e) => {
            return Err(AppError::internal_error(format!(
                "Failed to read source directory: {}",
                e
            )));
        }
    };

    if source_files.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "No files found in source directory",
        ));
    }

    // Determine which files to copy based on main filename and index files.
    // (`determine_files_to_copy` errors when no weight matches, so the result
    // is never an empty Ok — no separate empty-check needed.)
    let files_to_copy = determine_files_to_copy(&source_files, &request.main_filename)?;

    tracing::info!(
        "Found {} files to copy: {:?}",
        files_to_copy.len(),
        files_to_copy
    );

    // Copy the necessary files to the model directory and collect file info
    let mut total_size = 0u64;
    let file_count = files_to_copy.len();
    let mut file_records = Vec::new();

    for filename in &files_to_copy {
        let source_path = request.source_dir.join(filename);
        let dest_path = storage
            .get_model_path(&request.provider_id, &model_id)
            .join(filename);

        // Get file size
        let metadata = tokio::fs::metadata(&source_path).await.map_err(|e| {
            AppError::internal_error(format!(
                "Failed to get file metadata for {}: {}",
                filename, e
            ))
        })?;
        let file_size = metadata.len();
        total_size += file_size;

        // Copy the file
        tokio::fs::copy(&source_path, &dest_path)
            .await
            .map_err(|e| {
                AppError::internal_error(format!("Failed to copy file {}: {}", filename, e))
            })?;

        // Collect file information for database insertion later
        let file_type = determine_model_file_type(filename).to_string();
        let relative_path = format!("models/{}/{}/{}", request.provider_id, model_id, filename);

        file_records.push((
            filename.clone(),
            relative_path.clone(),
            file_size,
            file_type.clone(),
        ));

        tracing::debug!(
            "Copied file: {} -> {} ({} bytes)",
            filename,
            relative_path,
            file_size
        );
    }

    // Now that all files are processed successfully, create the model in the database
    let create_request = CreateLlmModelRequest {
        provider_id: request.provider_id,
        name: request.name,
        display_name: request.display_name,
        description: request.description,
        enabled: Some(true),
        capabilities: request.capabilities,
        parameters: request.parameters,
        engine_type: request.engine_type.unwrap_or(EngineType::Mistralrs),
        engine_settings: request.engine_settings,
        file_format: request.file_format,
    };

    // Preflight: explicit duplicate-name check.
    //
    // The previous implementation relied on string-matching the
    // sqlx::Error text for "llm_models_provider_id_name_unique" /
    // "duplicate key", but the A1 redaction (commit 94f5295) collapses
    // every sqlx error into a generic "An internal error occurred"
    // before it reaches this map_err — so the constraint-name detection
    // never fires and the user sees a confusing 500. Closes the
    // regression by checking before insert. The race window between
    // SELECT and INSERT is intentionally narrow and acceptable: in the
    // race, the second insert hits the unique index → 500, which is
    // the same outcome as today.
    let preflight_exists: bool = sqlx::query_scalar!(
        r#"SELECT EXISTS(
              SELECT 1 FROM llm_models
              WHERE provider_id = $1 AND name = $2
           ) AS "exists!""#,
        create_request.provider_id,
        create_request.name,
    )
    .fetch_one(crate::core::Repos.pool())
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Preflight duplicate-name query failed");
        AppError::internal_error("Storage error")
    })?;
    if preflight_exists {
        return Err(AppError::bad_request(
            "DUPLICATE_ENTRY",
            format!(
                "A model with the name '{}' already exists for this provider. \
                 Please use a different model name.",
                model_name
            ),
        ));
    }

    // Create the model record - it will generate its own ID
    let model_db = repo.create(create_request).await?;

    // The database record has been created with its own ID
    // But files are in directory with the pre-generated model_id
    // We need to rename the directory to match the database ID
    let old_dir = storage.get_model_path(&request.provider_id, &model_id);
    let new_dir = storage.get_model_path(&request.provider_id, &model_db.id);

    if old_dir != new_dir {
        tokio::fs::rename(&old_dir, &new_dir).await.map_err(|e| {
            AppError::internal_error(format!("Failed to rename model directory: {}", e))
        })?;
        tracing::debug!(
            "Renamed model directory from {} to {}",
            old_dir.display(),
            new_dir.display()
        );
    }

    // Update model with total size and validation status using the correct model ID
    repo.set_validation_status(model_db.id, "completed", None)
        .await?;

    // P1.k: enqueue a background Tier-2 validation (engine load
    // probe) for local models. This also extracts + persists
    // capabilities (P1.i). Non-blocking — the upload returns
    // immediately; the model card shows "Validating…" until the
    // worker transitions it to valid / validation_warning.
    if matches!(model_db.engine_type, EngineType::Llamacpp | EngineType::Mistralrs) {
        crate::modules::llm_local_runtime::validator::enqueue(
            model_db.id,
            crate::modules::llm_local_runtime::validator::ValidationTier::Tier2,
        )
        .await;
    }

    // Return the created model directly
    let model = model_db;

    tracing::info!(
        "Model created successfully: {} files, {} total size",
        file_count,
        total_size
    );

    // Realtime sync: a model was created (upload-commit or background
    // repository download). Notify admins (LlmModel) + every user's
    // accessible-providers view (UserLlmProvider). This is a shared helper
    // with no request context, so origin is None (the upload path's
    // originating tab already has the model from its response).
    sync_publish(
        SyncEntity::LlmModel,
        SyncAction::Create,
        model.id,
        Audience::perm::<LlmModelsRead>(),
        None,
    );
    sync_publish(
        SyncEntity::UserLlmProvider,
        SyncAction::Update,
        model.id,
        Audience::perm::<UserLlmProvidersRead>(),
        None,
    );

    Ok(model)
}

/// Determine which files to copy for a model download/upload.
///
/// Delegates to the shared mistral.rs-parity detector
/// ([`crate::modules::llm_model::model_files::select_download_files`]): for a
/// safetensors/pickle repo it keeps the WHOLE weight set (so a sharded repo
/// without a `*.index.json` still pulls every shard), for GGUF it keeps the
/// chosen quant (+ its shard siblings), and it always includes the
/// config/tokenizer/index aux files the engine loads alongside the weights.
fn determine_files_to_copy(
    source_files: &[String],
    main_filename: &str,
) -> Result<Vec<String>, AppError> {
    let files =
        crate::modules::llm_model::model_files::select_download_files(source_files, main_filename)
            .map_err(|m| AppError::bad_request("VALIDATION_ERROR", m))?;
    tracing::debug!("Files to copy: {:?}", files);
    Ok(files)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DownloadFromRepositoryRequest {
    pub provider_id: Uuid,
    pub repository_id: Uuid,
    pub repository_path: String,
    pub repository_branch: Option<String>,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub file_format: FileFormat,
    pub main_filename: String,
    pub capabilities: Option<ModelCapabilities>,
    pub parameters: Option<ModelParameters>,
    pub engine_type: Option<EngineType>,
    pub engine_settings: Option<ModelEngineSettings>,
}

/// Upload multiple model files and auto-commit as a model
#[debug_handler]
pub async fn upload_multiple_files_and_commit(
    _auth: RequirePermissions<(LlmModelsCreate,)>,

    mut multipart: Multipart,
) -> ApiResult<Json<LlmModel>> {
    let storage = ModelStorage::new().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Storage initialization failed: {}", e)),
        )
    })?;

    tracing::info!("Starting upload_local_model handler");

    let mut uploaded_files = Vec::new();
    let mut main_filename: Option<String> = None;
    let mut provider_id: Option<Uuid> = None;
    let mut name: Option<String> = None;
    let mut display_name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_format: Option<String> = None;
    let mut capabilities: Option<ModelCapabilities> = None;
    let mut engine_type: Option<EngineType> = None;
    let mut engine_settings: Option<ModelEngineSettings> = None;

    // Process multipart form data
    tracing::info!("Starting to parse multipart fields");
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "INVALID_INPUT",
                format!("Failed to read multipart field: {}", e),
            ),
        )
    })? {
        let field_name = field.name().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_INPUT",
                    "Multipart field missing name attribute",
                ),
            )
        })?
        .to_string();

        match field_name.as_str() {
            "files" => {
                if let Some(file_name) = field.file_name() {
                    let filename = std::path::Path::new(file_name)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(file_name)
                        .to_string();

                    let data = field.bytes().await.map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request(
                                "INVALID_INPUT",
                                format!("Failed to read file data: {}", e),
                            ),
                        )
                    })?;

                    // Per-upload cumulative size cap. Closes
                    // 07-llm-model F-03 (High): without this, an
                    // admin upload can stream multi-GB combined
                    // payloads (the route currently raises the
                    // global 16 MiB body limit). 20 GiB matches
                    // Llama-70B-class weights with comfortable
                    // headroom.
                    const MAX_MODEL_UPLOAD_BYTES: usize = 20 * 1024 * 1024 * 1024;
                    let already: usize = uploaded_files
                        .iter()
                        .map(|(_, d): &(String, Vec<u8>)| d.len())
                        .sum();
                    if already.saturating_add(data.len()) > MAX_MODEL_UPLOAD_BYTES {
                        return Err((
                            StatusCode::PAYLOAD_TOO_LARGE,
                            AppError::bad_request(
                                "MODEL_UPLOAD_TOO_LARGE",
                                format!(
                                    "Combined model upload exceeds {} GiB cap",
                                    MAX_MODEL_UPLOAD_BYTES / (1024 * 1024 * 1024)
                                ),
                            ),
                        ));
                    }

                    uploaded_files.push((filename, data.to_vec()));
                }
            }
            "main_filename" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read main_filename: {}", e),
                        ),
                    )
                })?;
                main_filename = Some(value);
            }
            "provider_id" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read provider_id: {}", e),
                        ),
                    )
                })?;
                provider_id = Some(Uuid::parse_str(&value).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Invalid provider_id format: {}", e),
                        ),
                    )
                })?);
            }
            "name" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read name: {}", e),
                        ),
                    )
                })?;
                name = Some(value);
            }
            "display_name" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read display_name: {}", e),
                        ),
                    )
                })?;
                display_name = Some(value);
            }
            "description" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read description: {}", e),
                        ),
                    )
                })?;
                description = if value.is_empty() { None } else { Some(value) };
            }
            "file_format" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read file_format: {}", e),
                        ),
                    )
                })?;
                file_format = Some(value);
            }
            "capabilities" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read capabilities: {}", e),
                        ),
                    )
                })?;
                if !value.is_empty() {
                    capabilities = serde_json::from_str(&value).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request(
                                "INVALID_INPUT",
                                format!("Invalid capabilities JSON: {}", e),
                            ),
                        )
                    })?;
                }
            }
            "engine_type" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read engine_type: {}", e),
                        ),
                    )
                })?;
                if !value.is_empty() {
                    engine_type = Some(EngineType::from_str(&value).ok_or_else(|| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request(
                                "INVALID_INPUT",
                                format!("Invalid engine_type: {}", value),
                            ),
                        )
                    })?);
                }
            }
            "engine_settings" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request(
                            "INVALID_INPUT",
                            format!("Failed to read engine_settings: {}", e),
                        ),
                    )
                })?;
                if !value.is_empty() {
                    engine_settings = Some(serde_json::from_str(&value).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request(
                                "INVALID_INPUT",
                                format!("Invalid engine_settings JSON: {}", e),
                            ),
                        )
                    })?)
                }
            }
            _ => {
                // Skip unknown fields
                continue;
            }
        }
    }

    tracing::info!(
        "Finished parsing multipart fields. Files: {}, provider_id: {:?}, name: {:?}",
        uploaded_files.len(),
        provider_id,
        name
    );

    // Validate required fields
    tracing::info!("Starting field validation");
    if uploaded_files.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            AppError::bad_request("VALIDATION_ERROR", "No files provided in multipart request"),
        ));
    }

    let provider_id = provider_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("MISSING_FIELD", "Missing provider_id in multipart request"),
        )
    })?;

    let main_filename = main_filename.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request(
                "MISSING_FIELD",
                "Missing main_filename in multipart request",
            ),
        )
    })?;

    let name = name.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("MISSING_FIELD", "Missing name in multipart request"),
        )
    })?;

    let display_name = display_name.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("MISSING_FIELD", "Missing display_name in multipart request"),
        )
    })?;

    let file_format = file_format.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("MISSING_FIELD", "Missing file_format in multipart request"),
        )
    })?;

    tracing::info!(
        "Processing multipart upload: {} files, main file: {}, name: {}, display_name: {}",
        uploaded_files.len(),
        main_filename,
        name,
        display_name
    );

    // Step 1: Upload files to temporary storage
    let temp_session_id = Uuid::new_v4();
    let mut total_size = 0u64;

    for (filename, file_data) in uploaded_files {
        total_size += file_data.len() as u64;

        // Validate file content; refuse the upload if any check fails.
        // The previous implementation collected the issues into
        // _validation_issues and threw them away — the model would be
        // accepted regardless of whether it actually looked like a valid
        // GGUF / safetensors / pytorch file. Closes 07-llm-model F-09
        // (Medium).
        let _file_type = determine_model_file_type(&filename);
        let validation_issues = validate_file_content(&filename, &file_data);
        if !validation_issues.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                AppError::bad_request(
                    "INVALID_MODEL_FILE",
                    format!(
                        "File '{}' failed validation: {}",
                        filename,
                        validation_issues.join("; ")
                    ),
                ),
            ));
        }

        // Save files to temporary storage
        let temp_file_id = Uuid::new_v4();
        storage
            .save_temp_file(&temp_session_id, &temp_file_id, &filename, &file_data)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error(format!("Failed to save file {}: {}", filename, e)),
                )
            })?;
    }

    tracing::info!(
        "Files uploaded successfully, total size: {} bytes",
        total_size
    );

    // Step 2: Auto-commit the uploaded files as a model
    let source_dir = crate::core::get_app_data_dir()
        .join("temp")
        .join(temp_session_id.to_string());

    tracing::info!(
        "Creating model with source_dir: {:?}, engine_type: {:?}, engine_settings: {:?}",
        source_dir,
        engine_type,
        engine_settings
    );

    // Create model using the existing function
    let model = create_model_with_files(
        &Repos.llm_model,
        CreateModelWithFilesRequest {
            provider_id,
            name,
            display_name,
            description,
            file_format: FileFormat::from_str(&file_format).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    AppError::bad_request(
                        "INVALID_INPUT",
                        format!("Invalid file format: {}", file_format),
                    ),
                )
            })?,
            main_filename,
            source_dir,
            capabilities,
            parameters: None,
            engine_type,
            engine_settings,
        },
    )
    .await
    .map_err(|e| e.to_api_error())?;

    tracing::info!(
        "Model created successfully: {} ({})",
        model.display_name,
        model.id
    );

    Ok((StatusCode::OK, Json(model)))
}

/// Determine model file type based on filename
fn determine_model_file_type(filename: &str) -> ModelFileType {
    let filename_lower = filename.to_lowercase();

    if filename_lower.ends_with(".bin")
        || filename_lower.ends_with(".pt")
        || filename_lower.ends_with(".pth")
        || filename_lower.ends_with(".safetensors")
        || filename_lower.ends_with(".gguf")
        || filename_lower.ends_with(".ggml")
    {
        return ModelFileType::WeightFile;
    }

    if filename_lower.contains("index") && filename_lower.ends_with(".json")
        || filename_lower == "pytorch_model.bin.index.json"
        || filename_lower == "model.safetensors.index.json"
        || filename_lower.ends_with(".index.json")
    {
        return ModelFileType::IndexFile;
    }

    if filename_lower == "config.json"
        || filename_lower.starts_with("config_")
        || filename_lower == "generation_config.json"
    {
        return ModelFileType::ConfigFile;
    }

    if filename_lower == "tokenizer.json"
        || filename_lower == "tokenizer_config.json"
        || filename_lower.starts_with("tokenizer_")
    {
        return ModelFileType::TokenizerFile;
    }

    if filename_lower == "vocab.json"
        || filename_lower == "merges.txt"
        || filename_lower == "special_tokens_map.json"
        || filename_lower == "vocab.txt"
        || filename_lower == "spiece.model"
    {
        return ModelFileType::VocabFile;
    }

    ModelFileType::UnknownFile
}

/// Validate file content and return any issues
fn validate_file_content(filename: &str, file_data: &[u8]) -> Vec<String> {
    let mut issues = Vec::new();

    if file_data.is_empty() {
        issues.push("File is empty".to_string());
        return issues;
    }

    let filename_lower = filename.to_lowercase();
    let file_type = determine_model_file_type(&filename_lower);

    match file_type {
        ModelFileType::WeightFile => {
            if file_data.len() < 1024 {
                issues.push("Model weight file is suspiciously small (< 1KB)".to_string());
            }
        }
        ModelFileType::ConfigFile => {
            if serde_json::from_slice::<serde_json::Value>(file_data).is_err() {
                issues.push("Config file is not valid JSON".to_string());
            }
        }
        ModelFileType::TokenizerFile => {
            if filename_lower == "tokenizer.json"
                && serde_json::from_slice::<serde_json::Value>(file_data).is_err() {
                    issues.push("Tokenizer file is not valid JSON".to_string());
                }
        }
        _ => {}
    }

    // Check for HTML content (error pages)
    if file_data.len() >= 4 {
        let first_4_bytes = &file_data[0..4];
        if matches!(
            first_4_bytes,
            [0x3C, 0x21, _, _] | [0x3C, 0x68, 0x74, 0x6D] | [0x3C, 0x48, 0x54, 0x4D]
        ) {
            issues.push("File appears to be HTML content (possibly an error page)".to_string());
        }
    }

    issues
}

/// File type classification for validation
#[derive(Debug, PartialEq)]
enum ModelFileType {
    WeightFile,
    IndexFile,
    ConfigFile,
    TokenizerFile,
    VocabFile,
    UnknownFile,
}

impl std::fmt::Display for ModelFileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFileType::WeightFile => write!(f, "weight"),
            ModelFileType::IndexFile => write!(f, "index"),
            ModelFileType::ConfigFile => write!(f, "config"),
            ModelFileType::TokenizerFile => write!(f, "tokenizer"),
            ModelFileType::VocabFile => write!(f, "vocab"),
            ModelFileType::UnknownFile => write!(f, "unknown"),
        }
    }
}

/// Internal function to initiate repository download without auth check
/// Used by both the public API endpoint and the hub module
/// Returns download instance immediately; actual download happens in background task
///
/// NOTE: this lower-level path is intentionally NOT subject to the hub
/// pre-download auth gate (`HUB_REPOSITORY_AUTH_NOT_CONFIGURED`, in
/// `hub/handlers.rs`). `auth_required` is a hub-catalog property; this API
/// accepts an arbitrary repository_id/path and downloads with whatever
/// credential the repo has (or none) — gating it unconditionally on
/// `has_credential()` would wrongly block legitimate public-repo downloads. If a
/// repo lacks a needed credential the background git clone surfaces the auth
/// error. The early-fail gate is hub-flow-only by design.
pub async fn initiate_repository_download_internal(
    request: DownloadFromRepositoryRequest,
) -> Result<DownloadInstance, String> {
    // Check if an identical download is already in progress
    if let Some(existing_download) = Repos
        .download_instance
        .find_existing_in_progress(
            request.repository_id,
            request.provider_id,
            &request.repository_path,
            &request.main_filename,
        )
        .await
        .map_err(|e| format!("Database error: {}", e))?
    {
        // Return the existing download instance instead of creating a new one
        return Ok(existing_download);
    }

    // Get repository information
    let repository = Repos.llm_repository.get_by_id(request.repository_id)
    .await
    .map_err(|e| format!("Database error: {}", e))?
    .ok_or_else(|| format!("Repository with ID {} not found", request.repository_id))?;

    // Create download instance in the database
    let download_request = CreateDownloadInstanceRequest {
        provider_id: request.provider_id,
        repository_id: request.repository_id,
        request_data: DownloadRequestData {
            model_name: request.name.clone(),
            revision: request.repository_branch.clone(),
            files: None,
            quantization: None,
            repository_path: Some(request.repository_path.clone()),
            display_name: Some(request.display_name.clone()),
            description: request.description.clone(),
            file_format: Some(request.file_format.as_str().to_string()),
            main_filename: Some(request.main_filename.clone()),
            capabilities: request.capabilities.clone(),
            parameters: request.parameters.clone(),
            engine_type: request.engine_type,
            engine_settings: request.engine_settings.clone(),
        },
    };

    let download_instance = match Repos.download_instance.create(download_request).await {
        Ok(d) => d,
        Err(e) => {
            // Lost the create race: a concurrent identical download won and the
            // partial unique index (uq_download_instances_in_progress) rejected
            // this duplicate. Return the in-flight winner rather than erroring.
            match Repos
                .download_instance
                .find_existing_in_progress(
                    request.repository_id,
                    request.provider_id,
                    &request.repository_path,
                    &request.main_filename,
                )
                .await
            {
                Ok(Some(existing)) => existing,
                _ => return Err(format!("Database error: {}", e)),
            }
        }
    };

    // Clone necessary data for the background task
    let download_id = download_instance.id;
    let _repository_id = repository.id;
    let repository_url =
        GitService::build_repository_url(&repository.url, &request.repository_path);
    let _repository_branch = request.repository_branch.clone();

    // Extract the credential for the git layer. For basic_auth the username and
    // password are kept SEPARATE — the password is the git secret and the
    // username is threaded through so the credential callback can pair them
    // correctly (libgit2 Basic auth = base64("username:password")). For
    // api_key/bearer_token the secret is a token paired with a host-default
    // username, so auth_username stays None and the token path is unchanged.
    let (auth_username, auth_token): (Option<String>, Option<String>) =
        match repository.auth_type.as_str() {
            "api_key" => (None, repository.auth_config.api_key.clone()),
            "bearer_token" => (None, repository.auth_config.token.clone()),
            "basic_auth" => match (
                &repository.auth_config.username,
                &repository.auth_config.password,
            ) {
                (Some(username), Some(password)) => {
                    (Some(username.clone()), Some(password.clone()))
                }
                _ => (None, None),
            },
            "none" | _ => (None, None),
        };

    // Create cancellation token for this download
    let cancellation_token =
        crate::utils::cancellation::create_cancellation_token(download_id).await;

    // Spawn background task to handle the download
    let download_handle = tokio::spawn(async move {
        // Update status to downloading
        if let Err(e) = Repos.download_instance
            .update_status(
                download_id,
                types::UpdateDownloadStatusRequest {
                    status: DownloadStatus::Downloading,
                    error_message: None,
                    model_id: None,
                },
            )
            .await
        {
            tracing::error!(
                "Failed to update download status to downloading for ID {}: {}",
                download_id,
                e
            );
            return;
        }

        tracing::info!(
            "Starting download for repository: {} (ID: {})",
            repository_url,
            download_id
        );

        // Create progress channel
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<GitProgress>();

        // Create git service
        let git_service = GitService::new();

        // Spawn task to update download progress in database
        let download_id_progress = download_id;
        let progress_task = tokio::spawn(async move {
            let mut tracker = ProgressTracker::new();
            while let Some(git_progress) = progress_rx.recv().await {
                // Calculate speed and ETA
                let current_bytes = git_progress.current;
                let total_bytes = git_progress.total;
                let (speed_bps_f64, _) = tracker.update(current_bytes);
                let speed_bps = speed_bps_f64.map(|s| s as i64);
                let eta_seconds = tracker
                    .calculate_eta(current_bytes, total_bytes, speed_bps_f64)
                    .map(|eta| eta as i64);

                let progress_data = DownloadProgressData {
                    phase: git_phase_to_download_phase(git_progress.phase),
                    current: git_progress.current as i64,
                    total: git_progress.total as i64,
                    message: git_progress.message.clone(),
                    speed_bps: speed_bps.unwrap_or(0),
                    eta_seconds: eta_seconds.unwrap_or(0),
                };

                let status = match git_progress.phase {
                    GitPhase::Error => Some(DownloadStatus::Failed),
                    _ => None,
                };

                let _ = Repos.download_instance
                    .update_progress(
                        download_id_progress,
                        types::UpdateDownloadProgressRequest {
                            progress_data,
                            status,
                        },
                    )
                    .await;

                // Break on error phase
                if matches!(git_progress.phase, GitPhase::Error) {
                    break;
                }
            }
        });

        // Clone repository (LFS files not included in initial clone)
        let clone_result = git_service
            .clone_repository(
                &repository_url,
                &request.repository_id,
                request.repository_branch.as_deref(),
                auth_token.as_deref(),
                auth_username.as_deref(),
                progress_tx.clone(),
                Some(cancellation_token.clone()),
            )
            .await;

        // Drop the progress sender to signal completion to the progress task
        drop(progress_tx);

        // Wait for progress task with timeout to ensure it processes any final messages
        let _ = tokio::time::timeout(std::time::Duration::from_secs(10), progress_task).await;

        tracing::info!("Clone result: {:?}", clone_result);

        match clone_result {
            Ok(cache_path) => {
                // Update progress: Analyzing files
                let _ = Repos.download_instance
                    .update_progress(
                        download_id,
                        types::UpdateDownloadProgressRequest {
                            progress_data: DownloadProgressData {
                                phase: DownloadPhase::Analyzing,
                                current: 10,
                                total: 100,
                                message: "Analyzing repository files...".to_string(),
                                speed_bps: 0,
                                eta_seconds: 0,
                            },
                            status: None,
                        },
                    )
                    .await;

                // List files in the repository
                let source_files = match std::fs::read_dir(&cache_path) {
                    Ok(entries) => entries
                        .filter_map(|entry| {
                            entry
                                .ok()
                                .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
                        })
                        .filter(|name| !name.starts_with('.'))
                        .collect::<Vec<String>>(),
                    Err(e) => {
                        tracing::error!("Failed to read repository directory: {}", e);
                        crate::utils::cancellation::remove_download_tracking(download_id).await;

                        let _ = Repos.download_instance
                            .update_status(
                                download_id,
                                types::UpdateDownloadStatusRequest {
                                    status: DownloadStatus::Failed,
                                    error_message: Some(format!(
                                        "Failed to read repository directory: {}",
                                        e
                                    )),
                                    model_id: None,
                                },
                            )
                            .await;
                        return;
                    }
                };

                // Determine which files to copy. NOTE: this selection drives
                // the LFS pull below, and `create_model_with_files` re-derives
                // the SAME selection from the same clone dir to copy into
                // storage. The two must agree — they do because the only step
                // between them (git-LFS smudge) rewrites pointer *content* in
                // place and never changes the directory's file-name set, which
                // is all `select_download_files` keys on.
                let files_to_copy =
                    match determine_files_to_copy(&source_files, &request.main_filename) {
                        Ok(files) => files,
                        Err(e) => {
                            tracing::error!("Failed to determine files to copy: {}", e);
                            crate::utils::cancellation::remove_download_tracking(download_id).await;

                            let _ = Repos.download_instance
                                .update_status(
                                    download_id,
                                    types::UpdateDownloadStatusRequest {
                                        status: DownloadStatus::Failed,
                                        error_message: Some(format!(
                                            "Failed to determine files to copy: {}",
                                            e
                                        )),
                                        model_id: None,
                                    },
                                )
                                .await;
                            return;
                        }
                    };

                // Update progress: Downloading LFS files
                let _ = Repos.download_instance
                    .update_progress(
                        download_id,
                        types::UpdateDownloadProgressRequest {
                            progress_data: DownloadProgressData {
                                phase: DownloadPhase::Downloading,
                                current: 20,
                                total: 100,
                                message: "Checking for LFS files...".to_string(),
                                speed_bps: 0,
                                eta_seconds: 0,
                            },
                            status: None,
                        },
                    )
                    .await;

                // Create new progress channel for LFS
                let (lfs_progress_tx, _lfs_progress_rx) = mpsc::unbounded_channel::<GitProgress>();

                // Pull LFS files
                let lfs_result = git_service
                    .pull_lfs_files_with_cancellation(
                        &cache_path,
                        &files_to_copy,
                        auth_token.as_deref(),
                        lfs_progress_tx,
                        Some(cancellation_token.clone()),
                    )
                    .await;

                // Check LFS result
                if let Err(e) = lfs_result {
                    let is_cancelled = matches!(e, GitError::Cancelled);
                    let (status, error_msg) = if is_cancelled {
                        (
                            DownloadStatus::Cancelled,
                            "Download was cancelled by user".to_string(),
                        )
                    } else {
                        (
                            DownloadStatus::Failed,
                            format!("Failed to download LFS files: {}", e),
                        )
                    };

                    crate::utils::cancellation::remove_download_tracking(download_id).await;

                    let _ = Repos.download_instance
                        .update_status(
                            download_id,
                            types::UpdateDownloadStatusRequest {
                                status,
                                error_message: Some(error_msg),
                                model_id: None,
                            },
                        )
                        .await;
                    return;
                }

                // Update progress: Creating model
                let _ = Repos.download_instance
                    .update_progress(
                        download_id,
                        types::UpdateDownloadProgressRequest {
                            progress_data: DownloadProgressData {
                                phase: DownloadPhase::Committing,
                                current: 90,
                                total: 100,
                                message: "Creating model from downloaded files...".to_string(),
                                speed_bps: 0,
                                eta_seconds: 0,
                            },
                            status: None,
                        },
                    )
                    .await;

                // Create model with files
                match create_model_with_files(
                    &Repos.llm_model,
                    CreateModelWithFilesRequest {
                        provider_id: request.provider_id,
                        name: request.name,
                        display_name: request.display_name,
                        description: request.description,
                        file_format: request.file_format,
                        main_filename: request.main_filename,
                        source_dir: cache_path,
                        capabilities: request.capabilities,
                        parameters: request.parameters,
                        engine_type: request.engine_type,
                        engine_settings: request.engine_settings,
                    },
                )
                .await
                {
                    Ok(model) => {
                        tracing::info!(
                            "Model created successfully from download: {} ({})",
                            model.display_name,
                            model.id
                        );

                        // Update download as completed with model ID
                        let _ = Repos.download_instance
                            .update_status(
                                download_id,
                                types::UpdateDownloadStatusRequest {
                                    status: DownloadStatus::Completed,
                                    error_message: None,
                                    model_id: Some(model.id),
                                },
                            )
                            .await;

                        // P1.k: background Tier-2 validation for local
                        // engine models freshly downloaded from HF.
                        if matches!(
                            model.engine_type,
                            EngineType::Llamacpp | EngineType::Mistralrs
                        ) {
                            crate::modules::llm_local_runtime::validator::enqueue(
                                model.id,
                                crate::modules::llm_local_runtime::validator::ValidationTier::Tier2,
                            )
                            .await;
                        }

                        crate::utils::cancellation::remove_download_tracking(download_id).await;

                        // Note: Don't delete the download record immediately
                        // The SSE needs it to send the provider_id to the frontend
                        // Frontend will delete it after processing the complete event
                    }
                    Err(e) => {
                        tracing::error!("Failed to create model: {}", e);
                        crate::utils::cancellation::remove_download_tracking(download_id).await;

                        let _ = Repos.download_instance
                            .update_status(
                                download_id,
                                types::UpdateDownloadStatusRequest {
                                    status: DownloadStatus::Failed,
                                    error_message: Some(format!("Failed to create model: {}", e)),
                                    model_id: None,
                                },
                            )
                            .await;
                    }
                }
            }
            Err(e) => {
                let is_cancelled = matches!(e, GitError::Cancelled);

                let (status, error_msg) = if is_cancelled {
                    (
                        DownloadStatus::Cancelled,
                        "Download was cancelled by user".to_string(),
                    )
                } else if matches!(e, GitError::AccessDenied(_))
                    || matches!(e, GitError::HttpStatus { status: 401 | 403, .. })
                    // Fallback for git2 smart-HTTP transport auth failures, whose
                    // status is only available inside the opaque git2 message.
                    || (matches!(e, GitError::Git(_))
                        && (e.to_string().contains("403") || e.to_string().contains("401")))
                {
                    (
                        DownloadStatus::Failed,
                        format!(
                            "Access denied (401/403): Authentication failed or insufficient permissions. {}",
                            e
                        ),
                    )
                } else {
                    (DownloadStatus::Failed, format!("Download failed: {}", e))
                };

                crate::utils::cancellation::remove_download_tracking(download_id).await;

                tracing::info!(
                    "Updating download {} status to {:?} with error: {}",
                    download_id,
                    status,
                    error_msg
                );

                if let Err(e) = Repos.download_instance
                    .update_status(
                        download_id,
                        types::UpdateDownloadStatusRequest {
                            status,
                            error_message: Some(error_msg.clone()),
                            model_id: None,
                        },
                    )
                    .await
                {
                    tracing::error!(
                        "Failed to update download {} status to {:?}: {}",
                        download_id,
                        status,
                        e
                    );
                } else {
                    tracing::info!(
                        "Successfully updated download {} status to {:?}",
                        download_id,
                        status
                    );
                }
            }
        }
    });

    // Spawn a watcher to log + RECONCILE panics from the background download
    // task (tokio::spawn panics are stored in the JoinHandle and silently
    // swallowed if the handle is dropped without awaiting). A panicked task
    // never reaches its own failure path, so the row would be stuck in
    // 'downloading' forever — mark it failed so the UI and any retry logic see
    // a terminal state.
    tokio::spawn(async move {
        if let Err(join_err) = download_handle.await {
            tracing::error!(
                "Download background task panicked for download {}: {:?}",
                download_id,
                join_err,
            );
            if join_err.is_panic() {
                if let Err(e) = Repos
                    .download_instance
                    .update_status(
                        download_id,
                        types::UpdateDownloadStatusRequest {
                            status: DownloadStatus::Failed,
                            error_message: Some(
                                "download task aborted unexpectedly (panic)".to_string(),
                            ),
                            model_id: None,
                        },
                    )
                    .await
                {
                    tracing::error!(
                        "Failed to mark panicked download {} as failed: {}",
                        download_id,
                        e
                    );
                }
            }
        }
    });

    // Return the download instance immediately
    Ok(download_instance)
}

/// Public API endpoint for initiating repository download
#[debug_handler]
pub async fn initiate_repository_download(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    Json(request): Json<DownloadFromRepositoryRequest>,
) -> ApiResult<Json<DownloadInstance>> {
    let download_instance = initiate_repository_download_internal(request)
        .await
        .map_err(|e| {
            // Propagate "Repository not found" as 404 rather than the
            // catch-all 500. The internal function returns String
            // errors today; map by content prefix until the function
            // is refactored to return AppError.
            if e.contains("not found") {
                (
                    StatusCode::NOT_FOUND,
                    AppError::not_found("Repository"),
                )
            } else {
                tracing::error!(error = %e, "initiate_repository_download_internal failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error("Storage error"),
                )
            }
        })?;

    Ok((StatusCode::OK, Json(download_instance)))
}

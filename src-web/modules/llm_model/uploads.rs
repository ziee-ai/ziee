// LLM Model file upload and download handlers
// Adapted from react-test/src-tauri/src/api/model_uploads.rs for ziee-chat

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::Json,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::common::r#type::{ApiResult, AppError};
use crate::modules::permissions::RequirePermissions;
use crate::utils::git::{GitError, GitPhase, GitProgress, GitService};

use super::{
    models::{
        DownloadInstance, DownloadPhase, DownloadProgressData, DownloadRequestData,
        DownloadStatus, EngineType, FileFormat, LlmModel, ModelCapabilities, ModelEngineSettings,
        ModelParameters, SourceInfo,
    },
    permissions::*,
    repository,
    storage::ModelStorage,
    types::{
        CreateDownloadInstanceRequest, CreateLlmModelRequest, UpdateDownloadProgressRequest,
        UpdateDownloadStatusRequest,
    },
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
    pub source: Option<SourceInfo>,
}

/// Shared model creation and file processing logic
async fn create_model_with_files(
    pool: &sqlx::PgPool,
    repo: &repository::LlmModelRepository,
    request: CreateModelWithFilesRequest,
) -> Result<LlmModel, AppError> {
    // Initialize storage
    let storage = ModelStorage::new()
        .await
        .map_err(|e| AppError::internal_error(&format!("Failed to initialize storage: {}", e)))?;

    // Validate provider exists and is of type 'local'
    let provider = crate::modules::llm_provider::repository::get_llm_provider_by_id(pool, request.provider_id)
        .await
        .map_err(|e| AppError::internal_error(&e.to_string()))?
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

    tracing::info!("Processing model with file format: {}", request.file_format.as_str());

    // Create storage directory
    storage
        .create_model_directory(&request.provider_id, &model_id)
        .await
        .map_err(|e| {
            AppError::internal_error(&format!("Failed to create storage directory: {}", e))
        })?;

    tracing::debug!("Source directory for model files: {}", request.source_dir.display());

    // List all files in the source directory
    let source_files = match tokio::fs::read_dir(&request.source_dir).await {
        Ok(mut entries) => {
            let mut files = Vec::new();
            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                AppError::internal_error(&format!("Failed to read directory entry: {}", e))
            })? {
                if entry
                    .file_type()
                    .await
                    .map_err(|e| {
                        AppError::internal_error(&format!("Failed to get file type: {}", e))
                    })?
                    .is_file()
                {
                    files.push(entry.file_name().to_string_lossy().to_string());
                }
            }
            files
        }
        Err(e) => {
            return Err(AppError::internal_error(&format!(
                "Failed to read source directory: {}",
                e
            )));
        }
    };

    if source_files.is_empty() {
        return Err(AppError::bad_request("VALIDATION_ERROR", "No files found in source directory"));
    }

    // Determine which files to copy based on main filename and index files
    let files_to_copy = determine_files_to_copy(&source_files, &request.main_filename)?;

    if files_to_copy.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            &format!(
                "No relevant files found for main filename: {}",
                request.main_filename
            )
        ));
    }

    tracing::info!("Found {} files to copy: {:?}", files_to_copy.len(), files_to_copy);

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
            AppError::internal_error(&format!(
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
                AppError::internal_error(&format!("Failed to copy file {}: {}", filename, e))
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
            filename, relative_path, file_size
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
        source: request.source,
    };

    // Create the model record - it will generate its own ID
    let model_db = repo.create(create_request)
        .await
        .map_err(|e| {
            let error_str = e.to_string();
            tracing::warn!("Database error during model creation: {}", error_str);
            if error_str.contains("llm_models_provider_id_name_unique")
                || (error_str.contains("duplicate key") && error_str.contains("name")) {
                AppError::bad_request(
                    "DUPLICATE_ENTRY",
                    &format!(
                        "A model with the name '{}' already exists for this provider. Please use a different model name.",
                        model_name
                    )
                )
            } else {
                e
            }
        })?;

    // The database record has been created with its own ID
    // But files are in directory with the pre-generated model_id
    // We need to rename the directory to match the database ID
    let old_dir = storage.get_model_path(&request.provider_id, &model_id);
    let new_dir = storage.get_model_path(&request.provider_id, &model_db.id);

    if old_dir != new_dir {
        tokio::fs::rename(&old_dir, &new_dir)
            .await
            .map_err(|e| {
                AppError::internal_error(&format!("Failed to rename model directory: {}", e))
            })?;
        tracing::debug!("Renamed model directory from {} to {}", old_dir.display(), new_dir.display());
    }

    // Create all file records in the database
    // TODO: Add model_files table support when repository functions are ready

    // Update model with total size and validation status using the correct model ID
    repo.set_validation_status(model_db.id, "completed", None)
        .await?;

    // Return the created model directly
    let model = model_db;

    tracing::info!(
        "Model created successfully: {} files, {} total size",
        file_count, total_size
    );

    Ok(model)
}

/// Determine which files to copy based on main filename and index files
fn determine_files_to_copy(
    source_files: &[String],
    main_filename: &str,
) -> Result<Vec<String>, AppError> {
    let mut files_to_copy = Vec::new();

    // First, check if main_filename ends with .json (if so, it might be an index file already)
    let main_is_json = main_filename.to_lowercase().ends_with(".json");

    // If main file doesn't end with .json, look for {main_filename}.index.json
    let index_filename = if !main_is_json {
        format!("{}.index.json", main_filename)
    } else {
        main_filename.to_string()
    };

    // Always check for index file first
    let index_exists = !main_is_json && source_files.contains(&index_filename);
    let main_exists = source_files.contains(&main_filename.to_string());

    // Check if index file exists first
    if index_exists {
        tracing::debug!("Found index file: {}", index_filename);

        // Add the index file itself
        files_to_copy.push(index_filename.clone());

        // Extract base name from main filename
        let mut base_name = main_filename
            .trim_end_matches(".safetensors")
            .trim_end_matches(".bin")
            .trim_end_matches(".pt")
            .trim_end_matches(".pth")
            .trim_end_matches(".gguf");

        // Handle case where user provided a sharded filename as main filename
        if let Some(of_pos) = base_name.find("-of-") {
            let before_of = &base_name[..of_pos];
            if let Some(dash_pos) = before_of.rfind('-') {
                if before_of[dash_pos + 1..]
                    .chars()
                    .all(|c| c.is_ascii_digit())
                {
                    base_name = &before_of[..dash_pos];
                }
            }
        } else if let Some(of_pos) = base_name.find("_of_") {
            let before_of = &base_name[..of_pos];
            if let Some(underscore_pos) = before_of.rfind('_') {
                if before_of[underscore_pos + 1..]
                    .chars()
                    .all(|c| c.is_ascii_digit())
                {
                    base_name = &before_of[..underscore_pos];
                }
            }
        }

        // Add all weight files that match the sharding pattern
        for file in source_files {
            if file.starts_with(base_name)
                && (file.contains("-of-") || file.contains("_of_"))
                && (file.ends_with(".safetensors")
                    || file.ends_with(".bin")
                    || file.ends_with(".pt")
                    || file.ends_with(".pth")
                    || file.ends_with(".gguf"))
            {
                files_to_copy.push(file.clone());
            }
        }
    } else if main_is_json
        && (main_filename.contains("index") || main_filename.ends_with(".index.json"))
    {
        tracing::debug!("Main file is an index file: {}", main_filename);

        files_to_copy.push(main_filename.to_string());

        // Extract base name from index file
        let base_name = main_filename
            .replace(".index.json", "")
            .replace("_index.json", "")
            .replace("-index.json", "");

        // Add all related weight files
        for file in source_files {
            if file.starts_with(&base_name)
                && file != main_filename
                && (file.ends_with(".safetensors")
                    || file.ends_with(".bin")
                    || file.ends_with(".pt")
                    || file.ends_with(".pth")
                    || file.ends_with(".gguf"))
            {
                files_to_copy.push(file.clone());
            }
        }
    } else if main_exists {
        tracing::debug!(
            "No index file found for {}. Only copying main file.",
            main_filename
        );
        files_to_copy.push(main_filename.to_string());
    } else {
        return Err(AppError::bad_request(
            "NOT_FOUND",
            &format!(
                "Neither '{}' nor '{}' found in source directory",
                main_filename,
                if !main_is_json {
                    &index_filename
                } else {
                    main_filename
                }
            )
        ));
    }

    // Always add configuration and tokenizer files regardless of sharding
    for file in source_files {
        if is_config_or_tokenizer_file(file) && !files_to_copy.contains(&file.to_string()) {
            files_to_copy.push(file.clone());
        }
    }

    // Remove duplicates and sort
    files_to_copy.sort();
    files_to_copy.dedup();

    tracing::debug!("Files to copy: {:?}", files_to_copy);

    Ok(files_to_copy)
}

/// Check if a file is a configuration or tokenizer file
fn is_config_or_tokenizer_file(filename: &str) -> bool {
    let filename_lower = filename.to_lowercase();
    filename_lower.ends_with("config.json")
        || filename_lower.ends_with("tokenizer.json")
        || filename_lower.ends_with("tokenizer_config.json")
        || filename_lower.ends_with("vocab.json")
        || filename_lower.ends_with("merges.txt")
        || filename_lower.ends_with("special_tokens_map.json")
        || filename_lower.ends_with("vocab.txt")
        || filename_lower.ends_with("spiece.model")
        || filename_lower == "generation_config.json"
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
    pub source: SourceInfo,
}

/// Upload multiple model files and auto-commit as a model
pub async fn upload_multiple_files_and_commit(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    State(pool): State<sqlx::PgPool>,
    mut multipart: Multipart,
) -> ApiResult<Json<LlmModel>> {
    // Create repository instances
    let model_repo = repository::LlmModelRepository::new(pool.clone());

    let storage = ModelStorage::new().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(&format!("Storage initialization failed: {}", e)),
        )
    })?;

    let mut uploaded_files = Vec::new();
    let mut main_filename: Option<String> = None;
    let mut provider_id: Option<Uuid> = None;
    let mut name: Option<String> = None;
    let mut display_name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut file_format: Option<String> = None;
    let mut capabilities: Option<ModelCapabilities> = None;
    let engine_type: Option<EngineType> = None;
    let mut engine_settings: Option<ModelEngineSettings> = None;

    // Process multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            AppError::bad_request("INVALID_INPUT", &format!("Failed to read multipart field: {}", e)),
        )
    })? {
        let field_name = field.name().unwrap_or("").to_string();

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
                            AppError::bad_request("INVALID_INPUT", &format!("Failed to read file data: {}", e)),
                        )
                    })?;

                    uploaded_files.push((filename, data.to_vec()));
                }
            }
            "main_filename" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read main_filename: {}", e)),
                    )
                })?;
                main_filename = Some(value);
            }
            "provider_id" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read provider_id: {}", e)),
                    )
                })?;
                provider_id = Some(Uuid::parse_str(&value).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Invalid provider_id format: {}", e)),
                    )
                })?);
            }
            "name" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read name: {}", e)),
                    )
                })?;
                name = Some(value);
            }
            "display_name" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read display_name: {}", e)),
                    )
                })?;
                display_name = Some(value);
            }
            "description" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read description: {}", e)),
                    )
                })?;
                description = if value.is_empty() { None } else { Some(value) };
            }
            "file_format" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read file_format: {}", e)),
                    )
                })?;
                file_format = Some(value);
            }
            "capabilities" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read capabilities: {}", e)),
                    )
                })?;
                if !value.is_empty() {
                    capabilities = serde_json::from_str(&value).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request("INVALID_INPUT", &format!("Invalid capabilities JSON: {}", e)),
                        )
                    })?;
                }
            }
            "settings" => {
                let value = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AppError::bad_request("INVALID_INPUT", &format!("Failed to read settings: {}", e)),
                    )
                })?;
                if !value.is_empty() {
                    engine_settings = Some(serde_json::from_str(&value).map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            AppError::bad_request("INVALID_INPUT", &format!("Invalid settings JSON: {}", e)),
                        )
                    })?)
                }
            }
            _ => {
                continue;
            }
        }
    }

    // Validate required fields
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
            AppError::bad_request("MISSING_FIELD", "Missing main_filename in multipart request"),
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

        // Check and validate files
        let _file_type = determine_model_file_type(&filename);
        let _validation_issues = validate_file_content(&filename, &file_data);

        // Save files to temporary storage
        let temp_file_id = Uuid::new_v4();
        storage
            .save_temp_file(&temp_session_id, &temp_file_id, &filename, &file_data)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AppError::internal_error(&format!("Failed to save file {}: {}", filename, e)),
                )
            })?;
    }

    tracing::info!("Files uploaded successfully, total size: {} bytes", total_size);

    // Step 2: Auto-commit the uploaded files as a model
    let source_dir = crate::core::get_app_data_dir()
        .join("temp")
        .join(temp_session_id.to_string());

    // Create model using the existing function
    let model = create_model_with_files(
        &pool,
        &model_repo,
        CreateModelWithFilesRequest {
            provider_id,
            name,
            display_name,
            description,
            file_format: FileFormat::from_str(&file_format).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    AppError::bad_request("INVALID_INPUT", &format!("Invalid file format: {}", file_format)),
                )
            })?,
            main_filename,
            source_dir,
            capabilities,
            parameters: None,
            engine_type,
            engine_settings,
            source: None,
        },
    )
    .await
    .map_err(|e| e.to_api_error())?;

    tracing::info!("Model created successfully: {} ({})", model.display_name, model.id);

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
            if filename_lower == "tokenizer.json" {
                if serde_json::from_slice::<serde_json::Value>(file_data).is_err() {
                    issues.push("Tokenizer file is not valid JSON".to_string());
                }
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

/// Initiate repository download (returns immediately with download instance ID)
/// The actual download happens in a background task
pub async fn initiate_repository_download(
    _auth: RequirePermissions<(LlmModelsCreate,)>,
    State(pool): State<sqlx::PgPool>,
    Json(request): Json<DownloadFromRepositoryRequest>,
) -> ApiResult<Json<DownloadInstance>> {
    // Validate that repository exists
    let _repository = crate::modules::llm_repository::repository::get_llm_repository_by_id(&pool, request.repository_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(&format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                AppError::not_found(&format!("Repository with ID {} not found", request.repository_id)),
            )
        })?;

    // Create repository instances
    let download_repo = repository::DownloadInstanceRepository::new(pool.clone());
    let model_repo = repository::LlmModelRepository::new(pool.clone());

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
            engine_type: request.engine_type.clone(),
            engine_settings: request.engine_settings.clone(),
            source: Some(request.source.clone()),
        },
    };

    let download_instance = download_repo.create(download_request)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(&format!("Database error: {}", e)),
            )
        })?;

    // Clone necessary data for the background task
    let download_id = download_instance.id;
    let repository_url = request.repository_path.clone();
    // TODO: Add proper auth token handling when repository module is implemented
    let auth_token: Option<String> = None;

    // Clone pool for background task
    let bg_pool = pool.clone();

    // Create cancellation token for this download
    let cancellation_token =
        crate::utils::cancellation::create_cancellation_token(download_id).await;

    // Spawn background task to handle the download
    tokio::spawn(async move {
        // Create repository instance for background task
        let model_repo = repository::LlmModelRepository::new(bg_pool.clone());
        // Use the repository in the background task
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
        // TODO: Implement progress tracking with database updates

        // Clone repository (LFS files not included in initial clone)
        let clone_result = git_service
            .clone_repository(
                &repository_url,
                &request.repository_id,
                request.repository_branch.as_deref(),
                auth_token.as_deref(),
                progress_tx.clone(),
                Some(cancellation_token.clone()),
            )
            .await;

        // Drop the progress sender to signal completion to the progress task
        drop(progress_tx);

        tracing::info!("Clone result: {:?}", clone_result);

        match clone_result {
            Ok(cache_path) => {
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
                        return;
                    }
                };

                // Determine which files to copy
                let files_to_copy =
                    match determine_files_to_copy(&source_files, &request.main_filename) {
                        Ok(files) => files,
                        Err(e) => {
                            tracing::error!("Failed to determine files to copy: {}", e);
                            crate::utils::cancellation::remove_download_tracking(download_id).await;
                            return;
                        }
                    };

                // Create new progress channel for LFS
                let (lfs_progress_tx, _lfs_progress_rx) =
                    mpsc::unbounded_channel::<GitProgress>();

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
                    if is_cancelled {
                        tracing::info!("Download was cancelled by user");
                    } else {
                        tracing::error!("Failed to download LFS files: {}", e);
                    }
                    crate::utils::cancellation::remove_download_tracking(download_id).await;
                    return;
                }

                // Create model with files
                match create_model_with_files(
                    &bg_pool,
                    &model_repo,
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
                        source: Some(request.source.clone()),
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
                        crate::utils::cancellation::remove_download_tracking(download_id).await;
                    }
                    Err(e) => {
                        tracing::error!("Failed to create model: {}", e);
                        crate::utils::cancellation::remove_download_tracking(download_id).await;
                    }
                }
            }
            Err(e) => {
                let is_cancelled = matches!(e, GitError::Cancelled);
                if is_cancelled {
                    tracing::info!("Download was cancelled by user");
                } else {
                    tracing::error!("Download failed: {}", e);
                }
                crate::utils::cancellation::remove_download_tracking(download_id).await;
            }
        }
    });

    // Return the download instance immediately
    Ok((StatusCode::OK, Json(download_instance)))
}

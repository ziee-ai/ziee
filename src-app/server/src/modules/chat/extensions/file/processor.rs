//! File → provider-specific ContentBlock conversion.
//!
//! Single source of truth for the provider-routing branch used by:
//!   * `FileExtension::before_llm_call` — per-message attachments.
//!   * `ProjectExtension::before_llm_call` — project-scoped knowledge
//!     files (prepended onto the user message ahead of the
//!     per-message ones).
//!
//! Both extensions used to duplicate the entire OpenAI/Anthropic/Gemini
//! routing block; extracting it here closes that drift hazard (Plan 5
//! §4 "Refactor required").

use sqlx::PgPool;
use uuid::Uuid;

use ai_providers::{AIProvider, ContentBlock, DocumentSource, ImageSource};

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::llm_provider_files;

/// Process a single file ID into provider-routed ContentBlock(s).
///
/// Ownership is re-validated here as defense in depth: callers
/// (file extension via per-message file_ids, project extension via
/// project_files JOIN) already enforce it, but the per-file repository
/// lookup costs nothing extra and fails safe.
pub async fn process_file_blocks(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    provider_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file = Repos
        .file
        .get_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;

    if file.user_id != user_id {
        return Err(AppError::forbidden(
            "FILE_ACCESS_DENIED",
            "You don't have access to this file",
        ));
    }

    let mime = file
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    match provider_type {
        "anthropic" | "gemini" => {
            if mime == "application/pdf" || mime.starts_with("image/") {
                process_via_provider_api(pool, file_id, provider_id, mime, user_id).await
            } else {
                process_via_base64(file_id, &file.filename, mime, user_id).await
            }
        }
        _ => process_via_base64(file_id, &file.filename, mime, user_id).await,
    }
}

async fn process_via_provider_api(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    mime_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let provider = Repos
        .llm_provider
        .get_by_id(provider_id)
        .await?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    let file_storage = get_file_storage();
    let file_repo = &Repos.file;

    let ai_provider: &dyn AIProvider = match provider.provider_type.as_str() {
        "anthropic" => &ai_providers::AnthropicProvider,
        "gemini" => &ai_providers::GeminiProvider,
        _ => &ai_providers::OpenAIProvider,
    };

    // user_id is threaded through for the user-scoped JOIN in
    // get_provider_file_mapping — closes 06-llm-provider F-04.
    let provider_file_id = llm_provider_files::service::get_or_upload_provider_file(
        pool,
        file_repo,
        &file_storage,
        file_id,
        user_id,
        &provider,
        ai_provider,
    )
    .await?;

    if mime_type.starts_with("image/") {
        Ok(vec![ContentBlock::Image {
            source: ImageSource::File {
                file_id: provider_file_id,
            },
        }])
    } else if mime_type == "application/pdf" {
        Ok(vec![ContentBlock::Document {
            source: DocumentSource::File {
                file_id: provider_file_id,
            },
        }])
    } else {
        Ok(vec![ContentBlock::Text {
            text: format!("[File: {} ({})]", file_id, mime_type),
        }])
    }
}

async fn process_via_base64(
    file_id: Uuid,
    filename: &str,
    mime_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file_storage = get_file_storage();
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let file_data = file_storage
        .load_original(user_id, file_id, &extension)
        .await?;

    if mime_type.starts_with("image/") {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&file_data);
        Ok(vec![ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: mime_type.to_string(),
                data: base64_data,
            },
        }])
    } else if mime_type == "application/pdf" {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&file_data);
        Ok(vec![ContentBlock::Document {
            source: DocumentSource::Base64 {
                media_type: mime_type.to_string(),
                data: base64_data,
            },
        }])
    } else if mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
        )
    {
        // Text-like files: inline the content verbatim as a Text
        // ContentBlock. The pre-R4 implementation returned only the
        // filename (`[File: foo.txt]`) which meant project knowledge
        // files reached the LLM with no body, breaking the
        // `project_files_appear_in_llm_response` integration test
        // and silently misleading any user attaching .txt/.md/.json
        // knowledge to a project.
        //
        // Best-effort UTF-8: if the file isn't valid UTF-8 we fall
        // back to a labeled placeholder so a binary-disguised-as-
        // text upload doesn't crash the chat send.
        match String::from_utf8(file_data) {
            Ok(text) => Ok(vec![ContentBlock::Text {
                text: format!("[File: {filename}]\n{text}"),
            }]),
            Err(_) => Ok(vec![ContentBlock::Text {
                text: format!(
                    "[File: {filename} ({mime_type}) — non-UTF8 content omitted]"
                ),
            }]),
        }
    } else {
        // Unsupported binary type — surface as a labeled placeholder
        // so the LLM at least knows the file existed.
        Ok(vec![ContentBlock::Text {
            text: format!("[File: {filename} ({mime_type})]"),
        }])
    }
}

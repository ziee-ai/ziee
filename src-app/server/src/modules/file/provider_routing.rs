//! File → provider-specific ContentBlock conversion.
//!
//! Single source of truth for the per-provider (OpenAI / Anthropic /
//! Gemini) routing block that turns a file row into the wire format
//! the provider expects. Originally located under
//! `chat/extensions/file/processor.rs` — moved here as part of the
//! file/project/mcp bridge extraction (chat knows nothing about
//! files; the file module owns this routing API directly).
//!
//! Has ZERO chat imports: works against `ai_providers`,
//! `modules::file::storage`, and `modules::llm_provider_files`. The
//! "ContentBlock" type comes from the `ai_providers` crate, not chat,
//! so this is properly a file-module concern.
//!
//! Consumers:
//!   - file's chat-extension (per-message file attachments)
//!   - project's chat-extension (knowledge-file batches)
//!   - any future caller that needs a file → provider-routed block

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
/// already enforce it, but the per-file repository lookup costs
/// nothing extra and fails safe.
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

    // Native image/PDF for providers that support it (anthropic/gemini).
    if matches!(provider_type, "anthropic" | "gemini")
        && (mime == "application/pdf" || mime.starts_with("image/"))
    {
        return process_via_provider_api(pool, file_id, provider_id, mime, user_id).await;
    }

    // Genuine OpenAI uploads PDFs via the Files API. The mapped "openai" type is
    // shared by groq/deepseek/mistral/custom/local — none of which implement it —
    // so only a real OpenAI provider uploads; the rest fall through to the
    // extracted-text / base64 paths below. (OpenAI images stay base64 — image
    // file_id is Responses-API only.)
    if provider_type == "openai" && mime == "application/pdf" {
        let is_real_openai = Repos
            .llm_provider
            .get_by_id(provider_id)
            .await?
            .map(|p| p.provider_type == "openai")
            .unwrap_or(false);
        if is_real_openai {
            return process_via_provider_api(pool, file_id, provider_id, mime, user_id).await;
        }
    }

    // Office docs (and PDFs on providers without native PDF) that have extracted
    // text → inline the extracted per-page text instead of dropping to a useless
    // `[File: x]` placeholder. Frees the old placeholder bug. text/* and JSON-ish
    // files still go through process_via_base64's verbatim-text branch; images go
    // there too for base64.
    if file.text_page_count > 0
        && !mime.starts_with("image/")
        && !mime.starts_with("text/")
        && !crate::modules::file::available_files::is_text_like(mime)
    {
        return inline_extracted_text(
            file.blob_version_id,
            &file.filename,
            file.text_page_count as u32,
            user_id,
        )
        .await;
    }

    process_via_base64(file.blob_version_id, &file.filename, mime, user_id).await
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
                media_type: Some(mime_type.to_string()),
            },
        }])
    } else if mime_type == "application/pdf" {
        Ok(vec![ContentBlock::Document {
            source: DocumentSource::File {
                file_id: provider_file_id,
                media_type: Some(mime_type.to_string()),
            },
        }])
    } else {
        Ok(vec![ContentBlock::Text {
            text: format!("[File: {} ({})]", file_id, mime_type),
        }])
    }
}

/// Inline a doc's already-extracted per-page text as a single Text block. Used
/// for office docs and (non-native) PDFs so their content reaches the model
/// instead of a `[File: x]` placeholder.
async fn inline_extracted_text(
    blob_version_id: Uuid,
    filename: &str,
    pages: u32,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let storage = get_file_storage();
    let mut text = format!("[File: {filename}]\n");
    for p in 1..=pages {
        // HEAD blob key — file_id keys v1's text pages (stale for edited files).
        if let Ok(page) = storage.load_text_page(user_id, blob_version_id, p).await {
            text.push_str(&page);
            if !page.ends_with('\n') {
                text.push('\n');
            }
        }
    }
    Ok(vec![ContentBlock::Text { text }])
}

async fn process_via_base64(
    blob_version_id: Uuid,
    filename: &str,
    mime_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file_storage = get_file_storage();
    // Canonical extension (matches upload's blob naming); blob_version_id is the
    // HEAD blob key — file_id would load v1's stale bytes for an edited file.
    let extension = crate::modules::file::utils::extension_of(filename);
    let file_data = file_storage
        .load_original(user_id, blob_version_id, &extension)
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
        // filename (`[File: foo.txt]`) which meant attached
        // knowledge files reached the LLM with no body — silently
        // misleading any user attaching .txt/.md/.json content.
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

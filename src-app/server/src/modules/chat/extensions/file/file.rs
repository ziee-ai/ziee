use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::AppError,
    core::Repos,
    modules::{
        chat::{
            core::extension::{ChatExtension, SendMessageRequest, StreamContext},
            extensions::file::types::{FileContent, ImageSource as FileImageSource},
        },
        file::storage::manager::get_file_storage,
        llm_provider_files,
    },
};
use ai_providers::{ChatRequest, ContentBlock, DocumentSource, ImageSource, Role};
use aide::axum::ApiRouter;

pub struct FileExtension {
    pool: PgPool,
}

impl FileExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Process a single file for the given provider
    async fn process_file(
        &self,
        file_id: Uuid,
        provider_id: Uuid,
        provider_type: &str,
        user_id: Uuid,
    ) -> Result<Vec<ContentBlock>, AppError> {
        // Get file metadata ONCE
        let file = Repos
            .file
            .get_by_id(file_id)
            .await?
            .ok_or_else(|| AppError::not_found("File"))?;

        // Validate ownership
        if file.user_id != user_id {
            return Err(AppError::forbidden("FILE_ACCESS_DENIED", "You don't have access to this file"));
        }

        let mime = file.mime_type.as_deref().unwrap_or("application/octet-stream");

        // Route based on provider type AND file type
        match provider_type {
            "anthropic" | "gemini" => {
                // Provider Files API supports both PDFs and images
                if mime == "application/pdf" || mime.starts_with("image/") {
                    self.process_file_with_provider_api(file_id, provider_id, &file.filename, mime, user_id)
                        .await
                } else {
                    // Other types use base64 fallback
                    self.process_file_base64(file_id, &file.filename, mime, user_id)
                        .await
                }
            }
            _ => {
                // All other providers use base64
                self.process_file_base64(file_id, &file.filename, mime, user_id)
                    .await
            }
        }
    }

    async fn process_file_with_provider_api(
        &self,
        file_id: Uuid,
        provider_id: Uuid,
        filename: &str,
        mime_type: &str,
        _user_id: Uuid,
    ) -> Result<Vec<ContentBlock>, AppError> {
        // Get provider configuration
        let provider = Repos
            .llm_provider
            .get_by_id(provider_id)
            .await?
            .ok_or_else(|| AppError::not_found("Provider"))?;

        // Get file storage singleton
        let file_storage = get_file_storage();

        // Get FileRepository
        let file_repo = &Repos.file;

        // Create AI provider instance (stateless trait object)
        let ai_provider: &dyn ai_providers::AIProvider = match provider.provider_type.as_str() {
            "anthropic" => &ai_providers::AnthropicProvider,
            "gemini" => &ai_providers::GeminiProvider,
            _ => &ai_providers::OpenAIProvider,
        };

        // Upload to provider or get cached file ID
        let provider_file_id = llm_provider_files::service::get_or_upload_provider_file(
            &self.pool,
            file_repo,
            &file_storage,
            file_id,
            &provider,
            ai_provider,
        )
        .await?;

        // Return appropriate content block based on mime type
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
            // Unsupported type - return text description
            Ok(vec![ContentBlock::Text {
                text: format!("[File: {} ({})]", filename, mime_type),
            }])
        }
    }

    async fn process_file_base64(
        &self,
        file_id: Uuid,
        filename: &str,
        mime_type: &str,
        user_id: Uuid,
    ) -> Result<Vec<ContentBlock>, AppError> {
        // Load file data from storage
        let file_storage = get_file_storage();
        let extension = get_extension(filename);
        let file_data = file_storage
            .load_original(user_id, file_id, &extension)
            .await?;

        // Encode as base64
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&file_data);

        if mime_type.starts_with("image/") {
            Ok(vec![ContentBlock::Image {
                source: ImageSource::Base64 {
                    media_type: mime_type.to_string(),
                    data: base64_data,
                },
            }])
        } else if mime_type == "application/pdf" {
            Ok(vec![ContentBlock::Document {
                source: DocumentSource::Base64 {
                    media_type: mime_type.to_string(),
                    data: base64_data,
                },
            }])
        } else {
            Ok(vec![ContentBlock::Text {
                text: format!("[File: {}]", filename),
            }])
        }
    }
}

#[async_trait]
impl ChatExtension for FileExtension {
    fn name(&self) -> &str {
        "file"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("File extension initialized");
        Ok(())
    }

    async fn provide_user_message_content(
        &self,
        context: &StreamContext,
        send_request: &SendMessageRequest,
        _text_content: &str,
    ) -> Result<Vec<crate::modules::chat::core::models::content::MessageContentData>, AppError> {
        // Check if request has file_ids
        let file_ids = match &send_request.file_ids {
            Some(ids) if !ids.is_empty() => ids,
            _ => return Ok(Vec::new()), // No files
        };

        let mut content_blocks = Vec::new();

        for file_id in file_ids {
            // Get file metadata
            let file = Repos
                .file
                .get_by_id(*file_id)
                .await?
                .ok_or_else(|| AppError::not_found("File"))?;

            // Validate ownership
            if file.user_id != context.user_id {
                return Err(AppError::forbidden(
                    "FILE_ACCESS_DENIED",
                    &format!("You don't have access to file {}", file_id),
                ));
            }

            // Create FileAttachment using FileContent enum
            let file_content = FileContent::FileAttachment {
                file_id: *file_id,
                filename: file.filename,
                mime_type: file.mime_type,
                file_size: file.file_size,
            };

            // Convert to MessageContentData::Extension
            content_blocks.push(file_content.to_message_content());
        }

        Ok(content_blocks)
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    ) -> Result<(), AppError> {
        // Access file_ids directly from composed request!
        if let Some(file_ids) = &send_request.file_ids {
            if file_ids.is_empty() {
                return Ok(());
            }

            // Get provider info from context
            let provider_id = context
                .metadata
                .get("provider_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| AppError::internal_error("Provider ID not in context"))?;

            let provider_type = context
                .metadata
                .get("provider_type")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::internal_error("Provider type not in context"))?;

            // Process each file
            let mut file_blocks = Vec::new();
            for file_id in file_ids {
                let blocks = self
                    .process_file(*file_id, provider_id, provider_type, context.user_id)
                    .await?;
                file_blocks.extend(blocks);
            }

            // Add file blocks to the user's message
            if let Some(last_message) = request.messages.last_mut() {
                if last_message.role == Role::User {
                    last_message.content.extend(file_blocks);
                }
            }
        }

        Ok(())
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
    }

    fn handled_content_types(&self) -> Vec<&'static str> {
        vec!["file"]
    }

    async fn process_content_for_llm(
        &self,
        content: &crate::modules::chat::core::models::content::MessageContentData,
        context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        // Try to extract FileContent from MessageContentData::Extension
        let file_content = match FileContent::from_message_content(content) {
            Some(fc) => fc,
            None => return Ok(None), // Not a file extension content
        };

        // Process based on FileContent variant
        match file_content {
            FileContent::FileAttachment { file_id, .. } => {
                // Get provider info from context
                let provider_id = context
                    .metadata
                    .get("provider_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or_else(|| AppError::internal_error("Provider ID not in context"))?;

                let provider_type = context
                    .metadata
                    .get("provider_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AppError::internal_error("Provider type not in context"))?;

                // Process file and return appropriate ContentBlock
                let blocks = self
                    .process_file(file_id, provider_id, provider_type, context.user_id)
                    .await?;

                // Return first block (process_file returns Vec but we need single)
                Ok(blocks.into_iter().next())
            }
            FileContent::Image { source, .. } => {
                // Convert FileImageSource to ai_providers::ImageSource
                let ai_source = match source {
                    FileImageSource::Url { url } => ImageSource::Url { url, detail: None },
                    FileImageSource::Base64 { media_type, data } => {
                        ImageSource::Base64 { media_type, data }
                    }
                    FileImageSource::File { file_id } => ImageSource::File { file_id },
                };

                Ok(Some(ContentBlock::Image { source: ai_source }))
            }
        }
    }

    async fn process_content_from_db(
        &self,
        _content: &mut crate::modules::chat::core::models::content::MessageContentData,
        _context: &StreamContext,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

/// Helper function to extract file extension
fn get_extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

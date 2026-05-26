use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::AppError,
    core::Repos,
    modules::chat::{
        core::extension::{BeforeLlmAction, ChatExtension, SendMessageRequest, StreamContext},
        extensions::file::{
            processor::process_file_blocks,
            types::{FileContent, ImageSource as FileImageSource},
        },
    },
};
use ai_providers::{ChatRequest, ContentBlock, ImageSource, Role};
use aide::axum::ApiRouter;

pub struct FileExtension {
    pool: PgPool,
}

impl FileExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
                    format!("You don't have access to file {}", file_id),
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
    ) -> Result<BeforeLlmAction, AppError> {
        if let Some(file_ids) = &send_request.file_ids {
            if file_ids.is_empty() {
                return Ok(BeforeLlmAction::Continue);
            }

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

            // Process each file via the shared free function (also used
            // by ProjectExtension at order 8). Single source of truth
            // for provider-specific routing — see processor.rs.
            let mut file_blocks = Vec::new();
            for file_id in file_ids {
                let blocks = process_file_blocks(
                    &self.pool,
                    *file_id,
                    provider_id,
                    provider_type,
                    context.user_id,
                )
                .await?;
                file_blocks.extend(blocks);
            }

            if let Some(last_message) = request.messages.last_mut()
                && last_message.role == Role::User
            {
                last_message.content.extend(file_blocks);
            }
        }

        Ok(BeforeLlmAction::Continue)
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

        match file_content {
            FileContent::FileAttachment { file_id, .. } => {
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

                let blocks = process_file_blocks(
                    &self.pool,
                    file_id,
                    provider_id,
                    provider_type,
                    context.user_id,
                )
                .await?;

                Ok(blocks.into_iter().next())
            }
            FileContent::Image { source, .. } => {
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

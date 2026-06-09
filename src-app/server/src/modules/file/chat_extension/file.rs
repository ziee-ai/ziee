use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    common::AppError,
    core::Repos,
    modules::{
        chat::core::extension::{
            BeforeLlmAction, ChatExtension, SendMessageRequest, StreamContext,
        },
        file::provider_routing::process_file_blocks,
    },
};
use ai_providers::{ChatMessage, ChatRequest, ContentBlock, ImageSource, Role};
use aide::axum::ApiRouter;

use super::types::{FileContent, ImageSource as FileImageSource};

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
        // Capability-gated manifest (Track A). When the model can use tools,
        // inject a compact manifest of ALL files available to this conversation
        // (project knowledge + attachments) and flag the MCP extension to
        // auto-attach the built-in `files` server. The model then reads files on
        // demand via read_file/grep_files instead of us inlining everything every
        // turn. The CURRENT upload is still inlined below (the user is asking
        // about it now); older files reach the model through the read tools.
        // Compute the tool-capability once per LLM iteration and memoize it into
        // `context.metadata` (idempotent — whichever extension's `before_llm_call`
        // runs first seeds it; the other before-call extensions read the cached
        // boolean). NOTE: the per-history `process_content_for_llm` path does NOT
        // see this memo — it runs on a SEPARATE `transform_context` whose metadata
        // is an empty map, built + consumed before this `stream_context` exists,
        // so its `model_supports_tools` call short-circuits to `false`.
        let tool_capable =
            crate::modules::file::available_files::ensure_model_tools_capable(
                &mut context.metadata,
            )
            .await;
        if tool_capable {
            match crate::modules::file::available_files::resolve_available_files(
                context.conversation_id,
                context.user_id,
            )
            .await
            {
                Ok(files) if !files.is_empty() => {
                    let manifest =
                        crate::modules::file::available_files::render_manifest(&files);
                    request.messages.insert(
                        0,
                        ChatMessage {
                            role: Role::System,
                            content: vec![ContentBlock::Text { text: manifest }],
                        },
                    );
                    context
                        .metadata
                        .insert("attach_files_mcp".to_string(), serde_json::json!("true"));
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("file ext: resolve_available_files failed: {e}");
                }
            }
        }

        // Inline the CURRENT message's upload (always — attached to THIS message).
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

            // Process each file via the shared free function —
            // single source of truth for provider-specific routing
            // (see modules/file/provider_routing.rs).
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

    /// File-attachment blocks that landed on ASSISTANT messages (via MCP
    /// tool results that produced files) are UI-only artifacts — the
    /// LLM already saw them described in the ToolResult content. Forwarding
    /// them again as image/document blocks confuses the LLM into
    /// re-describing the file. Owning the skip-decision here keeps chat's
    /// streaming code from having to know the `FileAttachment` variant name.
    async fn should_skip_in_assistant_forwarding(
        &self,
        content: &crate::modules::chat::core::models::content::MessageContentData,
        _context: &StreamContext,
    ) -> Result<bool, AppError> {
        let Some(file_content) = FileContent::from_message_content(content) else {
            return Ok(false);
        };
        Ok(matches!(file_content, FileContent::FileAttachment { .. }))
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
            FileContent::FileAttachment {
                file_id, mime_type, ..
            } => {
                // Recency rule (Track A §2): on a tool-capable model, OLD
                // attachments are DROPPED from the replayed history — they're
                // listed in the auto-injected manifest and read on demand via
                // `read_file`, so re-inlining them every turn was pure waste. We
                // drop rather than insert a marker so the CURRENT-turn upload
                // (whose attachment block also passes through here, and which
                // `before_llm_call` re-inlines in full) doesn't end up with both a
                // misleading "attached earlier" note AND its full content.
                // IMAGES are kept inlined for vision continuity (they can't be
                // re-read as text). Non-tool-capable models keep the full inline.
                let is_image = mime_type
                    .as_deref()
                    .map(|m| m.starts_with("image/"))
                    .unwrap_or(false);
                let tool_capable =
                    crate::modules::file::available_files::model_supports_tools(&context.metadata)
                        .await;
                if tool_capable && !is_image {
                    return Ok(None);
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
                // Images ALWAYS stay inlined (vision continuity), even on a
                // tool-capable model — unlike `FileAttachment`, an `Image` block
                // is a URL/base64/tool-produced image with no guaranteed
                // file-backed entry in the available-files set, so there is no
                // `read_file(id)` fallback to re-fetch it. Dropping it would
                // blind the model to an image it previously saw. These cost
                // nothing extra for URL refs (the provider fetches) and are the
                // model's only handle on tool-produced images.
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

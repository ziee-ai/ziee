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
        // auto-attach the built-in `files` server, so the model reads files on
        // demand via read_file/grep_files instead of us inlining everything.
        //
        // The available-files set was resolved ONCE this iteration by
        // streaming.rs::seed_available_files and shared via `context.metadata`,
        // so the manifest here and the replay recency-drop
        // (process_content_for_llm) use the SAME resolution and cannot disagree —
        // a resolve failure leaves `manifest_available` false, which makes BOTH
        // the manifest skip and the drop fall back to inlining (no data loss).
        let tool_capable =
            crate::modules::file::available_files::ensure_model_tools_capable(
                &mut context.metadata,
            )
            .await;
        let avail =
            crate::modules::file::available_files::available_files_from_metadata(&context.metadata);
        let manifest_available =
            crate::modules::file::available_files::files_manifest_available(&context.metadata);
        if tool_capable && manifest_available {
            let manifest = crate::modules::file::available_files::render_manifest(&avail);
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

        // Inline the CURRENT message's upload. Division of labor with the
        // history-replay path (process_content_for_llm), which processes EVERY
        // attachment block — including this message's:
        //   - tool-capable + manifest available: replay DROPS old (and this
        //     message's) NON-image attachments (recovered via the manifest +
        //     read_file) and KEEPS images inlined. So here we inline ONLY the
        //     current upload's NON-image files (the content the user is asking
        //     about now); images — current or old — are inlined by replay.
        //   - otherwise (non-tool-capable, or resolve failed): replay inlines
        //     every attachment (current + old), so we inline NOTHING here to
        //     avoid double-inlining the current upload.
        // ONLY on iteration 1: `before_llm_call` runs every tool-loop iteration,
        // but the "current upload" belongs to the first generation. On a
        // continuation (iteration >= 2) the last message is a Tool result (not
        // User), so re-inlining would push a stray User turn between the tool
        // round-trip and the model's continuation AND re-send the very bytes the
        // manifest exists to leave out. The model recovers them via read_file.
        if tool_capable && manifest_available && context.iteration == 1 {
            if let Some(file_ids) = &send_request.file_ids {
                if !file_ids.is_empty() {
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

                    let mut file_blocks = Vec::new();
                    for file_id in file_ids {
                        // Images are inlined by the replay path (vision); skip
                        // them here to avoid doubling. Classify by the file's OWN
                        // mime — NOT `avail` membership: content-dedup can fold
                        // this (newest) upload into an earlier same-checksum
                        // entry, so its id may be absent from `avail` even though
                        // it IS an image. (Ownership re-checked; a foreign/deleted
                        // id resolves to None and is skipped.)
                        let Some(file) =
                            Repos.file.get_by_id_and_user(*file_id, context.user_id).await?
                        else {
                            continue;
                        };
                        let is_image = file
                            .mime_type
                            .as_deref()
                            .map(|m| m.starts_with("image/"))
                            .unwrap_or(false);
                        if is_image {
                            continue;
                        }
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

                    // Append to the current User message if present; otherwise
                    // (e.g. an empty-text upload whose only block was the
                    // attachment, which the replay drop removed) push a fresh User
                    // turn so the current upload still lands exactly once. The
                    // System manifest sits at index 0, so order stays correct.
                    if let Some(last_message) = request.messages.last_mut()
                        && last_message.role == Role::User
                    {
                        last_message.content.extend(file_blocks);
                    } else if !file_blocks.is_empty() {
                        request.messages.push(ChatMessage {
                            role: Role::User,
                            content: file_blocks,
                        });
                    }
                }
            }
        }

        Ok(BeforeLlmAction::Continue)
    }

    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router
    }

    fn handled_content_types(&self) -> Vec<&'static str> {
        // The serde tags of the content variants this extension owns (NOT the
        // extension name "file"). Registry dispatch (process_content_for_llm /
        // should_skip_in_assistant_forwarding / process_content_from_db) is an
        // exact match on `MessageContentData::content_type()`, which for the
        // file ext's variants serializes to "file_attachment" / "image". With
        // the wrong name here those hooks were never dispatched for file content.
        vec!["file_attachment", "image"]
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
                // Recency rule (Track A §2): on a tool-capable model with a
                // resolved manifest, NON-image attachments are DROPPED from the
                // replayed history — they're listed in the auto-injected manifest
                // and read on demand via `read_file`, so re-inlining them every
                // turn was pure waste. The current-turn upload's attachment block
                // ALSO passes through here and is dropped; `before_llm_call`
                // re-inlines the current upload (so it lands exactly once). IMAGES
                // are kept inlined for vision continuity (current + old). The drop
                // is gated on `manifest_available` so that if file resolution
                // failed (no manifest), we fall back to inlining instead of
                // silently losing content; non-tool-capable models always inline.
                let is_image = mime_type
                    .as_deref()
                    .map(|m| m.starts_with("image/"))
                    .unwrap_or(false);
                let tool_capable =
                    crate::modules::file::available_files::model_supports_tools(&context.metadata)
                        .await;
                let manifest_available =
                    crate::modules::file::available_files::files_manifest_available(
                        &context.metadata,
                    );
                if tool_capable && manifest_available && !is_image {
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

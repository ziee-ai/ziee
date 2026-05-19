// MCP chat extension implementation

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use ai_providers::{ChatRequest, ContentBlock};

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};
use crate::modules::chat::core::models::{Message, MessageContentData};
use crate::modules::chat::core::types::streaming::ContentBlockDelta;
use crate::modules::mcp::client::manager::McpSessionManager;
use crate::modules::mcp::client::session::McpSession;
use crate::modules::mcp::UsageMode;
use crate::modules::mcp::sampling::{ChatSamplingHandler, acquire_session};
use crate::modules::mcp::elicitation::models::ElicitationStartedNotification;
use crate::core::repository::Repos;

use super::content::McpContentData;
use super::helpers;

/// Accumulated tool use data during streaming
#[derive(Debug, Clone, Default)]
struct AccumulatedToolUse {
    id: Option<String>,
    name: Option<String>,
    input_json: String, // Accumulated JSON string
}

/// MCP chat extension
///
/// Provides Model Context Protocol (MCP) tool calling functionality for chat.
pub struct McpChatExtension {
    pool: PgPool,
    config: Arc<crate::core::config::Config>,
    session_manager: Arc<McpSessionManager>,
    /// Storage for accumulating tool use deltas during streaming
    /// Key: (message_id, content_index)
    tool_use_accumulator: Arc<Mutex<HashMap<(Uuid, usize), AccumulatedToolUse>>>,
}

impl McpChatExtension {
    /// Create new MCP chat extension
    pub fn new(pool: PgPool, config: Arc<crate::core::config::Config>) -> Self {
        let session_manager = Arc::new(McpSessionManager::new(config.clone()));
        Self {
            pool,
            config,
            session_manager,
            tool_use_accumulator: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute approved tools and return (MessageContentData results, executed tool_use_ids)
    /// Used by both before_llm_call (no SSE) and after_llm_call (with SSE)
    async fn execute_approved_tools_sync(
        &self,
        approved_pending: &[super::approval::models::ToolUseApproval],
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(Vec<MessageContentData>, Vec<String>, Option<(String, Vec<super::content::Annotation>)>), AppError> {
        let mut tool_results = Vec::new();
        let mut executed_tool_use_ids = Vec::new();
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Channel for elicitation DB persistence (http.rs → mcp.rs via Repos)
        let (elicit_notify_tx, mut elicit_notify_rx) =
            tokio::sync::mpsc::unbounded_channel::<ElicitationStartedNotification>();
        tokio::spawn(async move {
            while let Some(notif) = elicit_notify_rx.recv().await {
                if let Some(msg_id) = notif.message_id {
                    let order = crate::core::Repos.chat.core
                        .get_message_with_content(msg_id).await
                        .map(|m| m.map(|msg| msg.contents.len() as i32).unwrap_or(0))
                        .unwrap_or(0);
                    let content_data = MessageContentData::ElicitationRequest {
                        elicitation_id: notif.elicitation_id.to_string(),
                        message: notif.message,
                        requested_schema: notif.requested_schema,
                        server: notif.server,
                        status: "pending".to_string(),
                        response_content: None,
                    };
                    let _ = crate::core::Repos.chat.core
                        .create_content_with_id(notif.content_id, msg_id, "elicitation_request", content_data, order)
                        .await;
                }
            }
        });

        for approval in approved_pending {
            let tool_use_id = approval.tool_use_id.clone();
            let tool_name = approval.tool_name.clone(); // Clean tool name (e.g., "fetch")
            let input = approval.tool_input.clone();

            // Use server_id from approval record (stored separately)
            let server_id = match approval.server_id {
                Some(id) => id,
                None => {
                    tracing::error!("No server_id in approval record for tool: {}", tool_name);
                    continue;
                }
            };

            // Find server by ID
            let server = accessible_servers.iter().find(|s| s.id == server_id);

            if server.is_none() {
                tracing::error!("Server not found for approved tool: {} (server_id={})", tool_name, server_id);
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: Some(tool_name.clone()),
                    server_id: Some(server_id.to_string()),
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                    annotations: None,
                    attachment: None,
                    resource_links: None,
                    hidden_content: None,
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event (if tx provided)
            if let Some(tx) = tx {
                helpers::send_tool_start_event(Some(tx), &tool_use_id, &tool_name, &server.name, &input).await;
            }

            // For sampling servers, create a fresh ephemeral session with the LLM handler.
            // Otherwise, use the shared pooled session (existing behaviour).
            let maybe_model_id = context.metadata.get("model_id")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok());

            let mut _owned: Option<McpSession> = None;
            let mut _guard: Option<tokio::sync::OwnedRwLockWriteGuard<McpSession>> = None;

            if server.supports_sampling {
                if let Some(model_id) = maybe_model_id {
                    match ChatSamplingHandler::new(model_id, context.user_id).await {
                        Ok(h) => {
                            let handler = Arc::new(h);
                            match McpSession::new_with_sampling(server.clone(), handler).await {
                                Ok(s) => _owned = Some(s),
                                Err(e) => {
                                    tracing::warn!(
                                        "[sampling] Failed to create sampling session for '{}': {}",
                                        server.name, e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "[sampling] Failed to init provider for '{}': {}",
                                server.name, e
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        "[sampling] server '{}' supports_sampling=true but no model_id in context metadata",
                        server.name
                    );
                }
            }

            if _owned.is_none() {
                if server.supports_sampling {
                    // Sampling server but no session could be created (no model_id or provider error).
                    // Fall back to the pooled session would deadlock with SSE-capable servers.
                    tracing::warn!(
                        "[sampling] server '{}' requires sampling but no session could be created; returning error",
                        server.name
                    );
                    let error_result = McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.to_string()),
                        server_id: Some(server.id.to_string()),
                        content: "Cannot execute sampling tool: no model available. Ensure a model is selected.".to_string(),
                        is_error: Some(true),
                        annotations: None,
                        attachment: None,
                        resource_links: None,
                        hidden_content: None,
                    };
                    tool_results.push(error_result.to_message_content());
                    continue;
                }
                let arc = match self.session_manager
                    .get_or_create_with_context(
                        server.id,
                        context.user_id,
                        Some(context.conversation_id),
                        context.message_id,
                    )
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get session for MCP server '{}': {}",
                            server.name, e
                        );
                        let err_result = McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: Some(server.id.to_string()),
                            content: format!("Failed to connect to server: {}", e),
                            is_error: Some(true),
                            annotations: None,
                            attachment: None,
                            resource_links: None,
                            hidden_content: None,
                        };
                        tool_results.push(err_result.to_message_content());
                        continue;
                    }
                };
                _guard = Some(arc.write_owned().await);
            }

            let session: &mut McpSession = if let Some(ref mut s) = _owned {
                s
            } else {
                _guard.as_deref_mut().unwrap()
            };


            // Execute tool with clean tool name
            let (mut result, is_final) = helpers::execute_tool(
                session,
                &tool_name,
                input,
                &server.name,
                Some(server.timeout_seconds),
                context.message_id,
                tx.cloned(),
                Some(elicit_notify_tx.clone()),
            )
            .await;

            // Set tool_use_id and server_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                server_id: ref mut sid,
                is_error,
                ref content,
                ..
            } = result
            {
                *id = tool_use_id.clone();
                *sid = Some(server.id.to_string());

                // Send tool complete event (if tx provided)
                if let Some(tx) = tx {
                    helpers::send_tool_complete_event(
                        Some(tx),
                        &tool_use_id,
                        &tool_name,
                        &server.name,
                        is_error.unwrap_or(false),
                        Some(content.as_str()),
                    )
                    .await;
                }
            }

            // Generic resource_link handling: fetch-and-save any resource_links returned by a tool.
            // Works uniformly for built-in servers (short-lived JWT auth) and external MCP servers
            // (server-configured headers). Runs the full processing pipeline (text extraction,
            // thumbnails) and creates a permanent DB artifact visible to the user.
            // Exception: is_saved=true links already exist in originals storage — skip all processing.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new(); // (artifact_id, display_name, download_url)
            let mut saved_file_urls: Vec<(String, String)> = Vec::new(); // (display_name, download_url) for is_saved links
            if let McpContentData::ToolResult { ref resource_links, is_error, .. } = result {
                if !is_error.unwrap_or(false) {
                    if let Some(links) = resource_links {
                        for link in links {

                        // is_saved=true: file already exists in originals storage.
                        // URI is a download-with-token URL — skip fetch/process/save pipeline.
                        if link.is_saved == Some(true) {
                            let name = link.name.as_deref().unwrap_or("file").to_string();
                            saved_file_urls.push((name, link.uri.clone()));
                            continue;
                        }

                        use crate::modules::file::models::FileCreateData;
                        use crate::modules::file::processing::ProcessingManager;
                        use crate::modules::file::storage::manager::get_file_storage;

                        // Build auth headers appropriate for the server type
                        let mut fetch_headers = reqwest::header::HeaderMap::new();
                        if server.is_built_in {
                            match McpSessionManager::generate_short_lived_jwt(
                                context.user_id, &self.config.jwt.secret, 10
                            ) {
                                Ok(token) => {
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &format!("Bearer {}", token)
                                    ) {
                                        fetch_headers.insert(reqwest::header::AUTHORIZATION, hval);
                                    }
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &context.conversation_id.to_string()
                                    ) {
                                        fetch_headers.insert(
                                            reqwest::header::HeaderName::from_static("x-conversation-id"),
                                            hval,
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to generate JWT for resource_link fetch: {}", e);
                                }
                            }
                        } else if let Some(headers_map) = server.headers.as_object() {
                            for (key, value) in headers_map.iter() {
                                if let Some(val_str) = value.as_str() {
                                    if let (Ok(hname), Ok(hval)) = (
                                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                                        reqwest::header::HeaderValue::from_str(val_str),
                                    ) {
                                        fetch_headers.insert(hname, hval);
                                    }
                                }
                            }
                        }

                        match reqwest::Client::builder()
                            .default_headers(fetch_headers)
                            .build()
                        {
                            Ok(client) => {
                                match client.get(&link.uri).send().await {
                                    Ok(response) if response.status().is_success() => {
                                        let content_type_mime = response
                                            .headers()
                                            .get(reqwest::header::CONTENT_TYPE)
                                            .and_then(|v| v.to_str().ok())
                                            .and_then(|s| s.split(';').next())
                                            .map(|s| s.trim().to_string());

                                        match response.bytes().await {
                                            Ok(bytes) => {
                                                let bytes = bytes.to_vec();
                                                let display_name =
                                                    link.name.as_deref().unwrap_or("file");
                                                let ext = std::path::Path::new(display_name)
                                                    .extension()
                                                    .and_then(|e| e.to_str())
                                                    .map(str::to_lowercase)
                                                    .unwrap_or_else(|| "bin".to_string());
                                                let mime_type = content_type_mime.or_else(|| {
                                                    mime_guess::from_ext(&ext)
                                                        .first()
                                                        .map(|m| m.to_string())
                                                });
                                                let mime_type_str = mime_type
                                                    .as_deref()
                                                    .unwrap_or("application/octet-stream");

                                                let processing_result = ProcessingManager::new()
                                                    .process_file(&bytes, mime_type_str)
                                                    .await
                                                    .unwrap_or_default();

                                                let artifact_id = Uuid::new_v4();
                                                let storage = get_file_storage();

                                                match storage
                                                    .save_original(
                                                        context.user_id,
                                                        artifact_id,
                                                        &ext,
                                                        &bytes,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        for (n, text) in processing_result
                                                            .text_pages
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_text_page(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    text,
                                                                )
                                                                .await;
                                                        }
                                                        if let Some(thumb) = processing_result
                                                            .thumbnails
                                                            .first()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    1,
                                                                    true,
                                                                    thumb,
                                                                )
                                                                .await;
                                                        }
                                                        for (n, img) in processing_result
                                                            .images
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    false,
                                                                    img,
                                                                )
                                                                .await;
                                                        }

                                                        let file_size = bytes.len() as i64;
                                                        match Repos
                                                            .file
                                                            .create(FileCreateData {
                                                                id: artifact_id,
                                                                user_id: context.user_id,
                                                                filename: display_name
                                                                    .to_string(),
                                                                file_size,
                                                                mime_type: mime_type.clone(),
                                                                checksum: None,
                                                                has_thumbnail:
                                                                    !processing_result
                                                                        .thumbnails
                                                                        .is_empty(),
                                                                preview_page_count:
                                                                    processing_result
                                                                        .images
                                                                        .len()
                                                                        as i32,
                                                                text_page_count:
                                                                    processing_result
                                                                        .text_pages
                                                                        .len()
                                                                        as i32,
                                                                processing_metadata:
                                                                    serde_json::to_value(
                                                                        &processing_result
                                                                            .metadata,
                                                                    )
                                                                    .unwrap_or_default(),
                                                                created_by: "mcp".to_string(),
                                                            })
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                helpers::send_artifact_created_event(
                                                                    tx,
                                                                    &artifact_id.to_string(),
                                                                    display_name,
                                                                    mime_type.as_deref(),
                                                                    file_size,
                                                                )
                                                                .await;

                                                                use crate::modules::chat::core::models::MessageContentData;
                                                                tool_results.push(
                                                                    MessageContentData::FileAttachment {
                                                                        file_id: artifact_id,
                                                                        filename: display_name
                                                                            .to_string(),
                                                                        mime_type,
                                                                        file_size,
                                                                    },
                                                                );

                                                                tracing::info!(
                                                                    "Artifact saved from resource_link: file_id={}, filename={}",
                                                                    artifact_id, display_name
                                                                );
                                                                let download_url = {
                                                                    use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                                                                    use crate::modules::file::types::DownloadTokenClaims;
                                                                    let now = chrono::Utc::now().timestamp() as usize;
                                                                    let claims = DownloadTokenClaims {
                                                                        file_id: artifact_id.to_string(),
                                                                        user_id: context.user_id.to_string(),
                                                                        exp: now + 3600,
                                                                        iat: now,
                                                                    };
                                                                    encode(
                                                                        &JwtHeader::default(),
                                                                        &claims,
                                                                        &EncodingKey::from_secret(self.config.jwt.secret.as_bytes()),
                                                                    )
                                                                    .ok()
                                                                    .map(|token| format!(
                                                                        "http://{}:{}{}/files/{}/download-with-token?token={}",
                                                                        self.config.server.host,
                                                                        self.config.server.port,
                                                                        self.config.server.api_prefix,
                                                                        artifact_id,
                                                                        token
                                                                    ))
                                                                };
                                                                saved_artifacts.push((artifact_id, display_name.to_string(), download_url));
                                                            }
                                                            Err(e) => {
                                                                tracing::error!(
                                                                    "Failed to create file DB record for resource_link: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Failed to save artifact original: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read resource_link response body: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(response) => {
                                        tracing::error!(
                                            "resource_link fetch returned HTTP {} for '{}': artifact NOT saved",
                                            response.status(),
                                            link.uri
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch resource_link '{}': {} — artifact NOT saved",
                                            link.uri, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build HTTP client for resource_link fetch: {}",
                                    e
                                );
                            }
                        }
                        } // end for link in links
                    }
                }
            }

            // Update tool result content with the saved artifact info so the LLM knows the file_ids.
            // Also set hidden_content with token-based download URLs — included in LLM messages
            // but stripped from browser API responses.
            // saved_file_urls holds download-with-token URLs for is_saved=true links (no pipeline needed).
            if !saved_artifacts.is_empty() || !saved_file_urls.is_empty() {
                if let McpContentData::ToolResult { ref mut content, ref mut hidden_content, .. } = result {
                    if !saved_artifacts.is_empty() {
                        let file_descriptions: Vec<String> = saved_artifacts
                            .iter()
                            .map(|(id, name, _)| format!("'{}' (file_id: {})", name, id))
                            .collect();
                        *content = format!(
                            "Files from MCP tool have been saved as artifact attachments: {}. \
                             They will be shown as file cards in the UI — do not embed them inline in your response.",
                            file_descriptions.join(", ")
                        );
                    }
                    let mut url_lines: Vec<String> = saved_artifacts
                        .iter()
                        .filter_map(|(_, name, url)| url.as_ref().map(|u| format!("{} - {}", name, u)))
                        .collect();
                    for (name, url) in &saved_file_urls {
                        url_lines.push(format!("{} - {}", name, url));
                    }
                    if !url_lines.is_empty() {
                        *hidden_content = Some(format!(
                            "[system: Files saved as artifact attachments (shown as file cards in UI). \
                             Do NOT embed file URLs or images inline in your text response. \
                             Internal download URLs for tool-to-tool access only:\n{}]",
                            url_lines.join("\n")
                        ));
                    }
                }
            }

            // Track executed tool_use_id
            executed_tool_use_ids.push(tool_use_id.clone());

            // Delete approval record after successful execution to prevent double-execution
            if let Err(e) = Repos
                .chat
                .mcp
                .delete_tool_approval(tool_use_id.clone(), approval.message_id)
                .await
            {
                tracing::error!(
                    "Failed to delete approval record for tool_use_id={}: {}. This may cause duplicate execution attempts.",
                    tool_use_id,
                    e
                );
            }

            // If this tool returns a final response, capture it and return early.
            // The caller will stream it directly without calling the LLM.
            if is_final {
                if let McpContentData::ToolResult { ref content, ref annotations, .. } = result {
                    tracing::info!(
                        "is_final_response: approved tool '{}' marked as final, will bypass LLM",
                        tool_name
                    );
                    let final_response = Some((content.clone(), annotations.clone().unwrap_or_default()));
                    // Push the tool_result BEFORE returning so the caller can persist it to DB.
                    // Without this, the tool_use in the assistant message would have no matching
                    // tool_result, causing "tool_use ids found without tool_result" on the next message.
                    tool_results.push(result.to_message_content());
                    return Ok((tool_results, executed_tool_use_ids, final_response));
                }
            }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());
        }

        Ok((tool_results, executed_tool_use_ids, None))
    }
}

#[async_trait]
impl ChatExtension for McpChatExtension {
    fn name(&self) -> &str {
        "mcp"
    }

    /// Don't create user message if we're resuming with tool approvals
    /// Tool approval resumption continues the existing conversation turn
    fn should_create_user_message(&self, request: &SendMessageRequest) -> bool {
        request.tool_approvals.is_none()
    }

    /// Provide existing assistant message when resuming with tool approvals
    /// Tool results append to the existing assistant message, not a new one
    async fn provide_assistant_message(
        &self,
        request: &SendMessageRequest,
        branch_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        // Only provide message if resuming with tool approvals
        if request.tool_approvals.is_some() {
            // Get last assistant message in branch
            let history = Repos.chat.core.get_conversation_history(branch_id).await?;

            // Find last assistant message
            let last_assistant = history.iter()
                .rev()
                .find(|msg| msg.message.role == "assistant");

            if let Some(msg) = last_assistant {
                return Ok(Some(msg.message.id));
            }
        }

        Ok(None)
    }

    /// Convert MCP content (ToolUse, ToolResult) to ContentBlock for LLM
    async fn process_content_for_llm(
        &self,
        content: &MessageContentData,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        // Try to convert MessageContentData to McpContentData
        if let Ok(mcp_content) = McpContentData::from_message_content(content) {
            // Convert to ContentBlock (handles both ToolUse and ToolResult)
            Ok(mcp_content.to_content_block())
        } else {
            Ok(None) // Not MCP content
        }
    }

    /// Register MCP approval workflow routes
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(super::approval::mcp_approval_router())
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // === STEP 1: Process tool approvals (if resuming after approval) ===
        if let Some(approvals) = &send_request.tool_approvals {
            tracing::info!(
                "Processing {} tool approval decisions for conversation {}, branch {}",
                approvals.len(),
                context.conversation_id,
                context.branch_id
            );

            // Log each approval decision for debugging
            for (idx, approval) in approvals.iter().enumerate() {
                tracing::info!(
                    "Approval[{}]: tool_use_id='{}', decision='{}', note={:?}",
                    idx,
                    approval.tool_use_id,
                    approval.decision,
                    approval.note
                );
            }

            // Process each approval decision
            for approval in approvals {
                tracing::info!("Processing approval decision: tool_use_id={}, decision={}, branch_id={}",
                    approval.tool_use_id, approval.decision, context.branch_id);
                match approval.decision.as_str() {
                    "approve" | "approved" => {
                        // Check what pending approvals exist for this branch
                        let pending = super::approval::repository::get_pending_approvals_for_branch(
                            &self.pool,
                            context.branch_id,
                        )
                        .await?;
                        tracing::info!(
                            "Pending approvals for branch {}: {:?}",
                            context.branch_id,
                            pending.iter().map(|p| (&p.tool_use_id, &p.status)).collect::<Vec<_>>()
                        );

                        // Check if this tool_use_id is still pending (idempotency check)
                        let is_pending = pending.iter().any(|p| p.tool_use_id == approval.tool_use_id);
                        if !is_pending {
                            tracing::info!(
                                "Approval for tool_use_id={} already processed (not in pending list), skipping",
                                approval.tool_use_id
                            );
                            continue;
                        }

                        // Approve the tool use
                        tracing::info!("Calling approve_tool_use for tool_use_id={}, branch_id={}",
                            approval.tool_use_id, context.branch_id);
                        match super::approval::repository::approve_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.branch_id,
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await {
                            Ok(approval_record) => {
                                tracing::info!("Successfully approved tool use: tool_use_id={}, status={}, branch_id={}, approval_id={}",
                                    approval.tool_use_id, approval_record.status, approval_record.branch_id, approval_record.id);
                            }
                            Err(e) => {
                                // Handle "not found" gracefully - might be a retry of an already-processed approval
                                if e.to_string().contains("not found") || e.to_string().contains("already processed") {
                                    tracing::warn!(
                                        "Approval for tool_use_id={} was already processed (concurrent request?), continuing",
                                        approval.tool_use_id
                                    );
                                    continue;
                                }
                                tracing::error!("Failed to approve tool use {}: {}", approval.tool_use_id, e);
                                return Err(e);
                            }
                        }
                    }
                    "deny" | "denied" => {
                        // Deny the tool use (with idempotency handling)
                        match super::approval::repository::deny_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.branch_id,
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await {
                            Ok(_) => {
                                tracing::info!("Denied tool use: {}", approval.tool_use_id);
                            }
                            Err(e) => {
                                // Handle "not found" gracefully - might be a retry of an already-processed denial
                                if e.to_string().contains("not found") || e.to_string().contains("already processed") {
                                    tracing::warn!(
                                        "Denial for tool_use_id={} was already processed (concurrent request?), continuing",
                                        approval.tool_use_id
                                    );
                                    continue;
                                }
                                tracing::error!("Failed to deny tool use {}: {}", approval.tool_use_id, e);
                                return Err(e);
                            }
                        }
                    }
                    _ => {
                        return Err(AppError::bad_request(
                            "INVALID_DECISION",
                            format!("Invalid decision: '{}'. Must be 'approve'/'approved' or 'deny'/'denied'", approval.decision),
                        ));
                    }
                }
            }

            // === STEP 1b: Check if all tools were denied ===
            // If all approvals were denied, skip LLM call and complete gracefully
            let all_denied = approvals.iter().all(|a|
                a.decision == "deny" || a.decision == "denied"
            );

            if all_denied {
                tracing::info!("All {} tool approvals were denied, skipping LLM call", approvals.len());

                // Optionally send an SSE event to inform the client
                if let Some(tx) = tx {
                    let _ = tx.send(Ok(Event::default()
                        .event("tool_denied")
                        .json_data(serde_json::json!({
                            "message": "Tool execution was denied by user",
                            "denied_count": approvals.len()
                        }))
                        .unwrap()));
                }

                return Ok(BeforeLlmAction::Complete);
            }

            // === STEP 1b.5: Guard — don't proceed if other tool_uses are still awaiting a decision ===
            // When the LLM requested multiple parallel tool calls that all need approval and the
            // user approves them one at a time, we must wait until every tool_use has been either
            // approved or denied before executing anything or calling the LLM.  Otherwise the LLM
            // request would contain tool_use blocks without matching tool_result blocks, causing
            // "tool_use ids found without tool_result" errors.
            let still_pending = super::approval::repository::get_pending_approvals_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            if !still_pending.is_empty() {
                tracing::info!(
                    "{} pending approval(s) still remain after processing {} decision(s); \
                     waiting for remaining approvals before executing tools or calling LLM",
                    still_pending.len(),
                    approvals.len()
                );
                return Ok(BeforeLlmAction::Complete);
            }

            // === STEP 1c: Execute approved tools immediately after approval ===
            let approved_pending = super::approval::repository::get_approved_tools_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            tracing::info!("before_llm_call: Found {} approved tools after processing approvals", approved_pending.len());

            // Collect all content blocks from both approved and denied tools so they can be
            // sent as a single User message.  Anthropic requires that every tool_use block in
            // the preceding assistant turn has a matching tool_result block in the next user
            // turn; mixing approved and denied results in one message satisfies that constraint.
            let mut content_blocks: Vec<ai_providers::ContentBlock> = Vec::new();

            if !approved_pending.is_empty() {
                // Execute approved tools and append results to request
                let (tool_results, executed_ids, final_response) = self.execute_approved_tools_sync(
                    &approved_pending,
                    context,
                    tx,
                ).await?;

                // Save tool results to the assistant message in database BEFORE any early returns.
                // This ensures tool_result blocks are persisted even when is_final_response: true
                // bypasses the normal Continue action. Without this, the tool_use block already in
                // the DB would have no matching tool_result, causing API errors on subsequent messages.
                if let Some(message_id) = context.message_id {
                    // Get current content count for sequence ordering
                    let current_count = match Repos.chat.core.get_message_with_content(message_id).await {
                        Ok(Some(msg)) => msg.contents.len() as i32,
                        _ => 0,
                    };

                    for (idx, result) in tool_results.iter().enumerate() {
                        let content_type = result.content_type();

                        if let Err(e) = Repos.chat.core.create_content(
                            message_id,
                            &content_type,
                            result.clone(),
                            current_count + idx as i32,
                        ).await {
                            tracing::error!("Failed to save tool result to message: {}", e);
                        } else {
                            tracing::info!("Saved tool_result to message {}, sequence {}", message_id, current_count + idx as i32);
                        }
                    }

                    // Cancel any elicitations that are still pending after tool execution ends
                    // (e.g., tool timed out while waiting for user input).
                    let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
                }

                // If any approved tool returned is_final_response: true, bypass LLM entirely.
                // tool_results are already saved to DB above.
                if let Some((text, annotations)) = final_response {
                    return Ok(BeforeLlmAction::CompleteWithContent {
                        text,
                        annotations,
                    });
                }

                // Store executed tool_use_ids in context metadata for later filtering
                if !executed_ids.is_empty() {
                    // Merge with any existing executed IDs
                    let mut all_executed: Vec<String> = context.metadata
                        .get("executed_tool_use_ids")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();
                    all_executed.extend(executed_ids.clone());
                    context.metadata.insert(
                        "executed_tool_use_ids".to_string(),
                        serde_json::to_value(&all_executed).unwrap_or_default(),
                    );
                    tracing::info!(
                        "Tracked {} executed tool_use_ids in context: {:?}",
                        executed_ids.len(),
                        executed_ids
                    );
                }

                // Convert approved tool results to content blocks
                for result in tool_results {
                    if let Some(block) = self.process_content_for_llm(&result, context).await? {
                        content_blocks.push(block);
                    }
                }
            }

            // === STEP 1d: Generate error tool_results for denied tools ===
            // Denied tools have no real result, but the LLM requires a tool_result for every
            // tool_use it emitted.  We create a synthetic error result so the message history
            // remains valid, then delete the denial record to prevent re-processing.
            let denied_tools = super::approval::repository::get_denied_tools_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            if !denied_tools.is_empty() {
                tracing::info!(
                    "before_llm_call: Creating error tool_results for {} denied tool(s)",
                    denied_tools.len()
                );

                if let Some(message_id) = context.message_id {
                    let current_count = match Repos.chat.core.get_message_with_content(message_id).await {
                        Ok(Some(msg)) => msg.contents.len() as i32,
                        _ => 0,
                    };

                    for (idx, denied) in denied_tools.iter().enumerate() {
                        let denied_result = McpContentData::ToolResult {
                            tool_use_id: denied.tool_use_id.clone(),
                            name: Some(denied.tool_name.clone()),
                            server_id: denied.server_id.map(|id| id.to_string()),
                            content: "Tool execution was denied by the user.".to_string(),
                            is_error: Some(true),
                            annotations: None,
                            attachment: None,
                            resource_links: None,
                            hidden_content: None,
                        };
                        let msg_content = denied_result.to_message_content();

                        // Persist denied result so the conversation history stays coherent
                        let content_type = msg_content.content_type();
                        if let Err(e) = Repos.chat.core.create_content(
                            message_id,
                            &content_type,
                            msg_content.clone(),
                            current_count + idx as i32,
                        ).await {
                            tracing::error!(
                                "Failed to save denied tool_result for tool_use_id={}: {}",
                                denied.tool_use_id, e
                            );
                        } else {
                            tracing::info!(
                                "Saved denied tool_result for tool_use_id={} to message {}",
                                denied.tool_use_id, message_id
                            );
                        }

                        // Convert for LLM request
                        if let Some(block) = self.process_content_for_llm(&msg_content, context).await? {
                            content_blocks.push(block);
                        }

                        // Delete the denial record so it isn't processed again on future resumptions
                        if let Err(e) = Repos.chat.mcp
                            .delete_tool_approval(denied.tool_use_id.clone(), denied.message_id)
                            .await
                        {
                            tracing::error!(
                                "Failed to delete denial record for tool_use_id={}: {}",
                                denied.tool_use_id, e
                            );
                        }
                    }
                }
            }

            // Append all tool results (approved + denied) as a single user message
            if !content_blocks.is_empty() {
                use ai_providers::{ChatMessage, Role};
                let count = content_blocks.len();
                request.messages.push(ChatMessage {
                    role: Role::User,
                    content: content_blocks,
                });
                tracing::info!("Appended {} tool result(s) to request (approved + denied)", count);
            }
        } else {
            // No tool_approvals provided - check if there are pending approvals to cancel
            let pending_count = super::approval::repository::get_pending_approvals_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?
            .len();

            if pending_count > 0 {
                tracing::info!(
                    "Cancelling {} pending approvals for branch {} (new message without approval)",
                    pending_count,
                    context.branch_id
                );
                super::approval::repository::cancel_pending_approvals_for_branch(
                    &self.pool,
                    context.branch_id,
                )
                .await?;
            }
        }

        // === STEP 2: Check if MCP is enabled ===
        if !send_request.enable_mcp {
            tracing::debug!("MCP not enabled for this request");
            return Ok(BeforeLlmAction::Continue);
        }

        // Get mcp_servers from config
        let mcp_servers = send_request.mcp_config
            .as_ref()
            .map(|config| config.mcp_servers.clone());

        tracing::info!(
            "MCP extension: enabled for user {}, servers requested: {}",
            context.user_id,
            mcp_servers.as_ref().map(|s| s.len()).unwrap_or(0)
        );

        // Validate and build server configuration
        let (server_configs, accessible_ids) =
            helpers::validate_and_build_config(&self.pool, context.user_id, mcp_servers).await?;

        if server_configs.is_empty() {
            tracing::debug!(
                "User {} can access 0 MCP servers (out of {} accessible)",
                context.user_id,
                accessible_ids.len()
            );
            return Ok(BeforeLlmAction::Continue);
        }

        // Get all accessible servers with details
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Extract user's raw message text (used for "always"-mode preprocessing)
        let user_message_text: Option<String> = request.messages.iter().rev()
            .find(|m| m.role == ai_providers::Role::User)
            .and_then(|m| m.content.iter().find_map(|block| {
                if let ai_providers::ContentBlock::Text { text } = block {
                    Some(text.clone())
                } else {
                    None
                }
            }));

        // Collect tools from all configured servers
        let mut all_tools = Vec::new();
        let mut always_mode_context: Vec<String> = Vec::new();

        for (server_id, requested_tools) in &server_configs {
            // Find server details
            let server = accessible_servers
                .iter()
                .find(|s| s.id == *server_id)
                .ok_or_else(|| AppError::internal_error("Server not found in accessible list"))?;

            if server.usage_mode == UsageMode::Always {
                // Always mode: pre-run tools with user's message and inject enriched context
                if let Some(ref query_text) = user_message_text {
                    let maybe_model_id = context.metadata.get("model_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| uuid::Uuid::parse_str(s).ok());

                    // Create session (with sampling if supported)
                    let session_result = if server.supports_sampling {
                        if let Some(model_id) = maybe_model_id {
                            match ChatSamplingHandler::new(model_id, context.user_id).await {
                                Ok(h) => McpSession::new_with_sampling(server.clone(), Arc::new(h)).await,
                                Err(e) => {
                                    tracing::warn!("Always-mode: failed to init sampling provider for {}: {}", server.name, e);
                                    McpSession::new(server.clone()).await
                                }
                            }
                        } else {
                            McpSession::new(server.clone()).await
                        }
                    } else {
                        McpSession::new(server.clone()).await
                    };

                    match session_result {
                        Err(e) => {
                            tracing::warn!("Always-mode: failed to connect to server {}: {}", server.name, e);
                        }
                        Ok(mut session) => {
                            let mcp_tools = match session.list_tools().await {
                                Ok(t) => t,
                                Err(e) => {
                                    tracing::warn!("Always-mode: failed to list tools from {}: {}", server.name, e);
                                    Vec::new()
                                }
                            };

                            let tools_to_run: Vec<_> = if requested_tools.is_empty() {
                                mcp_tools
                            } else {
                                mcp_tools.into_iter().filter(|t| requested_tools.contains(&t.name)).collect()
                            };

                            for tool in &tools_to_run {
                                // build_query_input returns None when the schema has required
                                // non-string params — skip auto-execution rather than submitting
                                // wrong inputs silently.
                                let input = match helpers::build_query_input(&tool.input_schema, query_text) {
                                    Some(v) => v,
                                    None => {
                                        tracing::debug!(
                                            "[mcp] Skipping always-mode tool '{}': schema has required non-string params",
                                            tool.name
                                        );
                                        continue;
                                    }
                                };
                                match session.call_tool(&tool.name, input, context.message_id, None, None).await {
                                    Ok(result) => {
                                        // Collect text content from tool result
                                        let text_parts: Vec<String> = result.content.iter()
                                            .filter_map(|c| c.content.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
                                            .collect();
                                        if !text_parts.is_empty() {
                                            always_mode_context.push(format!(
                                                "[{}] {}:\n{}",
                                                server.name,
                                                tool.name,
                                                text_parts.join("\n")
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("Always-mode: tool {} on {} failed: {}", tool.name, server.name, e);
                                    }
                                }
                            }
                        }
                    }
                }
                continue; // Don't add "always" server tools to the LLM tool list
            }

            // Auto mode: Get or create MCP session and collect tools for LLM
            let session_arc = match self.session_manager
                .get_or_create_with_context(
                    *server_id,
                    context.user_id,
                    Some(context.conversation_id),
                    context.message_id,
                )
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to MCP server '{}': {} — skipping",
                        server.name, e
                    );
                    continue;
                }
            };
            let mut session = session_arc.write().await;

            // List tools from server
            let mcp_tools = match session.list_tools().await {
                Ok(tools) => tools,
                Err(e) => {
                    tracing::warn!(
                        "Failed to list tools from server {}: {}",
                        server.name,
                        e
                    );
                    continue; // Skip this server
                }
            };

            // Filter tools if specific tools requested
            let tools_to_add = if requested_tools.is_empty() {
                // Empty array = all tools
                mcp_tools
            } else {
                // Filter to requested tools only
                mcp_tools
                    .into_iter()
                    .filter(|t| requested_tools.contains(&t.name))
                    .collect()
            };

            // Convert and add tools (using server_id for unique tool naming)
            for mcp_tool in tools_to_add {
                let ai_tool = helpers::convert_mcp_tool_to_ai_tool(server.id, &mcp_tool);
                all_tools.push(ai_tool);
            }
        }

        // Inject always-mode context into the system message
        if !always_mode_context.is_empty() {
            let context_block = format!(
                "\n\n--- Pre-fetched context ---\n{}\n--- End context ---",
                always_mode_context.join("\n\n")
            );
            // Append to existing system message or prepend a new one
            if let Some(sys_msg) = request.messages.iter_mut().find(|m| m.role == ai_providers::Role::System) {
                if let Some(ai_providers::ContentBlock::Text { text }) = sys_msg.content.first_mut() {
                    text.push_str(&context_block);
                }
            } else {
                request.messages.insert(0, ai_providers::ChatMessage {
                    role: ai_providers::Role::System,
                    content: vec![ai_providers::ContentBlock::Text { text: context_block }],
                });
            }
            tracing::info!("Injected {} always-mode context blocks into system message", always_mode_context.len());
        }

        tracing::info!(
            "MCP extension: added {} tools from {} servers",
            all_tools.len(),
            server_configs.len()
        );

        // DEBUG: Log each tool being added
        for (i, tool) in all_tools.iter().enumerate() {
            tracing::info!(
                "Tool {}: name='{}', description='{}', schema={}",
                i,
                tool.function.name,
                tool.function.description.as_ref().unwrap_or(&"".to_string()),
                serde_json::to_string(&tool.function.parameters).unwrap_or_default()
            );
        }

        // Add tools to ChatRequest
        if !all_tools.is_empty() {
            tracing::info!("Adding {} tools to ChatRequest", all_tools.len());
            request.tools = all_tools;

            // On the first iteration, nudge the model to prefer tools over training knowledge.
            // This is a soft hint — the model can still answer directly if no tool is relevant.
            // Only injected on iteration 1 to avoid redundancy in follow-up tool-calling loops.
            if context.iteration == 1 {
                let mut system_addition = String::from("\n\nYou have access to tools that can retrieve up-to-date or domain-specific information. When answering questions, prefer using these tools over relying solely on your training knowledge, especially when the tools are clearly relevant to the request.");

                if let Some(sys_msg) = request.messages.iter_mut().find(|m| m.role == ai_providers::Role::System) {
                    if let Some(ai_providers::ContentBlock::Text { text }) = sys_msg.content.first_mut() {
                        text.push_str(&system_addition);
                    }
                } else {
                    request.messages.insert(0, ai_providers::ChatMessage {
                        role: ai_providers::Role::System,
                        content: vec![ai_providers::ContentBlock::Text { text: system_addition }],
                    });
                }
            }
        } else {
            tracing::warn!("No tools to add to ChatRequest!");
        }

        Ok(BeforeLlmAction::Continue)
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        tracing::info!(
            "MCP after_llm_call: message_id={}, conversation_id={}, iteration={}",
            final_message.id,
            context.conversation_id,
            context.iteration
        );

        // === STEP 0: Check loop settings ===
        // Get loop settings from conversation MCP settings (or use defaults)
        let loop_settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?
            .map(|s| s.get_loop_settings())
            .unwrap_or_default();

        tracing::info!(
            "Loop settings: max_iteration={}, stop_when_no_tool_calling={}, stop_when_tools_called={}",
            loop_settings.max_iteration,
            loop_settings.stop_when_no_tool_calling,
            loop_settings.stop_when_tools_called.len()
        );

        // Check max_iteration (0 = unlimited)
        if loop_settings.max_iteration > 0 && context.iteration >= loop_settings.max_iteration {
            tracing::info!(
                "Max iteration limit reached: iteration={} >= max_iteration={}",
                context.iteration,
                loop_settings.max_iteration
            );
            // finalize() already wrote tool_use blocks for the current LLM response.
            // Create synthetic error tool_results for every unexecuted tool_use so the
            // DB invariant (each TU has a matching TR) is maintained. Without this,
            // the next user message would trigger an Anthropic "tool_use without tool_result" error.
            if let Some(message_id) = context.message_id {
                if let Ok(Some(msg)) = Repos.chat.core.get_message_with_content(message_id).await {
                    let executed_ids: std::collections::HashSet<String> = msg.contents.iter()
                        .filter_map(|c| c.parse_content().ok())
                        .filter_map(|cd| McpContentData::from_message_content(&cd).ok())
                        .filter_map(|mcd| match mcd {
                            McpContentData::ToolResult { tool_use_id, .. } => Some(tool_use_id),
                            _ => None,
                        })
                        .collect();
                    let pending_tool_uses: Vec<(String, String)> = msg.contents.iter()
                        .filter_map(|c| c.parse_content().ok())
                        .filter_map(|cd| McpContentData::from_message_content(&cd).ok())
                        .filter_map(|mcd| match mcd {
                            McpContentData::ToolUse { id, name, .. }
                                if !executed_ids.contains(&id) => Some((id, name)),
                            _ => None,
                        })
                        .collect();
                    let current_count = msg.contents.len() as i32;
                    for (idx, (tool_use_id, tool_name)) in pending_tool_uses.iter().enumerate() {
                        let error_result = McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: None,
                            content: "Tool execution stopped: maximum iteration limit reached."
                                .to_string(),
                            is_error: Some(true),
                            annotations: None,
                            attachment: None,
                            resource_links: None,
                            hidden_content: None,
                        };
                        let msg_content = error_result.to_message_content();
                        if let Err(e) = Repos.chat.core.create_content(
                            message_id,
                            &msg_content.content_type(),
                            msg_content,
                            current_count + idx as i32,
                        ).await {
                            tracing::error!(
                                "Failed to save synthetic tool_result for tool_use_id={}: {}",
                                tool_use_id, e
                            );
                        }
                    }
                }
            }
            return Ok(ExtensionAction::Complete);
        }

        // === STEP 1: Check for approved pending tools (from previous approval) ===
        tracing::info!("after_llm_call: Checking for approved tools on branch {}", context.branch_id);
        let approved_pending = super::approval::repository::get_approved_tools_for_branch(
            &self.pool,
            context.branch_id,
        )
        .await?;

        tracing::info!("after_llm_call: Found {} approved tools", approved_pending.len());

        if !approved_pending.is_empty() {
            tracing::info!(
                "Found {} approved pending tools to execute in after_llm_call",
                approved_pending.len()
            );

            // Execute approved tools using shared helper
            tracing::info!("after_llm_call: Executing approved tools with tx={}", tx.is_some());
            let (tool_results, executed_ids, final_response) = self.execute_approved_tools_sync(
                &approved_pending,
                context,
                tx,
            ).await?;
            tracing::info!(
                "after_llm_call: Executed {} tools successfully, tool_use_ids: {:?}",
                tool_results.len(),
                executed_ids
            );

            // Cancel any elicitations that are still pending after tool execution ends.
            if let Some(message_id) = context.message_id {
                let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
            }

            // If any approved tool returned is_final_response: true, bypass the next LLM call.
            if let Some((text, annotations)) = final_response {
                return Ok(ExtensionAction::CompleteWithContent {
                    text,
                    annotations,
                });
            }

            // Return Continue action to append tool results to assistant message
            tracing::info!("Returning {} approved tool results to append to assistant message", tool_results.len());
            return Ok(ExtensionAction::Continue {
                assistant_message_content: tool_results,
            });
        }

        // === STEP 2: Load message contents and find new ToolUse blocks ===
        let message_with_content = Repos
            .chat
            .core
            .get_message_with_content(final_message.id)
            .await?
            .ok_or_else(|| AppError::internal_error("Message not found after finalization"))?;

        tracing::info!(
            "Message {} has {} content blocks",
            final_message.id,
            message_with_content.contents.len()
        );

        // Find ToolUse and ToolResult content blocks
        let mut tool_uses = Vec::new();
        let mut executed_tool_use_ids = std::collections::HashSet::new();

        // First pass: collect tool_result tool_use_ids from context metadata (executed in before_llm_call)
        if let Some(context_executed) = context.metadata.get("executed_tool_use_ids") {
            if let Ok(ids) = serde_json::from_value::<Vec<String>>(context_executed.clone()) {
                tracing::info!("Found {} executed tool_use_ids in context metadata: {:?}", ids.len(), ids);
                executed_tool_use_ids.extend(ids);
            }
        }

        // Also collect from tool_result blocks in the message (for redundancy/safety)
        for content in &message_with_content.contents {
            let content_data = content.parse_content()?;
            if let Ok(mcp_content) = McpContentData::from_message_content(&content_data) {
                if let McpContentData::ToolResult { tool_use_id, .. } = mcp_content {
                    executed_tool_use_ids.insert(tool_use_id);
                }
            }
        }

        tracing::info!(
            "Total executed tool_use_ids (from context + message): {}",
            executed_tool_use_ids.len()
        );

        // Second pass: collect tool_uses that haven't been executed yet
        for content in &message_with_content.contents {
            tracing::info!(
                "  Content block: type='{}', sequence={}",
                content.content_type,
                content.sequence_order
            );

            let content_data = content.parse_content()?;

            // Try to parse as MCP Extension content
            if let Ok(mcp_content) = McpContentData::from_message_content(&content_data) {
                tracing::info!("    Parsed as MCP content: {:?}", match &mcp_content {
                    McpContentData::ToolUse { name, .. } => format!("ToolUse({})", name),
                    McpContentData::ToolResult { name, .. } => format!("ToolResult({:?})", name),
                });

                if let McpContentData::ToolUse { id, name, server_id, input } = mcp_content {
                    // Skip tool_uses that already have a tool_result (already executed)
                    if executed_tool_use_ids.contains(&id) {
                        tracing::info!("    Skipping tool_use {} - already has result", id);
                        continue;
                    }
                    // Store server_id and name separately
                    tool_uses.push((id, name, server_id, input));
                }
            }
        }

        tracing::info!(
            "Extracted {} tool uses from message {} ({} already executed)",
            tool_uses.len(),
            final_message.id,
            executed_tool_use_ids.len()
        );

        if tool_uses.is_empty() {
            // No tool uses - check stop_when_no_tool_calling setting
            if loop_settings.stop_when_no_tool_calling {
                tracing::info!("No tool uses found and stop_when_no_tool_calling=true, conversation complete");
                return Ok(ExtensionAction::Complete);
            } else {
                tracing::info!("No tool uses found but stop_when_no_tool_calling=false, continuing anyway");
                // Continue with empty results (LLM will generate next response)
                return Ok(ExtensionAction::Continue {
                    assistant_message_content: Vec::new(),
                });
            }
        }

        // Check MCP approval settings for this conversation
        let settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?;

        let (approval_mode, auto_approved_servers) = if let Some(ref settings) = settings {
            // Parse auto_approved_tools as Vec<AutoApprovedServer>
            let servers: Vec<super::approval::models::AutoApprovedServer> =
                serde_json::from_value(settings.auto_approved_tools.clone()).unwrap_or_default();
            (settings.get_approval_mode(), servers)
        } else {
            // No settings = default to manual approve with no auto-approved tools
            (crate::modules::chat::extensions::mcp::ApprovalMode::ManualApprove, Vec::new())
        };

        tracing::info!(
            "MCP extension: {} tools, approval_mode={}, auto_approved_servers={}",
            tool_uses.len(),
            approval_mode,
            auto_approved_servers.len()
        );

        // Check approval mode
        if matches!(approval_mode, crate::modules::chat::extensions::mcp::ApprovalMode::Disabled) {
            tracing::info!("MCP disabled for conversation {}", context.conversation_id);
            return Ok(ExtensionAction::Complete);
        }

        // Get accessible servers for lookups
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Load user defaults once for fallback auto-approval check (e.g. built-in servers)
        let user_auto_approved: Vec<super::approval::models::AutoApprovedServer> = {
            use crate::modules::chat::extensions::mcp::defaults::repository as defaults_repo;
            defaults_repo::get_user_defaults(&self.pool, context.user_id)
                .await
                .ok()
                .flatten()
                .map(|d| d.get_auto_approved_tools())
                .unwrap_or_default()
        };

        // Determine which tools need approval vs can execute immediately
        let mut tools_to_execute = Vec::new();
        let mut tools_needing_approval = Vec::new();

        for (tool_use_id, tool_name, server_id, input) in tool_uses {
            let needs_approval = match approval_mode {
                crate::modules::chat::extensions::mcp::ApprovalMode::AutoApprove => false,
                crate::modules::chat::extensions::mcp::ApprovalMode::ManualApprove => {
                    // Check if this tool is auto-approved using server_id directly
                    let is_auto_approved = if let Ok(sid) = uuid::Uuid::parse_str(&server_id) {
                        auto_approved_servers
                            .iter()
                            .any(|s| s.server_id == sid && s.contains_tool(&tool_name))
                        || user_auto_approved
                            .iter()
                            .any(|s| s.server_id == sid && s.contains_tool(&tool_name))
                    } else {
                        false
                    };
                    tracing::info!(
                        "Tool '{}' (server={}) auto-approved check: is_auto_approved={}",
                        tool_name,
                        server_id,
                        is_auto_approved
                    );
                    !is_auto_approved
                }
                crate::modules::chat::extensions::mcp::ApprovalMode::Disabled => {
                    unreachable!("Already handled above")
                }
            };

            tracing::info!(
                "Tool '{}' (server={}, id={}): needs_approval={}",
                tool_name,
                server_id,
                tool_use_id,
                needs_approval
            );

            if needs_approval {
                tools_needing_approval.push((tool_use_id, tool_name.clone(), server_id.clone(), input));
            } else {
                tools_to_execute.push((tool_use_id, tool_name, server_id, input));
            }
        }

        // Create pending approval records for tools that need manual approval
        if !tools_needing_approval.is_empty() {
            tracing::info!(
                "Creating {} pending approval records",
                tools_needing_approval.len()
            );

            for (tool_use_id, tool_name, server_id_str, input) in &tools_needing_approval {
                // Parse UUID and lookup server name
                let (server_id, server_name) = if let Ok(id) = uuid::Uuid::parse_str(server_id_str) {
                    let name = accessible_servers
                        .iter()
                        .find(|s| s.id == id)
                        .map(|s| s.name.clone())
                        .unwrap_or_else(|| id.to_string());
                    (Some(id), name)
                } else {
                    (None, server_id_str.to_string())
                };

                // Create pending approval with server_id and server_name
                tracing::info!(
                    "Creating approval record: tool_use_id={}, branch_id={}, message_id={}, tool_name={}",
                    tool_use_id, context.branch_id, final_message.id, tool_name
                );

                let approval_record = crate::core::Repos
                    .chat
                    .mcp
                    .create_tool_approval(
                        context.conversation_id,
                        context.branch_id,
                        final_message.id,
                        context.user_id,
                        tool_use_id.clone(),
                        tool_name.clone(),
                        input.clone(),
                        server_id,
                        server_name.clone(),
                    )
                    .await?;

                tracing::info!(
                    "Created approval record: id={}, tool_use_id={}, branch_id={}, status={}",
                    approval_record.id, approval_record.tool_use_id, approval_record.branch_id, approval_record.status
                );

                // Send SSE event for approval required
                helpers::send_approval_required_event(tx, tool_use_id, tool_name, &server_name, server_id_str, input).await?;
            }

            // Return Complete to pause conversation - user must approve via API or tool_approvals field
            tracing::info!("Conversation paused, waiting for {} tool approvals", tools_needing_approval.len());
            return Ok(ExtensionAction::Complete);
        }

        tracing::info!("MCP extension: executing {} auto-approved tools", tools_to_execute.len());

        // accessible_servers already available from above

        // Execute each auto-approved tool and collect results
        let mut tool_results = Vec::new();
        let mut final_response_text: Option<String> = None;
        let mut final_annotations: Vec<super::content::Annotation> = Vec::new();

        // Channel for elicitation DB persistence (http.rs → mcp.rs via Repos)
        let (elicit_notify_tx, mut elicit_notify_rx) =
            tokio::sync::mpsc::unbounded_channel::<ElicitationStartedNotification>();
        tokio::spawn(async move {
            while let Some(notif) = elicit_notify_rx.recv().await {
                if let Some(msg_id) = notif.message_id {
                    let order = crate::core::Repos.chat.core
                        .get_message_with_content(msg_id).await
                        .map(|m| m.map(|msg| msg.contents.len() as i32).unwrap_or(0))
                        .unwrap_or(0);
                    let content_data = MessageContentData::ElicitationRequest {
                        elicitation_id: notif.elicitation_id.to_string(),
                        message: notif.message,
                        requested_schema: notif.requested_schema,
                        server: notif.server,
                        status: "pending".to_string(),
                        response_content: None,
                    };
                    let _ = crate::core::Repos.chat.core
                        .create_content_with_id(notif.content_id, msg_id, "elicitation_request", content_data, order)
                        .await;
                }
            }
        });

        for (tool_use_id, tool_name, server_id_str, input) in tools_to_execute {
            // Parse UUID
            let server_id = match uuid::Uuid::parse_str(&server_id_str) {
                Ok(id) => id,
                Err(_) => {
                    tracing::error!("Invalid server_id: {}", server_id_str);
                    continue;
                }
            };

            // Find server by ID
            let server = accessible_servers
                .iter()
                .find(|s| s.id == server_id);

            if server.is_none() {
                tracing::error!("Server not found for tool: {}", tool_name);
                // Create error result
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: Some(tool_name.clone()),
                    server_id: Some(server_id_str.clone()),
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                    annotations: None,
                    attachment: None,
                    resource_links: None,
                    hidden_content: None,
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event
            helpers::send_tool_start_event(tx, &tool_use_id, &tool_name, &server.name, &input).await;

            let (mut result, is_final) = if server.supports_sampling {
                // Sampling path: create a fresh session with the sampling handler (bypass pool)
                let model_id_opt = context.metadata.get("model_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());

                if let Some(model_id) = model_id_opt {
                    // Acquire session guard (enforces max_concurrent_sessions if set)
                    match acquire_session(server.id, server.max_concurrent_sessions) {
                        Err(e) => {
                            tracing::warn!("Sampling session limit reached for server {}: {}", server.name, e);
                            (McpContentData::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                name: Some(tool_name.clone()),
                                server_id: Some(server.id.to_string()),
                                content: e.to_string(),
                                is_error: Some(true),
                                annotations: None,
                                attachment: None,
                                resource_links: None,
                                hidden_content: None,
                            }, false)
                        }
                        Ok(_guard) => {
                            // _guard keeps the session counter incremented until end of block
                            match ChatSamplingHandler::new(model_id, context.user_id).await {
                                Err(e) => {
                                    tracing::warn!("[sampling] Failed to init provider for '{}': {}", server.name, e);
                                    (McpContentData::ToolResult {
                                        tool_use_id: tool_use_id.clone(),
                                        name: Some(tool_name.clone()),
                                        server_id: Some(server.id.to_string()),
                                        content: format!("Failed to initialize sampling provider: {}", e),
                                        is_error: Some(true),
                                        annotations: None,
                                        attachment: None,
                                        resource_links: None,
                                        hidden_content: None,
                                    }, false)
                                }
                                Ok(h) => {
                                    match McpSession::new_with_sampling(server.clone(), Arc::new(h)).await {
                                        Ok(mut sampling_session) => {
                                            helpers::execute_tool(
                                                &mut sampling_session,
                                                &tool_name,
                                                input,
                                                &server.name,
                                                Some(server.timeout_seconds),
                                                context.message_id,
                                                tx.cloned(),
                                                Some(elicit_notify_tx.clone()),
                                            )
                                            .await
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to create sampling session for {}: {}", server.name, e);
                                            (McpContentData::ToolResult {
                                                tool_use_id: tool_use_id.clone(),
                                                name: Some(tool_name.clone()),
                                                server_id: Some(server.id.to_string()),
                                                content: format!("Failed to connect to server: {}", e),
                                                is_error: Some(true),
                                                annotations: None,
                                                attachment: None,
                                                resource_links: None,
                                                hidden_content: None,
                                            }, false)
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    tracing::warn!(
                        "[sampling] Server '{}' has supports_sampling=true but no model_id in context; cannot execute sampling tool",
                        server.name
                    );
                    (McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        name: Some(tool_name.clone()),
                        server_id: Some(server.id.to_string()),
                        content: "Cannot execute sampling tool: no model available in context. Ensure a model is selected.".to_string(),
                        is_error: Some(true),
                        annotations: None,
                        attachment: None,
                        resource_links: None,
                        hidden_content: None,
                    }, false)
                }
            } else {
                // Non-sampling path: use session manager (creates ephemeral session with context
                // headers for built-in servers; ephemeral non-pooled session for external servers)
                match self.session_manager
                    .get_or_create_with_context(
                        server.id,
                        context.user_id,
                        Some(context.conversation_id),
                        context.message_id,
                    )
                    .await
                {
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get session for MCP server '{}': {}",
                            server.name, e
                        );
                        (McpContentData::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            name: Some(tool_name.clone()),
                            server_id: Some(server.id.to_string()),
                            content: format!("Failed to connect to server: {}", e),
                            is_error: Some(true),
                            annotations: None,
                            attachment: None,
                            resource_links: None,
                            hidden_content: None,
                        }, false)
                    }
                    Ok(session_arc) => {
                        let mut session = session_arc.write().await;
                        helpers::execute_tool(&mut session, &tool_name, input, &server.name, Some(server.timeout_seconds), context.message_id, tx.cloned(), Some(elicit_notify_tx.clone())).await
                    }
                }
            };

            // Set tool_use_id and server_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                server_id: ref mut sid,
                is_error,
                ref content,
                ..
            } = result
            {
                *id = tool_use_id.clone();
                *sid = Some(server.id.to_string());

                // Send tool complete event
                helpers::send_tool_complete_event(
                    tx,
                    &tool_use_id,
                    &tool_name,
                    &server.name,
                    is_error.unwrap_or(false),
                    Some(content.as_str()),
                )
                .await;
            }

            // Generic resource_link handling: fetch-and-save any resource_links returned by a tool.
            // Works uniformly for built-in servers (short-lived JWT auth) and external MCP servers
            // (server-configured headers). Runs the full processing pipeline (text extraction,
            // thumbnails) and creates a permanent DB artifact visible to the user.
            // Exception: is_saved=true links already exist in originals storage — skip all processing.
            let mut saved_artifacts: Vec<(Uuid, String, Option<String>)> = Vec::new(); // (artifact_id, display_name, download_url)
            let mut saved_file_urls: Vec<(String, String)> = Vec::new(); // (display_name, download_url) for is_saved links
            if let McpContentData::ToolResult { ref resource_links, is_error, .. } = result {
                if !is_error.unwrap_or(false) {
                    if let Some(links) = resource_links {
                        for link in links {

                        // is_saved=true: file already exists in originals storage.
                        // URI is a download-with-token URL — skip fetch/process/save pipeline.
                        if link.is_saved == Some(true) {
                            let name = link.name.as_deref().unwrap_or("file").to_string();
                            saved_file_urls.push((name, link.uri.clone()));
                            continue;
                        }

                        use crate::modules::file::models::FileCreateData;
                        use crate::modules::file::processing::ProcessingManager;
                        use crate::modules::file::storage::manager::get_file_storage;

                        // Build auth headers appropriate for the server type
                        let mut fetch_headers = reqwest::header::HeaderMap::new();
                        if server.is_built_in {
                            match McpSessionManager::generate_short_lived_jwt(
                                context.user_id, &self.config.jwt.secret, 10
                            ) {
                                Ok(token) => {
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &format!("Bearer {}", token)
                                    ) {
                                        fetch_headers.insert(reqwest::header::AUTHORIZATION, hval);
                                    }
                                    if let Ok(hval) = reqwest::header::HeaderValue::from_str(
                                        &context.conversation_id.to_string()
                                    ) {
                                        fetch_headers.insert(
                                            reqwest::header::HeaderName::from_static("x-conversation-id"),
                                            hval,
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to generate JWT for resource_link fetch: {}", e);
                                }
                            }
                        } else if let Some(headers_map) = server.headers.as_object() {
                            for (key, value) in headers_map.iter() {
                                if let Some(val_str) = value.as_str() {
                                    if let (Ok(hname), Ok(hval)) = (
                                        reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                                        reqwest::header::HeaderValue::from_str(val_str),
                                    ) {
                                        fetch_headers.insert(hname, hval);
                                    }
                                }
                            }
                        }

                        match reqwest::Client::builder()
                            .default_headers(fetch_headers)
                            .build()
                        {
                            Ok(client) => {
                                match client.get(&link.uri).send().await {
                                    Ok(response) if response.status().is_success() => {
                                        let content_type_mime = response
                                            .headers()
                                            .get(reqwest::header::CONTENT_TYPE)
                                            .and_then(|v| v.to_str().ok())
                                            .and_then(|s| s.split(';').next())
                                            .map(|s| s.trim().to_string());

                                        match response.bytes().await {
                                            Ok(bytes) => {
                                                let bytes = bytes.to_vec();
                                                let display_name =
                                                    link.name.as_deref().unwrap_or("file");
                                                let ext = std::path::Path::new(display_name)
                                                    .extension()
                                                    .and_then(|e| e.to_str())
                                                    .map(str::to_lowercase)
                                                    .unwrap_or_else(|| "bin".to_string());
                                                let mime_type = content_type_mime.or_else(|| {
                                                    mime_guess::from_ext(&ext)
                                                        .first()
                                                        .map(|m| m.to_string())
                                                });
                                                let mime_type_str = mime_type
                                                    .as_deref()
                                                    .unwrap_or("application/octet-stream");

                                                let processing_result = ProcessingManager::new()
                                                    .process_file(&bytes, mime_type_str)
                                                    .await
                                                    .unwrap_or_default();

                                                let artifact_id = Uuid::new_v4();
                                                let storage = get_file_storage();

                                                match storage
                                                    .save_original(
                                                        context.user_id,
                                                        artifact_id,
                                                        &ext,
                                                        &bytes,
                                                    )
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        for (n, text) in processing_result
                                                            .text_pages
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_text_page(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    text,
                                                                )
                                                                .await;
                                                        }
                                                        if let Some(thumb) = processing_result
                                                            .thumbnails
                                                            .first()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    1,
                                                                    true,
                                                                    thumb,
                                                                )
                                                                .await;
                                                        }
                                                        for (n, img) in processing_result
                                                            .images
                                                            .iter()
                                                            .enumerate()
                                                        {
                                                            let _ = storage
                                                                .save_image(
                                                                    context.user_id,
                                                                    artifact_id,
                                                                    (n + 1) as u32,
                                                                    false,
                                                                    img,
                                                                )
                                                                .await;
                                                        }

                                                        let file_size = bytes.len() as i64;
                                                        match Repos
                                                            .file
                                                            .create(FileCreateData {
                                                                id: artifact_id,
                                                                user_id: context.user_id,
                                                                filename: display_name
                                                                    .to_string(),
                                                                file_size,
                                                                mime_type: mime_type.clone(),
                                                                checksum: None,
                                                                has_thumbnail:
                                                                    !processing_result
                                                                        .thumbnails
                                                                        .is_empty(),
                                                                preview_page_count:
                                                                    processing_result
                                                                        .images
                                                                        .len()
                                                                        as i32,
                                                                text_page_count:
                                                                    processing_result
                                                                        .text_pages
                                                                        .len()
                                                                        as i32,
                                                                processing_metadata:
                                                                    serde_json::to_value(
                                                                        &processing_result
                                                                            .metadata,
                                                                    )
                                                                    .unwrap_or_default(),
                                                                created_by: "mcp".to_string(),
                                                            })
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                helpers::send_artifact_created_event(
                                                                    tx,
                                                                    &artifact_id.to_string(),
                                                                    display_name,
                                                                    mime_type.as_deref(),
                                                                    file_size,
                                                                )
                                                                .await;

                                                                use crate::modules::chat::core::models::MessageContentData;
                                                                tool_results.push(
                                                                    MessageContentData::FileAttachment {
                                                                        file_id: artifact_id,
                                                                        filename: display_name
                                                                            .to_string(),
                                                                        mime_type,
                                                                        file_size,
                                                                    },
                                                                );

                                                                tracing::info!(
                                                                    "Artifact saved from resource_link: file_id={}, filename={}",
                                                                    artifact_id, display_name
                                                                );
                                                                let download_url = {
                                                                    use jsonwebtoken::{encode, EncodingKey, Header as JwtHeader};
                                                                    use crate::modules::file::types::DownloadTokenClaims;
                                                                    let now = chrono::Utc::now().timestamp() as usize;
                                                                    let claims = DownloadTokenClaims {
                                                                        file_id: artifact_id.to_string(),
                                                                        user_id: context.user_id.to_string(),
                                                                        exp: now + 3600,
                                                                        iat: now,
                                                                    };
                                                                    encode(
                                                                        &JwtHeader::default(),
                                                                        &claims,
                                                                        &EncodingKey::from_secret(self.config.jwt.secret.as_bytes()),
                                                                    )
                                                                    .ok()
                                                                    .map(|token| format!(
                                                                        "http://{}:{}{}/files/{}/download-with-token?token={}",
                                                                        self.config.server.host,
                                                                        self.config.server.port,
                                                                        self.config.server.api_prefix,
                                                                        artifact_id,
                                                                        token
                                                                    ))
                                                                };
                                                                saved_artifacts.push((artifact_id, display_name.to_string(), download_url));
                                                            }
                                                            Err(e) => {
                                                                tracing::error!(
                                                                    "Failed to create file DB record for resource_link: {}",
                                                                    e
                                                                );
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Failed to save artifact original: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read resource_link response body: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Ok(response) => {
                                        tracing::error!(
                                            "resource_link fetch returned HTTP {} for '{}': artifact NOT saved",
                                            response.status(),
                                            link.uri
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "Failed to fetch resource_link '{}': {} — artifact NOT saved",
                                            link.uri, e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to build HTTP client for resource_link fetch: {}",
                                    e
                                );
                            }
                        }
                        } // end for link in links
                    }
                }
            }

            // Update tool result content with the saved artifact info so the LLM knows the file_ids.
            // Also set hidden_content with token-based download URLs — included in LLM messages
            // but stripped from browser API responses.
            // saved_file_urls holds download-with-token URLs for is_saved=true links (no pipeline needed).
            if !saved_artifacts.is_empty() || !saved_file_urls.is_empty() {
                if let McpContentData::ToolResult { ref mut content, ref mut hidden_content, .. } = result {
                    if !saved_artifacts.is_empty() {
                        let file_descriptions: Vec<String> = saved_artifacts
                            .iter()
                            .map(|(id, name, _)| format!("'{}' (file_id: {})", name, id))
                            .collect();
                        *content = format!(
                            "Files from MCP tool have been saved as artifact attachments: {}. \
                             They will be shown as file cards in the UI — do not embed them inline in your response.",
                            file_descriptions.join(", ")
                        );
                    }
                    let mut url_lines: Vec<String> = saved_artifacts
                        .iter()
                        .filter_map(|(_, name, url)| url.as_ref().map(|u| format!("{} - {}", name, u)))
                        .collect();
                    for (name, url) in &saved_file_urls {
                        url_lines.push(format!("{} - {}", name, url));
                    }
                    if !url_lines.is_empty() {
                        *hidden_content = Some(format!(
                            "[system: Files saved as artifact attachments (shown as file cards in UI). \
                             Do NOT embed file URLs or images inline in your text response. \
                             Internal download URLs for tool-to-tool access only:\n{}]",
                            url_lines.join("\n")
                        ));
                    }
                }
            }

            // Capture is_final_response text + annotations before converting to MessageContentData
            if is_final {
                if let McpContentData::ToolResult { ref content, ref annotations, .. } = result {
                    tracing::info!(
                        "is_final_response: tool '{}' on server '{}' marked as final, will bypass LLM",
                        tool_name, server.name
                    );
                    final_response_text = Some(content.clone());
                    final_annotations = annotations.clone().unwrap_or_default();
                }
            }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());

            // Check stop_when_tools_called
            if loop_settings.stop_when_tools_called.iter().any(|stop_tool| {
                stop_tool.server_id == server_id && stop_tool.tool_name == tool_name
            }) {
                tracing::info!(
                    "Tool '{}' on server '{}' matches stop_when_tools_called, will complete after this iteration",
                    tool_name,
                    server_id
                );
                // Save accumulated tool_results to DB so tool_use blocks are not orphaned.
                // finalize() already wrote tool_use blocks; without matching tool_result blocks
                // the next LLM request will be rejected by Anthropic with "tool_use without tool_result".
                if let Some(message_id) = context.message_id {
                    let current_count = Repos.chat.core.get_message_with_content(message_id).await
                        .ok().flatten().map(|m| m.contents.len() as i32).unwrap_or(0);
                    for (idx, tr) in tool_results.iter().enumerate() {
                        let _ = Repos.chat.core.create_content(
                            message_id,
                            &tr.content_type(),
                            tr.clone(),
                            current_count + idx as i32,
                        ).await;
                    }
                }
                return Ok(ExtensionAction::Complete);
            }
        }

        // If any tool marked is_final_response: true, process references and bypass the LLM.
        // We must persist tool_results to DB BEFORE returning CompleteWithContent so that the
        // tool_use already stored by finalize() has a matching tool_result. Without this, the
        // next message's history reconstruction would see an unmatched tool_use and the API would
        // reject the request with "tool_use ids found without tool_result blocks".
        if let Some(text) = final_response_text {
            if let Some(message_id) = context.message_id {
                let current_count = match Repos.chat.core.get_message_with_content(message_id).await {
                    Ok(Some(msg)) => msg.contents.len() as i32,
                    _ => 0,
                };
                for (idx, result) in tool_results.iter().enumerate() {
                    let content_type = result.content_type();
                    if let Err(e) = Repos.chat.core.create_content(
                        message_id,
                        &content_type,
                        result.clone(),
                        current_count + idx as i32,
                    ).await {
                        tracing::error!("Failed to save tool result before CompleteWithContent: {}", e);
                    }
                }
                let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
            }
            return Ok(ExtensionAction::CompleteWithContent {
                text,
                annotations: final_annotations,
            });
        }

        // Cancel any elicitations that are still pending after all tools have been executed.
        if let Some(message_id) = context.message_id {
            let _ = Repos.chat.core.cancel_pending_elicitations(message_id).await;
        }

        // Return Continue action to append tool results to assistant message
        Ok(ExtensionAction::Continue {
            assistant_message_content: tool_results,
        })
    }

    fn convert_extension_content(&self, content: &Value) -> Option<ContentBlock> {
        // Check if this is MCP content by type field
        let content_type = content.get("type")?.as_str()?;
        if !matches!(content_type, "tool_use" | "tool_result") {
            return None;
        }

        // Deserialize to McpContentData and convert to ContentBlock
        serde_json::from_value::<McpContentData>(content.clone())
            .ok()
            .and_then(|mcp_content| mcp_content.to_content_block())
    }

    fn convert_from_content_block(&self, block: &ContentBlock) -> Option<MessageContentData> {
        // Try to convert ContentBlock to McpContentData
        McpContentData::from_content_block(block)
            .map(|mcp_content| mcp_content.to_message_content())
    }

    async fn process_delta(
        &self,
        delta: &ai_providers::ContentBlockDelta,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        // Convert ai-providers ToolUseDelta to our ContentBlockDelta::ToolUseDelta
        match delta {
            ai_providers::ContentBlockDelta::ToolUseDelta {
                index,
                id,
                name,
                input_delta,
            } => {
                tracing::info!(
                    "MCP process_delta: Converting ToolUseDelta at index {}, id={:?}, name={:?}",
                    index,
                    id,
                    name
                );
                Ok(Some(ContentBlockDelta::ToolUseDelta {
                    index: *index,
                    id: id.clone(),
                    name: name.clone(),
                    input_delta: input_delta.clone(),
                }))
            }
            _ => Ok(None), // Not a tool use delta
        }
    }

    async fn accumulate_delta(
        &self,
        delta: &ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<(), AppError> {
        tracing::info!(
            "MCP accumulate_delta called with delta variant: {}",
            match delta {
                ContentBlockDelta::ToolUseDelta { .. } => "ToolUseDelta",
                _ => "Other",
            }
        );

        // Only accumulate ToolUseDelta variants
        if let ContentBlockDelta::ToolUseDelta {
            index,
            id,
            name,
            input_delta,
        } = delta
        {
            // Get message_id from context
            let message_id = context
                .message_id
                .ok_or_else(|| AppError::internal_error("No message_id in context"))?;

            tracing::info!(
                "MCP accumulate_delta: Accumulating ToolUseDelta for message_id={}, index={}, id={:?}, name={:?}",
                message_id,
                index,
                id,
                name
            );

            let key = (message_id, *index);

            // Lock accumulator and update
            let mut accumulator = self
                .tool_use_accumulator
                .lock()
                .map_err(|e| AppError::internal_error(format!("Failed to lock accumulator: {}", e)))?;

            let entry = accumulator.entry(key).or_insert_with(Default::default);

            // Accumulate fields
            if let Some(id) = id {
                entry.id = Some(id.clone());
            }
            if let Some(name) = name {
                entry.name = Some(name.clone());
            }
            if let Some(input_delta) = input_delta {
                entry.input_json.push_str(input_delta);
            }

            tracing::debug!(
                "MCP: Accumulated tool use delta at index {}: id={:?}, name={:?}, input_len={}",
                index,
                entry.id,
                entry.name,
                entry.input_json.len()
            );
        }

        Ok(())
    }

    async fn get_accumulated_content(
        &self,
        context: &StreamContext,
    ) -> Result<Vec<(usize, MessageContentData)>, AppError> {
        // Get message_id from context
        let message_id = context
            .message_id
            .ok_or_else(|| AppError::internal_error("No message_id in context"))?;

        // Lock accumulator and extract all entries for this message
        let mut accumulator = self
            .tool_use_accumulator
            .lock()
            .map_err(|e| AppError::internal_error(format!("Failed to lock accumulator: {}", e)))?;

        let mut content_blocks = Vec::new();

        // Collect keys to remove (entries for this message)
        let keys_to_remove: Vec<_> = accumulator
            .keys()
            .filter(|(msg_id, _)| *msg_id == message_id)
            .copied()
            .collect();

        // Extract and convert each accumulated tool use
        for key in keys_to_remove {
            let (_, index) = key;
            if let Some(accumulated) = accumulator.remove(&key) {
                // Parse accumulated JSON input
                let input = serde_json::from_str(&accumulated.input_json).unwrap_or_else(|e| {
                    tracing::error!(
                        "Failed to parse accumulated tool use input JSON: {}. Input: {}",
                        e,
                        accumulated.input_json
                    );
                    serde_json::json!({}) // Fallback to empty object
                });

                // Parse server_id and tool name from accumulated name (format: server_id__tool_name)
                let full_name = accumulated.name.unwrap_or_default();
                let mut parts = full_name.splitn(2, "__");
                let (server_id, tool_name) = match (parts.next(), parts.next()) {
                    (Some(id), Some(name)) => (id.to_string(), name.to_string()),
                    _ => {
                        tracing::warn!("[mcp] Tool name missing server_id prefix: {}", full_name);
                        (String::new(), full_name)
                    }
                };

                // Create McpContentData::ToolUse with separate server_id and name
                let tool_use = McpContentData::ToolUse {
                    id: accumulated.id.unwrap_or_default(),
                    name: tool_name.clone(),
                    server_id: server_id.clone(),
                    input,
                };

                tracing::info!(
                    "MCP: Finalized tool use at index {}: id={}, name={}, server_id={}",
                    index,
                    tool_use.to_message_content().content_type(),
                    tool_name,
                    server_id,
                );

                content_blocks.push((index, tool_use.to_message_content()));
            }
        }

        Ok(content_blocks)
    }
}

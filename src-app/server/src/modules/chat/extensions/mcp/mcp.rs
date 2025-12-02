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
    ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};
use crate::modules::chat::core::models::{Message, MessageContentData};
use crate::modules::chat::core::types::streaming::ContentBlockDelta;
use crate::modules::mcp::client::manager::McpSessionManager;
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
    session_manager: Arc<McpSessionManager>,
    /// Storage for accumulating tool use deltas during streaming
    /// Key: (message_id, content_index)
    tool_use_accumulator: Arc<Mutex<HashMap<(Uuid, usize), AccumulatedToolUse>>>,
}

impl McpChatExtension {
    /// Create new MCP chat extension
    pub fn new(pool: PgPool) -> Self {
        let session_manager = Arc::new(McpSessionManager::new(pool.clone()));
        Self {
            pool,
            session_manager,
            tool_use_accumulator: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute approved tools and return MessageContentData results
    /// Used by both before_llm_call (no SSE) and after_llm_call (with SSE)
    async fn execute_approved_tools_sync(
        &self,
        approved_pending: &[super::approval::models::ToolUseApproval],
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<Vec<MessageContentData>, AppError> {
        
        

        let mut tool_results = Vec::new();
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        for approval in approved_pending {
            let tool_use_id = approval.tool_use_id.clone();
            let tool_name = approval.tool_name.clone();
            let input = approval.tool_input.clone();

            // Parse server_id from tool name (format: "server_id__tool_name" with __)
            let server_id_str = if let Some(idx) = tool_name.find("__") {
                &tool_name[..idx]
            } else {
                tracing::error!("Invalid tool name format: {}", tool_name);
                continue;
            };

            // Parse UUID
            let server_id = match uuid::Uuid::parse_str(server_id_str) {
                Ok(id) => id,
                Err(_) => {
                    tracing::error!("Invalid server_id in tool name: {}", tool_name);
                    continue;
                }
            };

            // Find server by ID
            let server = accessible_servers.iter().find(|s| s.id == server_id);

            if server.is_none() {
                tracing::error!("Server not found for approved tool: {}", tool_name);
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: Some(tool_name.clone()),
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event (if tx provided)
            if let Some(tx) = tx {
                helpers::send_tool_start_event(Some(tx), &tool_use_id, &tool_name, &server.name).await?;
            }

            // Get or create session
            let session_arc = self.session_manager.get_or_create(server.id).await?;
            let mut session = session_arc.write().await;


            // Execute tool
            let mut result = helpers::execute_tool(
                &mut session,
                &tool_name,
                input,
                &server.name,
                Some(server.timeout_seconds),
            )
            .await;

            // Set tool_use_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                is_error,
                ..
            } = result
            {
                *id = tool_use_id.clone();

                // Send tool complete event (if tx provided)
                if let Some(tx) = tx {
                    helpers::send_tool_complete_event(
                        Some(tx),
                        &tool_use_id,
                        &tool_name,
                        &server.name,
                        is_error.unwrap_or(false),
                    )
                    .await?;
                }
            }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());

            // Delete approval record after successful execution to prevent double-execution
            let _ = Repos
                .chat
                .mcp
                .delete_tool_approval(tool_use_id, approval.message_id)
                .await;
        }

        Ok(tool_results)
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
    ) -> Result<(), AppError> {
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
                                tracing::error!("Failed to approve tool use {}: {}", approval.tool_use_id, e);
                                return Err(e);
                            }
                        }
                    }
                    "deny" | "denied" => {
                        // Deny the tool use
                        super::approval::repository::deny_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.branch_id,
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await?;
                        tracing::debug!("Denied tool use: {}", approval.tool_use_id);
                    }
                    _ => {
                        return Err(AppError::bad_request(
                            "INVALID_DECISION",
                            format!("Invalid decision: '{}'. Must be 'approve'/'approved' or 'deny'/'denied'", approval.decision),
                        ));
                    }
                }
            }

            // === STEP 1b: Execute approved tools immediately after approval ===
            let approved_pending = super::approval::repository::get_approved_tools_for_branch(
                &self.pool,
                context.branch_id,
            )
            .await?;

            tracing::info!("before_llm_call: Found {} approved tools after processing approvals", approved_pending.len());

            if !approved_pending.is_empty() {
                // Execute approved tools and append results to request
                let tool_results = self.execute_approved_tools_sync(
                    &approved_pending,
                    context,
                    tx,
                ).await?;

                // Convert tool results to content blocks using extension's process_content_for_llm
                let mut content_blocks = Vec::new();
                for result in tool_results {
                    if let Some(block) = self.process_content_for_llm(&result, context).await? {
                        content_blocks.push(block);
                    }
                }

                // Append tool results as user message
                if !content_blocks.is_empty() {
                    use ai_providers::{ChatMessage, Role};
                    let count = content_blocks.len();
                    request.messages.push(ChatMessage {
                        role: Role::User,
                        content: content_blocks,
                    });
                    tracing::info!("Appended {} tool results to request", count);
                }
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
            return Ok(());
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
            return Ok(());
        }

        // Get all accessible servers with details
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Collect tools from all configured servers
        let mut all_tools = Vec::new();

        for (server_id, requested_tools) in &server_configs {
            // Find server details
            let server = accessible_servers
                .iter()
                .find(|s| s.id == *server_id)
                .ok_or_else(|| AppError::internal_error("Server not found in accessible list"))?;

            // Get or create MCP session
            let session_arc = self.session_manager.get_or_create(*server_id).await?;
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
        } else {
            tracing::warn!("No tools to add to ChatRequest!");
        }

        Ok(())
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        tracing::info!(
            "MCP after_llm_call: message_id={}, conversation_id={}",
            final_message.id,
            context.conversation_id
        );

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
            let tool_results = self.execute_approved_tools_sync(
                &approved_pending,
                context,
                tx,
            ).await?;
            tracing::info!("after_llm_call: Executed {} tools successfully", tool_results.len());

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

        // Find ToolUse content blocks
        let mut tool_uses = Vec::new();

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

                if let McpContentData::ToolUse { id, name, input } = mcp_content {
                    tool_uses.push((id, name, input));
                }
            }
        }

        tracing::info!(
            "Extracted {} tool uses from message {}",
            tool_uses.len(),
            final_message.id
        );

        if tool_uses.is_empty() {
            // No tool uses - conversation complete
            tracing::info!("No tool uses found, conversation complete");
            return Ok(ExtensionAction::Complete);
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

        // Determine which tools need approval vs can execute immediately
        let mut tools_to_execute = Vec::new();
        let mut tools_needing_approval = Vec::new();

        for (tool_use_id, tool_name, input) in tool_uses {
            let needs_approval = match approval_mode {
                crate::modules::chat::extensions::mcp::ApprovalMode::AutoApprove => false,
                crate::modules::chat::extensions::mcp::ApprovalMode::ManualApprove => {
                    // tool_name is in server_id__tool_name format
                    // Parse server_id and actual tool name, then check against auto_approved_servers
                    let is_auto_approved = if let Some(idx) = tool_name.find("__") {
                        let server_id_str = &tool_name[..idx];
                        let actual_tool_name = &tool_name[idx + 2..];
                        if let Ok(server_id) = uuid::Uuid::parse_str(server_id_str) {
                            // Check if server_id exists in auto_approved_servers and tool is in list
                            auto_approved_servers
                                .iter()
                                .any(|s| s.server_id == server_id && s.contains_tool(actual_tool_name))
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    tracing::info!(
                        "Tool '{}' auto-approved check: is_auto_approved={}",
                        tool_name,
                        is_auto_approved
                    );
                    !is_auto_approved
                }
                crate::modules::chat::extensions::mcp::ApprovalMode::Disabled => {
                    unreachable!("Already handled above")
                }
            };

            tracing::info!(
                "Tool '{}' (id={}): needs_approval={}",
                tool_name,
                tool_use_id,
                needs_approval
            );

            if needs_approval {
                tools_needing_approval.push((tool_use_id, tool_name, input));
            } else {
                tools_to_execute.push((tool_use_id, tool_name, input));
            }
        }

        // Create pending approval records for tools that need manual approval
        if !tools_needing_approval.is_empty() {
            tracing::info!(
                "Creating {} pending approval records",
                tools_needing_approval.len()
            );

            for (tool_use_id, tool_name, input) in &tools_needing_approval {
                // Extract server_id from tool name (format: "server_id__tool_name" with __)
                let server_id_str = if let Some(idx) = tool_name.find("__") {
                    &tool_name[..idx]
                } else {
                    "unknown"
                };

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
                crate::core::Repos
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

                // Send SSE event for approval required
                helpers::send_approval_required_event(tx, tool_use_id, tool_name, &server_name, input).await?;
            }

            // Return Complete to pause conversation - user must approve via API or tool_approvals field
            tracing::info!("Conversation paused, waiting for {} tool approvals", tools_needing_approval.len());
            return Ok(ExtensionAction::Complete);
        }

        tracing::info!("MCP extension: executing {} auto-approved tools", tools_to_execute.len());

        // accessible_servers already available from above

        // Execute each auto-approved tool and collect results
        let mut tool_results = Vec::new();

        for (tool_use_id, tool_name, input) in tools_to_execute {
            // Parse server_id from tool name (format: "server_id__tool_name" with __)
            let server_id_str = if let Some(idx) = tool_name.find("__") {
                &tool_name[..idx]
            } else {
                tracing::error!("Invalid tool name format: {}", tool_name);
                continue;
            };

            // Parse UUID
            let server_id = match uuid::Uuid::parse_str(server_id_str) {
                Ok(id) => id,
                Err(_) => {
                    tracing::error!("Invalid server_id in tool name: {}", tool_name);
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
                    content: format!("Server '{}' not found", server_id),
                    is_error: Some(true),
                };
                tool_results.push(error_result.to_message_content());
                continue;
            }

            let server = server.unwrap();

            // Send tool start event
            helpers::send_tool_start_event(tx, &tool_use_id, &tool_name, &server.name).await?;

            // Get or create session
            let session_arc = self.session_manager.get_or_create(server.id).await?;
            let mut session = session_arc.write().await;

            // Execute tool
            let mut result = helpers::execute_tool(
                &mut session,
                &tool_name,
                input,
                &server.name,
                Some(server.timeout_seconds),
            )
            .await;

            // Set tool_use_id
            if let McpContentData::ToolResult {
                tool_use_id: ref mut id,
                is_error,
                ..
            } = result
            {
                *id = tool_use_id.clone();

                // Send tool complete event
                helpers::send_tool_complete_event(
                    tx,
                    &tool_use_id,
                    &tool_name,
                    &server.name,
                    is_error.unwrap_or(false),
                )
                .await?;
            }

            // Convert to MessageContentData and add to results
            tool_results.push(result.to_message_content());
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

                // Create McpContentData::ToolUse
                let tool_use = McpContentData::ToolUse {
                    id: accumulated.id.unwrap_or_default(),
                    name: accumulated.name.unwrap_or_default(),
                    input,
                };

                tracing::info!(
                    "MCP: Finalized tool use at index {}: id={}, name={}",
                    index,
                    tool_use.to_message_content().content_type(),
                    match &tool_use {
                        McpContentData::ToolUse { name, .. } => name,
                        _ => "",
                    }
                );

                content_blocks.push((index, tool_use.to_message_content()));
            }
        }

        Ok(content_blocks)
    }
}

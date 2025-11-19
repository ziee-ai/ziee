// MCP chat extension implementation

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use serde_json::Value;
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;

use ai_providers::{ChatRequest, ContentBlock};

use crate::common::AppError;
use crate::modules::chat::core::extension::{
    ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};
use crate::modules::chat::core::models::{Message, MessageContentData};
use crate::modules::mcp::client::manager::McpSessionManager;
use crate::core::repository::Repos;

use super::content::McpContentData;
use super::extension::{McpServerConfig, SendMessageRequestFields};
use super::helpers;

/// MCP chat extension
///
/// Provides Model Context Protocol (MCP) tool calling functionality for chat.
pub struct McpChatExtension {
    pool: PgPool,
    session_manager: Arc<McpSessionManager>,
}

impl McpChatExtension {
    /// Create new MCP chat extension
    pub fn new(pool: PgPool) -> Self {
        let session_manager = Arc::new(McpSessionManager::new(pool.clone()));
        Self {
            pool,
            session_manager,
        }
    }
}

#[async_trait]
impl ChatExtension for McpChatExtension {
    fn name(&self) -> &str {
        "mcp"
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
    ) -> Result<(), AppError> {
        // === STEP 1: Process tool approvals (if resuming after approval) ===
        if let Some(approvals_value) = &send_request.tool_approvals {
            // Parse tool_approvals array
            let approvals: Vec<super::approval::models::ToolApprovalDecision> =
                serde_json::from_value(approvals_value.clone())
                    .map_err(|e| AppError::bad_request("INVALID_TOOL_APPROVALS", format!("Invalid tool_approvals format: {}", e)))?;

            tracing::info!(
                "Processing {} tool approval decisions for conversation {}",
                approvals.len(),
                context.conversation_id
            );

            // Process each approval decision
            for approval in approvals {
                match approval.decision.as_str() {
                    "approve" => {
                        // Approve the tool use
                        super::approval::repository::approve_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.message_id.unwrap_or(context.conversation_id), // Use message_id if available
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await?;
                        tracing::debug!("Approved tool use: {}", approval.tool_use_id);
                    }
                    "deny" => {
                        // Deny the tool use
                        super::approval::repository::deny_tool_use(
                            &self.pool,
                            approval.tool_use_id.clone(),
                            context.message_id.unwrap_or(context.conversation_id),
                            context.user_id,
                            approval.note.clone(),
                        )
                        .await?;
                        tracing::debug!("Denied tool use: {}", approval.tool_use_id);
                    }
                    _ => {
                        return Err(AppError::bad_request(
                            "INVALID_DECISION",
                            format!("Invalid decision: {}. Must be 'approve' or 'deny'", approval.decision),
                        ));
                    }
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

        // Parse mcp_config to get mcp_servers
        let mcp_servers: Option<Vec<McpServerConfig>> = if let Some(config) = &send_request.mcp_config {
            // Try to deserialize mcp_servers from config
            if let Some(servers) = config.get("mcp_servers") {
                serde_json::from_value(servers.clone()).ok()
            } else {
                None
            }
        } else {
            None
        };

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

            // Convert and add tools
            for mcp_tool in tools_to_add {
                let ai_tool = helpers::convert_mcp_tool_to_ai_tool(&server.name, &mcp_tool);
                all_tools.push(ai_tool);
            }
        }

        tracing::info!(
            "MCP extension: added {} tools from {} servers",
            all_tools.len(),
            server_configs.len()
        );

        // Add tools to ChatRequest
        if !all_tools.is_empty() {
            request.tools = all_tools;
        }

        Ok(())
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        // === STEP 1: Check for approved pending tools (from previous approval) ===
        let approved_pending = super::approval::repository::get_pending_approvals_for_branch(
            &self.pool,
            context.branch_id,
        )
        .await?
        .into_iter()
        .filter(|approval| approval.get_status() == super::approval::models::ApprovalStatus::Approved)
        .collect::<Vec<_>>();

        if !approved_pending.is_empty() {
            tracing::info!(
                "Found {} approved pending tools to execute",
                approved_pending.len()
            );

            // Execute approved tools
            let mut tool_results = Vec::new();
            let accessible_servers =
                helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

            for approval in approved_pending {
                let tool_use_id = approval.tool_use_id.clone();
                let tool_name = approval.tool_name.clone();
                let input = approval.tool_input.clone();

                // Parse server name from tool name (format: "server_name::tool_name")
                let server_name = if let Some(idx) = tool_name.find("::") {
                    &tool_name[..idx]
                } else {
                    tracing::error!("Invalid tool name format: {}", tool_name);
                    continue;
                };

                // Find server by name
                let server = accessible_servers.iter().find(|s| s.name == server_name);

                if server.is_none() {
                    tracing::error!("Server not found for approved tool: {}", tool_name);
                    let error_result = McpContentData::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: format!("Server '{}' not found", server_name),
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
                    content: _,
                    is_error,
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

                // Mark approval as executed (update to 'executed' or delete)
                // For now, we'll leave it as approved - could add an 'executed' state later
            }

            // Return Continue action to send tool results back to LLM
            tracing::info!("Returning {} approved tool results to LLM", tool_results.len());
            return Ok(ExtensionAction::Continue {
                user_message_content: tool_results,
            });
        }

        // === STEP 2: Load message contents and find new ToolUse blocks ===
        let message_with_content = Repos
            .chat
            .core
            .get_message_with_content(final_message.id)
            .await?
            .ok_or_else(|| AppError::internal_error("Message not found after finalization"))?;

        // Find ToolUse content blocks
        let mut tool_uses = Vec::new();

        for content in &message_with_content.contents {
            let content_data = content.parse_content()?;

            // Try to parse as MCP Extension content
            if let Ok(mcp_content) = McpContentData::from_message_content(&content_data) {
                if let McpContentData::ToolUse { id, name, input } = mcp_content {
                    tool_uses.push((id, name, input));
                }
            }
        }

        if tool_uses.is_empty() {
            // No tool uses - conversation complete
            return Ok(ExtensionAction::Complete);
        }

        // Check MCP approval settings for this conversation
        let settings = crate::core::Repos
            .chat
            .mcp
            .get_conversation_settings(context.conversation_id)
            .await?;

        let (approval_mode, auto_approved_tools) = if let Some(ref settings) = settings {
            // Normalize auto_approved_tools to canonical format
            let normalized_tools = crate::core::Repos
                .chat
                .mcp
                .normalize_auto_approved_tools(&settings.auto_approved_tools)
                .await
                .unwrap_or_default();
            (settings.get_approval_mode(), normalized_tools)
        } else {
            // No settings = default to manual approve with no auto-approved tools
            (crate::modules::chat::extensions::mcp::ApprovalMode::ManualApprove, Vec::new())
        };

        tracing::info!(
            "MCP extension: {} tools, approval_mode={}, auto_approved_count={}",
            tool_uses.len(),
            approval_mode,
            auto_approved_tools.len()
        );

        // Check approval mode
        if matches!(approval_mode, crate::modules::chat::extensions::mcp::ApprovalMode::Disabled) {
            tracing::info!("MCP disabled for conversation {}", context.conversation_id);
            return Ok(ExtensionAction::Complete);
        }

        // Determine which tools need approval vs can execute immediately
        let mut tools_to_execute = Vec::new();
        let mut tools_needing_approval = Vec::new();

        for (tool_use_id, tool_name, input) in tool_uses {
            let needs_approval = match approval_mode {
                crate::modules::chat::extensions::mcp::ApprovalMode::AutoApprove => false,
                crate::modules::chat::extensions::mcp::ApprovalMode::ManualApprove => {
                    // Check if this specific tool is auto-approved
                    !auto_approved_tools.contains(&tool_name)
                }
                crate::modules::chat::extensions::mcp::ApprovalMode::Disabled => {
                    unreachable!("Already handled above")
                }
            };

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

            // Build server_name -> server_id map for lookups
            let accessible_servers =
                helpers::get_all_accessible_config(&self.pool, context.user_id).await?;
            let server_name_to_id: std::collections::HashMap<String, uuid::Uuid> =
                accessible_servers
                    .iter()
                    .map(|s| (s.name.clone(), s.id))
                    .collect();

            for (tool_use_id, tool_name, input) in &tools_needing_approval {
                // Extract server name from tool name (format: "server_name::tool_name")
                let server_name = if let Some(idx) = tool_name.find("::") {
                    &tool_name[..idx]
                } else {
                    "unknown"
                };

                // Lookup server_id
                let server_id = server_name_to_id.get(server_name).copied();

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
                        server_name.to_string(),
                    )
                    .await?;

                // Send SSE event for approval required
                helpers::send_approval_required_event(tx, tool_use_id, tool_name, server_name).await?;
            }

            // Return Complete to pause conversation - user must approve via API or tool_approvals field
            tracing::info!("Conversation paused, waiting for {} tool approvals", tools_needing_approval.len());
            return Ok(ExtensionAction::Complete);
        }

        tracing::info!("MCP extension: executing {} auto-approved tools", tools_to_execute.len());

        // Get accessible servers from context metadata
        let accessible_servers =
            helpers::get_all_accessible_config(&self.pool, context.user_id).await?;

        // Execute each auto-approved tool and collect results
        let mut tool_results = Vec::new();

        for (tool_use_id, tool_name, input) in tools_to_execute {
            // Parse server name from tool name (format: "server_name::tool_name")
            let server_name = if let Some(idx) = tool_name.find("::") {
                &tool_name[..idx]
            } else {
                tracing::error!("Invalid tool name format: {}", tool_name);
                continue;
            };

            // Find server by name
            let server = accessible_servers
                .iter()
                .find(|s| s.name == server_name);

            if server.is_none() {
                tracing::error!("Server not found for tool: {}", tool_name);
                // Create error result
                let error_result = McpContentData::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: format!("Server '{}' not found", server_name),
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
                content: _,
                is_error,
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

        // Return Continue action to send tool results back to LLM
        Ok(ExtensionAction::Continue {
            user_message_content: tool_results,
        })
    }

    fn convert_extension_content(
        &self,
        extension_name: &str,
        content: &Value,
    ) -> Option<ContentBlock> {
        // Only handle "mcp" extension content
        if extension_name != "mcp" {
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
}

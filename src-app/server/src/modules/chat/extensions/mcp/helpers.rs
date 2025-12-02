// MCP extension helper functions

use axum::response::sse::Event;
use serde_json::Value;
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::types::streaming::SSEChatStreamEvent;
use crate::modules::mcp::client::session::McpSession;
use crate::modules::mcp::client::traits::Tool;
use crate::modules::mcp::{McpRepository, McpServer};

use super::content::McpContentData;
use super::extension::{
    McpServerConfig, SSEChatStreamMcpApprovalRequiredData, SSEChatStreamMcpToolCompleteData,
    SSEChatStreamMcpToolStartData,
};

/// Get all MCP servers accessible to the user
pub async fn get_all_accessible_config(
    pool: &sqlx::PgPool,
    user_id: Uuid,
) -> Result<Vec<McpServer>, AppError> {
    let repo = McpRepository::new(pool.clone());

    // Get all accessible servers (user servers + system servers via groups)
    let response = repo.list_accessible(user_id, 1, 1000).await?;

    // Filter out disabled servers
    let enabled_servers: Vec<McpServer> = response
        .servers
        .into_iter()
        .filter(|s| s.enabled)
        .collect();

    Ok(enabled_servers)
}

/// Validate requested servers and build final configuration
/// Returns (valid_configs, accessible_server_ids)
pub async fn validate_and_build_config(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    requested_servers: Option<Vec<McpServerConfig>>,
) -> Result<(Vec<(Uuid, Vec<String>)>, Vec<Uuid>), AppError> {
    // Get all accessible servers
    let accessible_servers = get_all_accessible_config(pool, user_id).await?;
    let accessible_ids: Vec<Uuid> = accessible_servers.iter().map(|s| s.id).collect();

    let config = if let Some(requested) = requested_servers {
        // Validate each requested server
        let mut valid_configs = Vec::new();

        for req in requested {
            // Check if user has access to this server
            if !accessible_ids.contains(&req.server_id) {
                tracing::warn!(
                    "User {} requested inaccessible MCP server {}",
                    user_id,
                    req.server_id
                );
                continue; // Skip inaccessible servers
            }

            valid_configs.push((req.server_id, req.tools));
        }

        valid_configs
    } else {
        // No specific servers requested - use all accessible servers with all tools
        accessible_ids.iter().map(|&id| (id, vec![])).collect()
    };

    Ok((config, accessible_ids))
}

/// Convert MCP Tool to AI provider Tool format
/// Uses server_id (UUID) to ensure uniqueness across users with same server names
pub fn convert_mcp_tool_to_ai_tool(
    server_id: Uuid,
    mcp_tool: &Tool,
) -> ai_providers::Tool {
    // Use double underscore separator for compatibility with Anthropic's naming rules
    // Anthropic requires: ^[a-zA-Z0-9_-]{1,128}$ (no colons allowed)
    // Using server_id (UUID) ensures uniqueness when multiple servers have same name
    ai_providers::Tool::function(
        format!("{}__{}", server_id, mcp_tool.name),
        mcp_tool.description.clone().unwrap_or_default(),
        mcp_tool.input_schema.clone(),
    )
}

/// Execute a tool via MCP session
pub async fn execute_tool(
    session: &mut McpSession,
    tool_name: &str,
    input: Value,
    _server_name: &str,
    timeout_seconds: Option<i32>,
) -> McpContentData {
    // Parse tool name (format: "server_id__tool_name")
    let actual_tool_name = if let Some(idx) = tool_name.rfind("__") {
        &tool_name[idx + 2..]
    } else {
        tool_name
    };

    // Execute with timeout
    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30) as u64);

    let result = tokio::time::timeout(
        timeout,
        session.call_tool(actual_tool_name, input.clone())
    ).await;

    match result {
        Ok(Ok(tool_result)) => {
            // Success - convert MCP ToolResult to our format
            let content_text = tool_result
                .content
                .iter()
                .map(|c| serde_json::to_string(&c.content).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n");

            // Truncate if too large (100KB limit)
            let final_content = if content_text.len() > 100_000 {
                let truncated = &content_text[..100_000];
                format!(
                    "{}\n\n[... truncated {} bytes ...]",
                    truncated,
                    content_text.len() - 100_000
                )
            } else {
                content_text
            };

            McpContentData::ToolResult {
                tool_use_id: String::new(), // Will be set by caller
                name: Some(tool_name.to_string()),
                content: final_content,
                is_error: Some(tool_result.is_error),
            }
        }
        Ok(Err(e)) => {
            // MCP error
            McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                content: format!("Tool execution failed: {}", e),
                is_error: Some(true),
            }
        }
        Err(_) => {
            // Timeout
            McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                content: format!(
                    "Tool execution timed out after {}s",
                    timeout_seconds.unwrap_or(30)
                ),
                is_error: Some(true),
            }
        }
    }
}

/// Send SSE event for tool start
/// Non-fatal: logs warning if send fails but doesn't return error
pub async fn send_tool_start_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
) -> Result<(), AppError> {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpToolStart(SSEChatStreamMcpToolStartData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE tool start event: {:?}", e);
        }
    }

    Ok(())
}

/// Send SSE event for tool complete
/// Non-fatal: logs warning if send fails but doesn't return error
pub async fn send_tool_complete_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    is_error: bool,
) -> Result<(), AppError> {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpToolComplete(SSEChatStreamMcpToolCompleteData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            is_error,
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE tool complete event: {:?}", e);
        }
    }

    Ok(())
}

/// Send SSE event for approval required
pub async fn send_approval_required_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    input: &serde_json::Value,
) -> Result<(), AppError> {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpApprovalRequired(SSEChatStreamMcpApprovalRequiredData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            input: input.clone(),
        });

        tx.send(Ok(event.into()))
            .map_err(|_| AppError::internal_error("Failed to send SSE event"))?;
    }

    Ok(())
}

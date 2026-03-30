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
///
/// # Arguments
/// * `session` - MCP session
/// * `tool_name` - Clean tool name (without server_id prefix)
/// * `input` - Tool input parameters
/// * `_server_name` - Server name (for logging)
/// * `timeout_seconds` - Execution timeout
pub async fn execute_tool(
    session: &mut McpSession,
    tool_name: &str,
    input: Value,
    _server_name: &str,
    timeout_seconds: Option<i32>,
) -> McpContentData {
    // Execute with timeout using the clean tool name
    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30) as u64);

    let result = tokio::time::timeout(
        timeout,
        session.call_tool(tool_name, input.clone())
    ).await;

    match result {
        Ok(Ok(tool_result)) => {
            // Success - convert MCP ToolResult to our format, parsing rich content types
            let mut text_parts: Vec<String> = Vec::new();
            let mut references: Vec<super::content::ReferenceItem> = Vec::new();
            let mut attachment: Option<super::content::RichFile> = None;

            for item in &tool_result.content {
                let content_type = item.content.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                match content_type {
                    "text" => {
                        if let Some(text) = item.content.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    }
                    "references" => {
                        if let Some(refs_array) = item.content.get("references").and_then(|r| r.as_array()) {
                            for ref_val in refs_array {
                                if let (Some(id), Some(display_text)) = (
                                    ref_val.get("id").and_then(|v| v.as_str()),
                                    ref_val.get("display_text").and_then(|v| v.as_str()),
                                ) {
                                    references.push(super::content::ReferenceItem {
                                        id: id.to_string(),
                                        display_text: display_text.to_string(),
                                        source_url: ref_val.get("source_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                    });
                                }
                            }
                        }
                    }
                    "file" => {
                        if let (Some(filename), Some(mime_type), Some(data)) = (
                            item.content.get("filename").and_then(|v| v.as_str()),
                            item.content.get("mime_type").and_then(|v| v.as_str()),
                            item.content.get("data").and_then(|v| v.as_str()),
                        ) {
                            attachment = Some(super::content::RichFile {
                                filename: filename.to_string(),
                                mime_type: mime_type.to_string(),
                                data: data.to_string(),
                            });
                        }
                    }
                    _ => {
                        // Unknown type: serialize as-is
                        if let Ok(s) = serde_json::to_string(&item.content) {
                            text_parts.push(s);
                        }
                    }
                }
            }

            let content_text = text_parts.join("\n");

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
                references: if references.is_empty() { None } else { Some(references) },
                attachment,
            }
        }
        Ok(Err(e)) => {
            // MCP error
            McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                content: format!("Tool execution failed: {}", e),
                is_error: Some(true),
                references: None,
                attachment: None,
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
                references: None,
                attachment: None,
            }
        }
    }
}

/// Send SSE event for tool start.
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_tool_start_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
) {
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
}

/// Send SSE event for tool complete.
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_tool_complete_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    is_error: bool,
) {
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
}

/// Send SSE event for approval required.
///
/// This is **fatal** (returns `Err`) if the channel send fails. Unlike tool start/complete
/// events which are purely informational, an approval-required notification that never
/// reaches the client leaves the user with no way to act — there is no point continuing
/// the request.
pub async fn send_approval_required_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    tool_use_id: &str,
    tool_name: &str,
    server: &str,
    server_id: &str,
    input: &serde_json::Value,
) -> Result<(), AppError> {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpApprovalRequired(SSEChatStreamMcpApprovalRequiredData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            server_id: server_id.to_string(),
            input: input.clone(),
        });

        tx.send(Ok(event.into()))
            .map_err(|_| AppError::internal_error("Failed to send SSE event"))?;
    }

    Ok(())
}

/// Build tool input by mapping user message text to the first required string parameter.
///
/// Returns `None` when the schema has required parameters but none of them are strings,
/// meaning we cannot auto-map the query text — the caller should skip "always mode" for
/// this tool rather than submitting wrong inputs silently.
///
/// Returns `Some` in two cases:
/// - A required string parameter was found → `{ param_name: query_text }`
/// - No schema information available → generic fallback `{ "query": query_text }`
pub fn build_query_input(schema: &serde_json::Value, query_text: &str) -> Option<serde_json::Value> {
    if let (Some(props), Some(required)) = (
        schema.get("properties").and_then(|p| p.as_object()),
        schema.get("required").and_then(|r| r.as_array()),
    ) {
        for req_key in required {
            if let Some(key) = req_key.as_str() {
                let is_string = props.get(key)
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("string");
                if is_string {
                    return Some(serde_json::json!({ key: query_text }));
                }
            }
        }
        // Has required params but none are strings — cannot auto-map
        None
    } else {
        // No schema info — use generic fallback
        Some(serde_json::json!({ "query": query_text }))
    }
}

#[cfg(test)]
mod tests {
    use super::build_query_input;

    #[test]
    fn test_build_query_input_required_string_param() {
        let schema = serde_json::json!({
            "required": ["query"],
            "properties": {
                "query": { "type": "string" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "query": "test message" })));
    }

    #[test]
    fn test_build_query_input_fallback_to_query_key() {
        // Schema has no required params (only optional) → uses generic fallback
        let schema = serde_json::json!({
            "properties": {
                "count": { "type": "integer" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "query": "test message" })));
    }

    #[test]
    fn test_build_query_input_picks_first_required_string() {
        // First required string param is "topic", not "limit" (integer)
        let schema = serde_json::json!({
            "required": ["topic", "limit"],
            "properties": {
                "topic": { "type": "string" },
                "limit": { "type": "integer" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, Some(serde_json::json!({ "topic": "test message" })));
    }

    #[test]
    fn test_build_query_input_returns_none_for_non_string_required_params() {
        // Schema has required params but none are strings — auto-mapping is impossible
        let schema = serde_json::json!({
            "required": ["count", "enabled"],
            "properties": {
                "count": { "type": "integer" },
                "enabled": { "type": "boolean" }
            }
        });
        let result = build_query_input(&schema, "test message");
        assert_eq!(result, None, "Should return None when required params exist but none are strings");
    }
}

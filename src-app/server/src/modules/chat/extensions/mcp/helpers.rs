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
    SSEChatStreamMcpToolStartData, SSEChatStreamArtifactCreatedData,
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
    message_id: Option<uuid::Uuid>,
    sse_tx: Option<tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    elicit_notify_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::modules::mcp::elicitation::models::ElicitationStartedNotification>>,
) -> (McpContentData, bool) {
    // Returns (result, user_only_audience).
    //
    // user_only_audience is true when at least one content block carries the
    // MCP-spec `annotations.audience: ["user"]` metadata EXACTLY — meaning
    // "intended for the human user only, not the assistant." When set, the
    // caller streams the tool text directly to the user without another LLM
    // call (the tool's output IS the assistant's final answer).
    //
    // The exact-match check (audience contains "user" and ONLY "user", not
    // also "assistant") is deliberate: per the MCP spec
    // (modelcontextprotocol.io/specification/2025-11-25/server/resources#annotations),
    // `["user", "assistant"]` means "both audiences should see it" — the LLM
    // should ALSO process such content, which means we must NOT bypass it.

    // Elicitation may block for up to 300s; use a generous outer timeout so that
    // elicitation requests have time to complete before we give up.
    // The tool-level timeout is enforced separately inside call_tool_with_sampling.
    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(30) as u64 + 300);

    let result = tokio::time::timeout(
        timeout,
        session.call_tool(tool_name, input.clone(), message_id, sse_tx, elicit_notify_tx)
    ).await;

    match result {
        Ok(Ok(tool_result)) => {
            // Success - convert MCP ToolResult to our format, parsing rich content types
            let mut text_parts: Vec<String> = Vec::new();
            let mut attachment: Option<super::content::RichFile> = None;
            let mut resource_links: Vec<super::content::ResourceLink> = Vec::new();

            for item in &tool_result.content {
                let content_type = item.content.get("type").and_then(|t| t.as_str()).unwrap_or("text");
                match content_type {
                    "text" => {
                        if let Some(text) = item.content.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(text.to_string());
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
                    "resource_link" => {
                        // MCP resource_link: a reference to a persisted resource (not inline content)
                        if let Some(uri) = item.content.get("uri").and_then(|v| v.as_str()) {
                            let name = item.content.get("name").and_then(|v| v.as_str()).unwrap_or("file");
                            resource_links.push(super::content::ResourceLink {
                                uri: uri.to_string(),
                                name: item.content.get("name").and_then(|v| v.as_str()).map(String::from),
                                mime_type: item.content.get("mimeType").and_then(|v| v.as_str()).map(String::from),
                                size: item.content.get("size").and_then(|v| v.as_i64()),
                                is_saved: item.content.get("is_saved").and_then(|v| v.as_bool()),
                            });
                            // Provide the LLM with a readable confirmation so it doesn't retry
                            text_parts.push(format!("resource_link available — name: {}, uri: {}", name, uri));
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

            let mcp_result = McpContentData::ToolResult {
                tool_use_id: String::new(), // Will be set by caller
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: final_content,
                is_error: Some(tool_result.is_error),
                attachment,
                resource_links: if resource_links.is_empty() { None } else { Some(resource_links) },
                hidden_content: None, // Set later if resource_links artifacts are saved
            };
            // Bypass the LLM only when at least one content block is exactly
            // user-targeted: audience == ["user"] (single-element array, no
            // "assistant"). Per the MCP spec, ["user", "assistant"] means
            // both should see it — the LLM still needs to process the content
            // in that case, so we must NOT bypass.
            let user_only_audience = tool_result.content.iter().any(|c| {
                c.content
                    .get("annotations")
                    .and_then(|a| a.get("audience"))
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.len() == 1 && arr[0].as_str() == Some("user"))
                    .unwrap_or(false)
            });
            (mcp_result, user_only_audience)
        }
        Ok(Err(e)) => {
            // MCP error
            (McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: format!("Tool execution failed: {}", e),
                is_error: Some(true),
                attachment: None,
                resource_links: None,
                hidden_content: None,
            }, false)
        }
        Err(_) => {
            // Timeout
            (McpContentData::ToolResult {
                tool_use_id: String::new(),
                name: Some(tool_name.to_string()),
                server_id: None, // Will be set by caller
                content: format!(
                    "Tool execution timed out after {}s",
                    timeout_seconds.unwrap_or(30)
                ),
                is_error: Some(true),
                attachment: None,
                resource_links: None,
                hidden_content: None,
            }, false)
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
    input: &serde_json::Value,
) {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::McpToolStart(SSEChatStreamMcpToolStartData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            input: input.clone(),
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
    result: Option<&str>,
) {
    if let Some(tx) = tx {
        let result_truncated = result.map(|r| {
            if r.len() > 2000 {
                format!("{}...[truncated]", &r[..2000])
            } else {
                r.to_string()
            }
        });

        let event = SSEChatStreamEvent::McpToolComplete(SSEChatStreamMcpToolCompleteData {
            tool_use_id: tool_use_id.to_string(),
            tool_name: tool_name.to_string(),
            server: server.to_string(),
            is_error,
            result: result_truncated,
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

/// Send SSE event when a tool creates an artifact file (via MCP resource_link).
/// Fire-and-forget: logs a warning if the channel is closed but never fails the caller.
pub async fn send_artifact_created_event(
    tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    file_id: &str,
    filename: &str,
    mime_type: Option<&str>,
    file_size: i64,
) {
    if let Some(tx) = tx {
        let event = SSEChatStreamEvent::ArtifactCreated(SSEChatStreamArtifactCreatedData {
            file_id: file_id.to_string(),
            filename: filename.to_string(),
            mime_type: mime_type.map(String::from),
            file_size,
        });

        if let Err(e) = tx.send(Ok(event.into())) {
            tracing::warn!("Failed to send SSE artifact created event: {:?}", e);
        }
    }
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

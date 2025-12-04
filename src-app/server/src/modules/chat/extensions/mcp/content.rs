// MCP content types
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::common::AppError;
use crate::modules::chat::core::models::content::MessageContentData;

/// MCP-specific content data types
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentData {
    /// Tool use request (AI wants to call a tool)
    ToolUse {
        id: String,
        /// Tool name (without server_id prefix)
        name: String,
        /// Server ID (UUID)
        server_id: String,
        input: serde_json::Value,
    },
    /// Tool result (response from tool execution)
    ToolResult {
        tool_use_id: String,
        /// Function/tool name (required for some providers like Gemini)
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl McpContentData {
    /// Convert to MessageContentData via serialization/deserialization
    /// McpContentData variants map directly to MessageContentData variants (ToolUse, ToolResult)
    pub fn to_message_content(&self) -> MessageContentData {
        // Serialize to JSON and deserialize as MessageContentData
        // This works because both enums have the same variant structure and use #[serde(tag = "type")]
        let json = serde_json::to_value(self).expect("McpContentData should serialize");
        serde_json::from_value(json).expect("Failed to deserialize as MessageContentData")
    }

    /// Try to convert from MessageContentData
    pub fn from_message_content(content: &MessageContentData) -> Result<Self, AppError> {
        // Serialize MessageContentData to JSON and try to deserialize as McpContentData
        // This works because both enums have matching variant structures (ToolUse, ToolResult)
        let json = serde_json::to_value(content)
            .map_err(|e| AppError::internal_error(format!("Failed to serialize MessageContentData: {}", e)))?;

        serde_json::from_value(json).map_err(|e| {
            AppError::internal_error(format!("Failed to deserialize as McpContentData: {}", e))
        })
    }

    /// Convert to ai-providers ContentBlock
    pub fn to_content_block(&self) -> Option<ai_providers::ContentBlock> {
        match self {
            Self::ToolUse { id, name, server_id, input } => Some(ai_providers::ContentBlock::ToolUse {
                id: id.clone(),
                // Reconstruct server_id__name format for AI providers
                name: format!("{}__{}", server_id, name),
                input: input.clone(),
            }),
            Self::ToolResult {
                tool_use_id,
                name,
                content,
                is_error,
            } => Some(ai_providers::ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                name: name.clone(),
                content: vec![ai_providers::ContentBlock::Text {
                    text: content.clone(),
                }],
                is_error: *is_error,
            }),
        }
    }

    /// Convert from ai-providers ContentBlock
    pub fn from_content_block(block: &ai_providers::ContentBlock) -> Option<Self> {
        match block {
            ai_providers::ContentBlock::ToolUse { id, name, input } => {
                // Parse server_id__tool_name format
                let (server_id, tool_name) = if let Some(idx) = name.find("__") {
                    (name[..idx].to_string(), name[idx + 2..].to_string())
                } else {
                    // Fallback: no server_id prefix
                    (String::new(), name.clone())
                };
                Some(Self::ToolUse {
                    id: id.clone(),
                    name: tool_name,
                    server_id,
                    input: input.clone(),
                })
            }
            ai_providers::ContentBlock::ToolResult {
                tool_use_id,
                name,
                content,
                is_error,
            } => {
                // Extract text from content blocks
                let text = content
                    .iter()
                    .filter_map(|block| match block {
                        ai_providers::ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                Some(Self::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: name.clone(),
                    content: text,
                    is_error: *is_error,
                })
            }
            _ => None,
        }
    }

    /// Get content type string
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::ToolUse { .. } => "tool_use",
            Self::ToolResult { .. } => "tool_result",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_use_conversion() {
        let tool_use = McpContentData::ToolUse {
            id: "toolu_01".to_string(),
            name: "get_weather".to_string(),
            server_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            input: serde_json::json!({"location": "SF"}),
        };

        // Convert to MessageContentData
        let msg_content = tool_use.to_message_content();
        assert!(matches!(msg_content, MessageContentData::Extension { .. }));

        // Convert back
        let recovered = McpContentData::from_message_content(&msg_content).unwrap();
        match recovered {
            McpContentData::ToolUse { id, name, server_id, .. } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "get_weather");
                assert_eq!(server_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_tool_result_conversion() {
        let tool_result = McpContentData::ToolResult {
            tool_use_id: "toolu_01".to_string(),
            name: Some("get_weather".to_string()),
            content: "Sunny, 72°F".to_string(),
            is_error: Some(false),
        };

        // Convert to MessageContentData
        let msg_content = tool_result.to_message_content();

        // Convert back
        let recovered = McpContentData::from_message_content(&msg_content).unwrap();
        match recovered {
            McpContentData::ToolResult {
                tool_use_id,
                name,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "toolu_01");
                assert_eq!(name, Some("get_weather".to_string()));
                assert_eq!(content, "Sunny, 72°F");
                assert_eq!(is_error, Some(false));
            }
            _ => panic!("Expected ToolResult"),
        }
    }

    #[test]
    fn test_to_content_block() {
        let tool_use = McpContentData::ToolUse {
            id: "toolu_01".to_string(),
            name: "get_weather".to_string(),
            server_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            input: serde_json::json!({"location": "SF"}),
        };

        let block = tool_use.to_content_block().unwrap();
        match block {
            ai_providers::ContentBlock::ToolUse { name, .. } => {
                // Should reconstruct server_id__name format
                assert_eq!(name, "550e8400-e29b-41d4-a716-446655440000__get_weather");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    #[test]
    fn test_from_content_block() {
        // AI providers return server_id__tool_name format
        let block = ai_providers::ContentBlock::ToolUse {
            id: "toolu_01".to_string(),
            name: "550e8400-e29b-41d4-a716-446655440000__get_weather".to_string(),
            input: serde_json::json!({"location": "SF"}),
        };

        let mcp_content = McpContentData::from_content_block(&block).unwrap();
        match mcp_content {
            McpContentData::ToolUse { id, name, server_id, .. } => {
                assert_eq!(id, "toolu_01");
                assert_eq!(name, "get_weather");
                assert_eq!(server_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected ToolUse"),
        }
    }
}

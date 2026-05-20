// MCP content types
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::common::AppError;
use crate::modules::chat::core::models::content::MessageContentData;

/// A file attachment returned by a tool (inline base64 content)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RichFile {
    pub filename: String,
    pub mime_type: String,
    /// Base64-encoded file content
    pub data: String,
}

/// A reference to a resource returned by a tool (MCP resource_link content type)
///
/// Used when a tool creates or references a file that is already persisted on the server.
/// The URI follows the pattern `/api/files/{file_id}` for files stored in the file system.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ResourceLink {
    /// URI of the resource (e.g. `/api/files/{file_id}`)
    pub uri: String,
    /// Display name for the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// MIME type of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// File size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    /// Whether the file is already persisted in originals storage (user-attached).
    /// true  → URI is a download-with-token URL; skip fetch-and-save pipeline.
    /// false → workspace file; run full processing pipeline.
    /// None  → external MCP server; run full processing pipeline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_saved: Option<bool>,
}

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
        /// ID of the MCP server that executed this tool (UUID string)
        #[serde(skip_serializing_if = "Option::is_none")]
        server_id: Option<String>,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
        /// Inline file attachment returned by a tool (base64-encoded content)
        #[serde(skip_serializing_if = "Option::is_none")]
        attachment: Option<RichFile>,
        /// References to persisted files returned by a tool (MCP resource_link)
        #[serde(skip_serializing_if = "Option::is_none")]
        resource_links: Option<Vec<ResourceLink>>,
        /// Context injected into LLM messages but never rendered for users (e.g. download URL)
        #[serde(skip_serializing_if = "Option::is_none")]
        hidden_content: Option<String>,
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
                hidden_content,
                ..
            } => {
                // Send text content to LLM.
                // Append hidden_content (e.g. download URL) for the LLM — never stored in `content`.
                let mut llm_text = content.clone();
                if let Some(hidden) = hidden_content {
                    llm_text.push_str(&format!("\n{}", hidden));
                }
                Some(ai_providers::ContentBlock::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    name: name.clone(),
                    content: vec![ai_providers::ContentBlock::Text { text: llm_text }],
                    is_error: *is_error,
                })
            }
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
                    server_id: None,
                    content: text,
                    is_error: *is_error,
                    attachment: None,
                    resource_links: None,
                    hidden_content: None,
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

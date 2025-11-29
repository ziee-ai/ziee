// Chat content models
#![allow(dead_code)]

// Content models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::common::AppError;

/// Message content entity - Represents a content block within a message
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageContent {
    pub id: Uuid,
    pub message_id: Uuid,
    pub content_type: String,
    #[schemars(with = "MessageContentData")]
    pub content: Value, // JSONB - contains MessageContentData
    pub sequence_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MessageContent {
    /// Parse the JSONB content into MessageContentData
    pub fn parse_content(&self) -> Result<MessageContentData, AppError> {
        serde_json::from_value(self.content.clone()).map_err(|e| AppError::database_error(e))
    }
}

/// Metadata for thinking content
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThinkingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
}

/// Content data types - Base types (extensions can add more via composition)
///
/// Extensions add variants by defining MessageContentDataVariants enums in their
/// extension.rs files. The compose_message_content_variants macro merges them at compile time.
#[macros::compose_message_content_variants]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContentData {
    /// Plain text content
    Text {
        text: String,
    },

    /// Thinking/reasoning content (Claude-style extended thinking)
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<ThinkingMetadata>,
    },

    // Extension variants (Image, FileAttachment, ToolUse, ToolResult) are added by the macro
}

impl MessageContentData {
    /// Get the content type string
    pub fn content_type(&self) -> String {
        match self {
            Self::Text { .. } => "text".to_string(),
            Self::Thinking { .. } => "thinking".to_string(),
            // Extension variants return their own type strings automatically via serde
            _ => {
                // For extension variants, use serde to get the type field
                serde_json::to_value(self)
                    .ok()
                    .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
                    .unwrap_or_else(|| "unknown".to_string())
            }
        }
    }

    /// Serialize to API-facing JSON format
    pub fn to_api_content(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

// Conversion implementations
// When adding a new content type above, add its conversion logic here
impl MessageContentData {
    /// Convert to ai-providers ContentBlock (only for base types)
    pub fn to_content_block(&self) -> Option<ai_providers::ContentBlock> {
        match self {
            Self::Text { text } => Some(ai_providers::ContentBlock::Text { text: text.clone() }),
            Self::Thinking { thinking, .. } => Some(ai_providers::ContentBlock::Thinking {
                thinking: thinking.clone(),
            }),
            // Extension content (Image, FileAttachment, ToolUse, ToolResult) handled by ExtensionRegistry
            _ => None,
        }
    }

    /// Convert from ai-providers ContentBlock (only for base types)
    pub fn from_content_block(block: &ai_providers::ContentBlock) -> Option<Self> {
        match block {
            ai_providers::ContentBlock::Text { text } => Some(Self::Text { text: text.clone() }),
            ai_providers::ContentBlock::Thinking { thinking } => Some(Self::Thinking {
                thinking: thinking.clone(),
                metadata: None,
            }),
            // Extension types (Image, FileAttachment, ToolUse, ToolResult) handled by ExtensionRegistry
            _ => None,
        }
    }

    /// Extract text content if this is a Text variant
    pub fn to_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text.as_str()),
            _ => None,
        }
    }
}

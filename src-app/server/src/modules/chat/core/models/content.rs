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
    pub content: Value, // JSONB - contains MessageContentData
    pub sequence_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MessageContent {
    /// Parse the JSONB content into MessageContentData
    pub fn parse_content(&self) -> Result<MessageContentData, AppError> {
        serde_json::from_value(self.content.clone())
            .map_err(|e| AppError::database_error(e))
    }
}

/// Metadata for thinking content
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThinkingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
}

/// Image source (URL or base64)
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Url { url: String },
    Base64 { media_type: String, data: String },
}

/// Macro to define MessageContentData with all content types
///
/// To add a new content type:
/// 1. Add one line to the content_types list below:
///    `(VariantName, { field1: Type1, field2: Type2 }, "content_type_string", "Documentation")`
/// 2. Add conversion logic in the impl block below (to_content_block/from_content_block)
///
/// Example:
/// ```
/// (CustomContent, { data: String, metadata: Option<Value> }, "custom", "Custom content type"),
/// ```
macro_rules! define_message_content_data {
    (
        content_types: [
            $(
                (
                    $variant:ident,
                    {
                        $(
                            $(#[$field_attr:meta])*
                            $field:ident : $field_ty:ty
                        ),* $(,)?
                    },
                    $content_type_str:expr,
                    $doc:expr
                )
            ),* $(,)?
        ]
    ) => {
        #[doc = "Content data types - Extensions can add more by modifying the macro invocation"]
        #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum MessageContentData {
            $(
                #[doc = $doc]
                $variant {
                    $(
                        $(#[$field_attr])*
                        $field: $field_ty,
                    )*
                },
            )*
        }

        impl MessageContentData {
            /// Get the content type string
            pub fn content_type(&self) -> &'static str {
                match self {
                    $(
                        Self::$variant { .. } => $content_type_str,
                    )*
                }
            }
        }
    };
}

// Central registry of all content types
// To add a new content type, add one line here following the pattern
define_message_content_data! {
    content_types: [
        // Base content types (always available)
        (
            Text,
            { text: String },
            "text",
            "Plain text content"
        ),
        (
            Thinking,
            {
                thinking: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                metadata: Option<ThinkingMetadata>
            },
            "thinking",
            "Thinking/reasoning content (Claude-style extended thinking)"
        ),
        (
            Image,
            {
                source: ImageSource,
                #[serde(skip_serializing_if = "Option::is_none")]
                alt_text: Option<String>
            },
            "image",
            "Image content"
        ),

        // MCP extension content types (for tool calling)
        (
            ToolUse,
            {
                id: String,
                name: String,
                input: serde_json::Value
            },
            "tool_use",
            "Tool use request (AI wants to call a tool)"
        ),
        (
            ToolResult,
            {
                tool_use_id: String,
                content: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                is_error: Option<bool>
            },
            "tool_result",
            "Tool result (response from tool execution)"
        ),
    ]
}

// Conversion implementations
// When adding a new content type above, add its conversion logic here
impl MessageContentData {
    /// Convert to ai-providers ContentBlock
    pub fn to_content_block(&self) -> Option<ai_providers::ContentBlock> {
        match self {
            // Base types
            Self::Text { text } => Some(ai_providers::ContentBlock::Text {
                text: text.clone(),
            }),
            Self::Thinking { thinking, .. } => Some(ai_providers::ContentBlock::Thinking {
                thinking: thinking.clone(),
            }),
            Self::Image { source, alt_text: _ } => Some(ai_providers::ContentBlock::Image {
                source: match source {
                    ImageSource::Url { url } => ai_providers::ImageSource::Url {
                        url: url.clone(),
                        detail: None, // We don't store detail level
                    },
                    ImageSource::Base64 { media_type, data } => {
                        ai_providers::ImageSource::Base64 {
                            media_type: media_type.clone(),
                            data: data.clone(),
                        }
                    }
                },
            }),

            // MCP extension types
            Self::ToolUse { id, name, input } => Some(ai_providers::ContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(ai_providers::ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
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
            // Base types
            ai_providers::ContentBlock::Text { text } => Some(Self::Text {
                text: text.clone(),
            }),
            ai_providers::ContentBlock::Thinking { thinking } => Some(Self::Thinking {
                thinking: thinking.clone(),
                metadata: None,
            }),
            ai_providers::ContentBlock::Image { source } => Some(Self::Image {
                source: match source {
                    ai_providers::ImageSource::Url { url, .. } => ImageSource::Url {
                        url: url.clone(),
                    },
                    ai_providers::ImageSource::Base64 { media_type, data } => {
                        ImageSource::Base64 {
                            media_type: media_type.clone(),
                            data: data.clone(),
                        }
                    }
                },
                alt_text: None,
            }),

            // MCP extension types
            ai_providers::ContentBlock::ToolUse { id, name, input } => Some(Self::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            ai_providers::ContentBlock::ToolResult {
                tool_use_id,
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
                    content: text,
                    is_error: *is_error,
                })
            }
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

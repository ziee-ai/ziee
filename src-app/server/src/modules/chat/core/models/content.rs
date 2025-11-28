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
        serde_json::from_value(self.content.clone()).map_err(|e| AppError::database_error(e))
    }
}

/// Metadata for thinking content
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ThinkingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
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
        #[derive(Debug, Clone, schemars::JsonSchema)]
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

        // Generic extension content type
        (
            Extension,
            {
                content: serde_json::Value
            },
            "extension",
            "Extension-specific content (flattened in storage)"
        ),
    ]
}

impl MessageContentData {
    /// Get the content type string
    pub fn content_type(&self) -> String {
        match self {
            Self::Text { .. } => "text".to_string(),
            Self::Thinking { .. } => "thinking".to_string(),
            Self::Extension { content } => {
                // Extract type from flattened content
                content
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("extension")
                    .to_string()
            }
        }
    }

    /// Serialize to API-facing JSON format (flattened)
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
            // Extension content handled by ExtensionRegistry
            Self::Extension { .. } => None,
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
            // Extension types handled by ExtensionRegistry
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

// Custom Serialize implementation to flatten Extension content
impl Serialize for MessageContentData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        match self {
            Self::Text { text } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "text")?;
                map.serialize_entry("text", text)?;
                map.end()
            }
            Self::Thinking { thinking, metadata } => {
                let mut map = serializer.serialize_map(Some(if metadata.is_some() { 3 } else { 2 }))?;
                map.serialize_entry("type", "thinking")?;
                map.serialize_entry("thinking", thinking)?;
                if let Some(meta) = metadata {
                    map.serialize_entry("metadata", meta)?;
                }
                map.end()
            }
            Self::Extension { content } => {
                // Serialize just the flattened content (which already has "type" field)
                content.serialize(serializer)
            }
        }
    }
}

// Custom Deserialize implementation to handle flattened Extension content
impl<'de> Deserialize<'de> for MessageContentData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let content_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| serde::de::Error::missing_field("type"))?;

        match content_type {
            "text" => {
                let text = value
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| serde::de::Error::missing_field("text"))?
                    .to_string();
                Ok(MessageContentData::Text { text })
            }
            "thinking" => {
                let thinking = value
                    .get("thinking")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| serde::de::Error::missing_field("thinking"))?
                    .to_string();
                let metadata = value
                    .get("metadata")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
                Ok(MessageContentData::Thinking { thinking, metadata })
            }
            _ => {
                // Unknown type - treat as Extension with flattened content
                Ok(MessageContentData::Extension { content: value })
            }
        }
    }
}

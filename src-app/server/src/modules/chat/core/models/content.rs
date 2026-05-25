// Chat content models
#![allow(dead_code)]

// Content models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::common::AppError;

/// Serializer that strips `hidden_content` from the JSONB Value before sending to clients.
/// DB writes go through `MessageContentData` (not `MessageContent`), so storage is unaffected.
fn strip_hidden_content_serialize<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut v = value.clone();
    if let Value::Object(ref mut m) = v {
        m.remove("hidden_content");
    }
    v.serialize(serializer)
}

/// Message content entity - Represents a content block within a message
#[derive(Debug, Clone, FromRow, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MessageContent {
    pub id: Uuid,
    pub message_id: Uuid,
    pub content_type: String,
    #[schemars(with = "MessageContentData")]
    #[serde(serialize_with = "strip_hidden_content_serialize")]
    pub content: Value, // JSONB - contains MessageContentData
    pub sequence_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MessageContent {
    /// Parse the JSONB content into MessageContentData
    pub fn parse_content(&self) -> Result<MessageContentData, AppError> {
        serde_json::from_value(self.content.clone()).map_err(AppError::database_error)
    }
}

/// Content data types - All content types come from extensions via composition
///
/// Extensions add variants by defining MessageContentDataVariants enums in their
/// extension.rs files. The compose_message_content_variants macro merges them at compile time.
#[macros::compose_message_content_variants]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContentData {
    // All variants (Text, Thinking, Image, FileAttachment, ToolUse, ToolResult) are added by extensions
}

impl MessageContentData {
    /// Get the content type string
    pub fn content_type(&self) -> String {
        // All variants are now extension types - use serde to get the type field
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(String::from))
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Serialize to API-facing JSON format
    pub fn to_api_content(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

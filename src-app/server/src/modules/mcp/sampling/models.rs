// MCP Sampling Protocol Types
// https://spec.modelcontextprotocol.io/specification/client/sampling/

use serde::{Deserialize, Serialize};

/// A message in a sampling request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: String, // "user" or "assistant"
    pub content: SamplingContent,
}

/// Content of a sampling message.
///
/// Covers the three content types defined by the MCP sampling spec:
/// - `text`  — plain text
/// - `image` — base64-encoded binary data with MIME type (used for images, PDFs, and
///             other file attachments that MCP servers pass to the LLM for analysis)
/// - Any future/unknown type is captured by `Unknown` for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SamplingContent {
    Text { text: String },
    Image {
        /// Base64-encoded binary data (e.g. a screenshot, chart, or PDF page)
        data: String,
        /// MIME type, e.g. "image/png", "image/jpeg", "application/pdf"
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Forward-compatible fallback: captures any unknown future content type so that
    /// deserialization never fails for types we haven't implemented yet.
    #[serde(other)]
    Unknown,
}

impl SamplingContent {
    /// Returns the text content if this is a `Text` variant, otherwise `None`.
    /// Test-only convenience accessor (no production callers yet).
    #[cfg(test)]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            SamplingContent::Text { text } => Some(text.as_str()),
            _ => None,
        }
    }
}

/// Model preferences for sampling requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f64>,
}

/// Model hint for selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelHint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Request from MCP server to create a message (sampling request)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCreateMessageRequest {
    pub messages: Vec<SamplingMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Result from the platform after handling a sampling request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCreateMessageResult {
    pub role: String,
    pub content: SamplingContent,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampling_content_text_roundtrip() {
        let content = SamplingContent::Text { text: "hello".to_string() };
        let json = serde_json::to_string(&content).unwrap();
        let parsed: SamplingContent = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, SamplingContent::Text { .. }));
        assert_eq!(parsed.as_text(), Some("hello"));
    }

    #[test]
    fn test_sampling_content_image_deserializes() {
        let json = r#"{"type":"image","data":"abc123","mimeType":"image/png"}"#;
        let content: SamplingContent = serde_json::from_str(json).unwrap();
        assert!(matches!(content, SamplingContent::Image { .. }));
        assert!(content.as_text().is_none());
    }

    #[test]
    fn test_sampling_content_unknown_does_not_panic() {
        // Unknown future type should deserialize to Unknown, not fail
        let json = r#"{"type":"document","uri":"file:///doc.pdf"}"#;
        let content: SamplingContent = serde_json::from_str(json).unwrap();
        assert!(matches!(content, SamplingContent::Unknown));
        assert!(content.as_text().is_none());
    }

    #[test]
    fn test_sampling_content_image_as_text_returns_none() {
        let content = SamplingContent::Image {
            data: "abc".to_string(),
            mime_type: "image/png".to_string(),
        };
        assert!(content.as_text().is_none());
    }
}

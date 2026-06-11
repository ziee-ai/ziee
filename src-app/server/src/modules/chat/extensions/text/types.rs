use crate::define_extension_content;

/// Metadata for thinking content
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ThinkingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
    /// Anthropic extended-thinking signature — required to replay the block on a
    /// later turn (`None` for Gemini/OpenAI, whose thinking isn't replayable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Opaque data for a redacted-thinking block (Anthropic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redacted_data: Option<String>,
}

// Define type-safe text content types using the macro
define_extension_content! {
    extension: "text",
    name: TextContent,

    Text {
        text: String,
    } => "text",

    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<ThinkingMetadata>,
    } => "thinking",
}

use crate::define_extension_content;

/// Metadata for thinking content
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ThinkingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u32>,
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

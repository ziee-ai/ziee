//! Unified request/response models for AI providers
//!
//! These types provide a common interface across different AI providers (OpenAI, Anthropic, Gemini).
//! Conversion functions map between these types and provider-specific types.

use super::tools::{Tool, ToolCall, ToolChoice};
use serde::{Deserialize, Serialize};

/// A chat completion request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    /// The model to use (e.g., "gpt-4", "claude-3-opus-20240229", "gemini-2.5-flash")
    pub model: String,

    /// The messages in the conversation
    pub messages: Vec<ChatMessage>,

    /// Temperature controls randomness (0.0 to 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Nucleus sampling threshold (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Whether to stream the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Tools/functions available for the model to call
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,

    /// How the model should use tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Thinking/reasoning configuration (unified across providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
}

/// A message in a chat conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// The role of the message author
    pub role: Role,

    /// Content blocks (text, images, thinking, tool use, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentBlock>,
}

/// A content block (text, image, thinking, tool use, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text {
        text: String,
    },

    /// Thinking/reasoning content (Anthropic, Gemini)
    Thinking {
        thinking: String,
    },

    /// Image content
    Image {
        #[serde(flatten)]
        source: ImageSource,
    },

    /// Tool use request (model wants to call a tool)
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool result (response from tool execution)
    ToolResult {
        tool_use_id: String,
        /// Can contain text, images, or structured data (recursive)
        content: Vec<ContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Image source (URL or base64 data)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data
    Base64 {
        media_type: String,
        data: String,
    },
    /// URL to image
    Url {
        url: String,
        /// OpenAI-specific: image detail level
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

impl ChatMessage {
    /// Creates a system message with text content
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    /// Creates a user message with text content
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    /// Creates an assistant message with text content
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlock::Text {
                text: content.into(),
            }],
        }
    }

    /// Creates a message with custom content blocks
    pub fn with_blocks(role: Role, content: Vec<ContentBlock>) -> Self {
        Self { role, content }
    }

    /// Creates a user message with text and image
    pub fn user_with_image(
        text: impl Into<String>,
        image_data: Vec<u8>,
        media_type: impl Into<String>,
    ) -> Self {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(image_data);

        Self::with_blocks(
            Role::User,
            vec![
                ContentBlock::Text {
                    text: text.into(),
                },
                ContentBlock::Image {
                    source: ImageSource::Base64 {
                        media_type: media_type.into(),
                        data: base64_data,
                    },
                },
            ],
        )
    }

    /// Creates a user message with text and image URL
    pub fn user_with_image_url(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self::with_blocks(
            Role::User,
            vec![
                ContentBlock::Text {
                    text: text.into(),
                },
                ContentBlock::Image {
                    source: ImageSource::Url {
                        url: url.into(),
                        detail: None,
                    },
                },
            ],
        )
    }

    /// Creates a tool result message
    pub fn tool_result(tool_use_id: impl Into<String>, content: Vec<ContentBlock>) -> Self {
        Self::with_blocks(
            Role::Tool,
            vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content,
                is_error: None,
            }],
        )
    }

    /// Creates a tool result with simple text
    pub fn tool_result_text(
        tool_use_id: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::tool_result(
            tool_use_id,
            vec![ContentBlock::Text {
                text: text.into(),
            }],
        )
    }
}

impl ThinkingConfig {
    /// Create a config with just effort level (for OpenAI)
    pub fn with_effort(effort: ThinkingEffort) -> Self {
        Self {
            enabled: true,
            budget_tokens: None,
            effort: Some(effort),
            include_thinking: true,
        }
    }

    /// Create a config with token budget (for Anthropic/Gemini)
    pub fn with_budget(budget_tokens: i32) -> Self {
        Self {
            enabled: true,
            budget_tokens: Some(budget_tokens),
            effort: None,
            include_thinking: true,
        }
    }

    /// Create a config with both effort and budget
    pub fn with_effort_and_budget(effort: ThinkingEffort, budget_tokens: i32) -> Self {
        Self {
            enabled: true,
            budget_tokens: Some(budget_tokens),
            effort: Some(effort),
            include_thinking: true,
        }
    }
}

/// The role of a message author
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System message (instructions)
    System,
    /// User message (human input)
    User,
    /// Assistant message (AI response)
    Assistant,
    /// Tool response message
    Tool,
}

/// A chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique identifier for the response
    pub id: String,

    /// The model that generated the response
    pub model: String,

    /// The generated completions
    pub choices: Vec<Choice>,

    /// Token usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// A single completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// The generated message
    pub message: Message,

    /// The reason the model stopped generating
    pub finish_reason: String,

    /// The index of this choice
    #[serde(default)]
    pub index: u32,
}

/// A message in a response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message author
    pub role: Role,

    /// The content of the message
    pub content: String,

    /// Tool calls made by the assistant
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,

    /// The model's thinking/reasoning process (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u32,

    /// Number of tokens in the completion
    pub completion_tokens: u32,

    /// Total tokens used
    pub total_tokens: u32,

    /// Number of tokens used for reasoning/thinking (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
}

/// A chunk of streamed chat content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChatChunk {
    /// Content block deltas
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<ContentBlockDelta>,

    /// The reason the model stopped (if finished)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// Usage metadata (typically in final chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<StreamUsage>,

    /// Refusal message (when model refuses to respond)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,

    /// Safety ratings (Gemini only)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub safety_ratings: Vec<SafetyRating>,

    /// Whether content was blocked by safety filters
    #[serde(default)]
    pub safety_blocked: bool,
}

/// Incremental content block update in streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text content delta
    TextDelta {
        index: usize,
        delta: String,
    },

    /// Thinking content delta
    ThinkingDelta {
        index: usize,
        delta: String,
    },

    /// Tool use delta (incremental JSON)
    ToolUseDelta {
        index: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_delta: Option<String>,
    },
}

/// An incremental tool call update in a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolCall {
    /// The index of this tool call in the array
    pub index: u32,

    /// The ID of the tool call (appears in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// The type of tool (e.g., "function")
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,

    /// The function name (appears in first chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_name: Option<String>,

    /// Incremental JSON arguments string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments_delta: Option<String>,
}

/// Usage metadata in streaming responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUsage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u32,

    /// Number of tokens in the completion
    pub completion_tokens: u32,

    /// Total tokens used
    pub total_tokens: u32,

    /// Number of tokens used for reasoning/thinking (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
}

/// Safety rating for content (Gemini)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    /// The safety category
    pub category: String,

    /// The probability of harm
    pub probability: String,

    /// Whether this category blocked the content
    #[serde(default)]
    pub blocked: bool,
}

/// An embeddings request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsRequest {
    /// The model to use for embeddings
    pub model: String,

    /// The input text(s) to embed
    pub input: Vec<String>,
}

/// An embeddings response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsResponse {
    /// The generated embeddings (one vector per input)
    pub embeddings: Vec<Vec<f32>>,

    /// Token usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// Configuration for thinking/reasoning mode (unified across providers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether to enable thinking/reasoning mode
    pub enabled: bool,

    /// Maximum tokens to allocate for thinking/reasoning
    /// - OpenAI: Not directly used (controlled by reasoning_effort)
    /// - Anthropic: budget_tokens (minimum 1024)
    /// - Gemini: thinking_budget (-1 for dynamic, 0 to disable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<i32>,

    /// Effort level for reasoning (provider-specific interpretation)
    /// - OpenAI: "minimal", "low", "medium", "high"
    /// - Anthropic: Not used (only budget_tokens matters)
    /// - Gemini: -1 for dynamic (automatic budget adjustment)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ThinkingEffort>,

    /// Whether to include thinking/reasoning content in the response
    /// - OpenAI: Not supported (reasoning tokens are hidden)
    /// - Anthropic: Returns thinking blocks when enabled
    /// - Gemini: include_thoughts parameter
    #[serde(default = "default_include_thinking")]
    pub include_thinking: bool,
}

fn default_include_thinking() -> bool {
    true
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            budget_tokens: None,
            effort: None,
            include_thinking: true,
        }
    }
}

/// Thinking/reasoning effort level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    /// Minimal thinking (OpenAI GPT-5 only)
    Minimal,
    /// Low effort
    Low,
    /// Medium effort (recommended starting point)
    Medium,
    /// High effort (maximum reasoning)
    High,
    /// Dynamic effort (Gemini only - model decides based on complexity)
    Dynamic,
}

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

    /// File attachments (for providers that support multimodal input)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<FileAttachment>,

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

    /// The text content of the message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls made by the assistant
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,

    /// Tool call ID (for tool response messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Creates a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// Creates a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// Creates an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    /// Creates an assistant message with tool calls
    pub fn assistant_with_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
        }
    }

    /// Creates a tool response message
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
        }
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
    /// The delta content
    pub content: String,

    /// The reason the model stopped (if finished)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,

    /// The thinking/reasoning delta (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

/// File attachment for multimodal requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAttachment {
    /// The filename
    pub filename: String,

    /// The file content (raw bytes)
    pub content: Vec<u8>,

    /// The MIME type (e.g., "image/png", "application/pdf")
    pub mime_type: String,
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

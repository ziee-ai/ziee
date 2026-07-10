//! Unified request/response models for AI providers
//!
//! These types provide a common interface across different AI providers (OpenAI, Anthropic, Gemini).
//! Conversion functions map between these types and provider-specific types.

use super::tools::{Tool, ToolChoice};
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

    /// Top-k sampling. Supported by Anthropic + Gemini; ignored by OpenAI
    /// (Chat Completions has no top_k). Anthropic drops it for models that
    /// reject sampling params (Opus 4.7/4.8 family).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,

    /// Frequency penalty (-2.0..2.0). OpenAI + Gemini; Anthropic doesn't support it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0..2.0). OpenAI + Gemini; Anthropic doesn't support it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Random seed for reproducible sampling. OpenAI + Gemini; not Anthropic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,

    /// Stop sequences. OpenAI `stop`, Anthropic `stop_sequences`, Gemini `stopSequences`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// End-user identifier for provider abuse monitoring. OpenAI `user`,
    /// Anthropic `metadata.user_id`; Gemini has no equivalent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Disable Anthropic prompt-cache breakpoints for this request. Default
    /// `false` (caching is auto-on for Anthropic). OpenAI/Gemini cache
    /// automatically regardless of this flag.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_prompt_cache: bool,

    /// OpenAI `prompt_cache_key` — routes requests with a shared prefix to the
    /// same cache. Passed through to OpenAI only; ignored elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,

    /// Per-model capability override, resolved from the editable DB model row by
    /// the server and threaded down as the top-priority source of the parameter
    /// contract (see `param_policy`). `None` ⇒ the provider adapter falls back to
    /// the curated catalog + model-family policy, so non-chat callers need not
    /// set it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_caps: Option<crate::param_policy::ModelParamContract>,
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
        /// Anthropic extended-thinking signature. Required to replay a thinking
        /// block on a later turn; a signature-less thinking block is NOT sent
        /// back to Anthropic (it would be rejected). `None` for Gemini/OpenAI,
        /// whose thinking is not replayable.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    /// Redacted thinking block (Anthropic) — opaque encrypted reasoning that
    /// must be replayed verbatim on later turns.
    RedactedThinking {
        data: String,
    },

    /// Image content
    Image {
        #[serde(flatten)]
        source: ImageSource,
    },

    /// Document content (PDFs, etc.)
    Document {
        #[serde(flatten)]
        source: DocumentSource,
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
        /// Function/tool name (required for some providers like Gemini)
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// Can contain text, images, or structured data (recursive)
        content: Vec<ContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Image source (URL, base64 data, or provider file reference)
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
    /// Provider file reference (Anthropic/Gemini Files API)
    File {
        file_id: String,
        /// Original MIME type of the uploaded image (e.g. `image/png`). Lets
        /// providers that echo the MIME on a file reference (Gemini `FileData`)
        /// send the real type instead of a hard-coded default. `None` when the
        /// caller didn't carry it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
    },
}

/// Document source for PDFs and other documents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DocumentSource {
    /// Base64-encoded document data
    Base64 {
        media_type: String,
        data: String,
    },
    /// URL to document
    Url {
        url: String,
    },
    /// Provider file reference (Anthropic/Gemini Files API)
    File {
        file_id: String,
        /// Original MIME type of the uploaded document (e.g. `application/pdf`).
        /// `None` when the caller didn't carry it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
    },
}

/// File upload request for provider Files APIs
#[derive(Debug, Clone)]
pub struct FileUpload {
    /// Original filename
    pub filename: String,
    /// Raw file data
    pub file_data: Vec<u8>,
    /// MIME type (e.g., "application/pdf", "image/jpeg")
    pub mime_type: String,
}

/// File upload response from provider Files API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadResponse {
    /// Provider's file ID or URI
    pub provider_file_id: String,

    /// When the file expires (None = no expiration)
    /// - Anthropic: None (no expiration)
    /// - Gemini: Some(timestamp) - 48 hours from upload
    /// - OpenAI: None (no expiration for Assistants API)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Provider-specific metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
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
    pub fn tool_result(tool_use_id: impl Into<String>, name: Option<String>, content: Vec<ContentBlock>) -> Self {
        Self::with_blocks(
            Role::Tool,
            vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                name,
                content,
                is_error: None,
            }],
        )
    }

    /// Creates a tool result with simple text
    pub fn tool_result_text(
        tool_use_id: impl Into<String>,
        name: Option<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::tool_result(
            tool_use_id,
            name,
            vec![ContentBlock::Text {
                text: text.into(),
            }],
        )
    }
}

impl ThinkingConfig {
    /// Adaptive thinking — the model decides when/how much to think. The modern
    /// default for current Anthropic models (Opus 4.6+) and Gemini 2.5.
    pub fn adaptive() -> Self {
        Self {
            mode: ThinkingMode::Adaptive,
            budget_tokens: None,
            effort: None,
            include_thinking: true,
        }
    }

    /// Adaptive thinking with an effort level (Anthropic `output_config.effort`,
    /// OpenAI `reasoning_effort`).
    pub fn adaptive_with_effort(effort: ThinkingEffort) -> Self {
        Self {
            mode: ThinkingMode::Adaptive,
            budget_tokens: None,
            effort: Some(effort),
            include_thinking: true,
        }
    }

    /// Thinking explicitly disabled.
    pub fn disabled() -> Self {
        Self {
            mode: ThinkingMode::Disabled,
            budget_tokens: None,
            effort: None,
            include_thinking: false,
        }
    }

    /// Create a config with just an effort level (drives OpenAI `reasoning_effort`
    /// and Anthropic `output_config.effort`; treated as adaptive thinking).
    pub fn with_effort(effort: ThinkingEffort) -> Self {
        Self::adaptive_with_effort(effort)
    }

    /// Create a legacy fixed-budget config (`{type:"enabled", budget_tokens}`),
    /// for older Anthropic models (Opus 4.6 and earlier) and Gemini.
    pub fn with_budget(budget_tokens: i32) -> Self {
        Self {
            mode: ThinkingMode::Enabled,
            budget_tokens: Some(budget_tokens),
            effort: None,
            include_thinking: true,
        }
    }

    /// Legacy fixed-budget config with an effort hint.
    pub fn with_effort_and_budget(effort: ThinkingEffort, budget_tokens: i32) -> Self {
        Self {
            mode: ThinkingMode::Enabled,
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

    /// Prompt tokens served from cache this request (Anthropic
    /// `cache_read_input_tokens`, OpenAI `prompt_tokens_details.cached_tokens`,
    /// Gemini `cachedContentTokenCount`). The cache-hit signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,

    /// Prompt tokens written to cache this request (Anthropic
    /// `cache_creation_input_tokens`). `None` for providers that don't report it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
}

/// Unified, provider-agnostic finish reason. Each provider adapter maps its own
/// wire vocabulary into this via `FinishReason::from_provider`; the canonical
/// string (`as_canonical_str`) is what flows out on `StreamChatChunk.finish_reason`
/// so downstream (chat loop, MCP) sees one vocabulary regardless of provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishReason {
    /// Natural end of turn.
    Stop,
    /// Hit the max-output-token cap.
    Length,
    /// Stopped to call tools.
    ToolCalls,
    /// Blocked by a safety/content filter.
    ContentFilter,
    /// The model refused to respond.
    Refusal,
    /// An unrecognized provider value, preserved verbatim.
    Other(String),
}

impl FinishReason {
    /// Map a provider's raw finish-reason token into the canonical enum.
    /// `family` selects the provider vocabulary; unknown tokens become `Other`.
    pub fn from_provider(family: crate::param_policy::ProviderFamily, raw: &str) -> Self {
        use crate::param_policy::ProviderFamily;
        match family {
            ProviderFamily::Anthropic => match raw {
                "end_turn" | "stop_sequence" => Self::Stop,
                "tool_use" => Self::ToolCalls,
                "max_tokens" => Self::Length,
                "refusal" => Self::Refusal,
                other => Self::Other(other.to_string()),
            },
            ProviderFamily::OpenAiCompat => match raw {
                "stop" => Self::Stop,
                "length" => Self::Length,
                "tool_calls" | "function_call" => Self::ToolCalls,
                "content_filter" => Self::ContentFilter,
                other => Self::Other(other.to_string()),
            },
            ProviderFamily::Gemini => match raw {
                "STOP" => Self::Stop,
                "MAX_TOKENS" => Self::Length,
                "SAFETY" | "RECITATION" | "PROHIBITED_CONTENT" | "BLOCKLIST" => Self::ContentFilter,
                other => Self::Other(other.to_string()),
            },
        }
    }

    /// The canonical wire string emitted on `StreamChatChunk.finish_reason`.
    pub fn as_canonical_str(&self) -> &str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ToolCalls => "tool_calls",
            Self::ContentFilter => "content_filter",
            Self::Refusal => "refusal",
            Self::Other(s) => s.as_str(),
        }
    }

    /// Convenience: map a raw provider token straight to its canonical string.
    pub fn canonicalize(family: crate::param_policy::ProviderFamily, raw: &str) -> String {
        Self::from_provider(family, raw).as_canonical_str().to_string()
    }
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

    /// Thinking-block signature (Anthropic `signature_delta`). Arrives in its
    /// own SSE event after the thinking text; carries the signature needed to
    /// replay the block on a later turn.
    ThinkingSignatureDelta {
        index: usize,
        signature: String,
    },

    /// Redacted thinking block start (Anthropic `redacted_thinking`). Carries
    /// the opaque `data` that must be replayed verbatim.
    RedactedThinkingDelta {
        index: usize,
        data: String,
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

    /// Prompt tokens served from cache this request (Anthropic
    /// `cache_read_input_tokens`, OpenAI `prompt_tokens_details.cached_tokens`,
    /// Gemini `cachedContentTokenCount`). The cache-hit signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,

    /// Prompt tokens written to cache this request (Anthropic
    /// `cache_creation_input_tokens`). `None` for providers that don't report it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
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

/// Which thinking mode to request.
/// - `Adaptive`: model decides when/how much to think (Anthropic `{type:"adaptive"}`,
///   the only on-mode for Opus 4.7/4.8; Gemini dynamic). Depth tuned by `effort`.
/// - `Enabled`: legacy fixed token budget (Anthropic `{type:"enabled", budget_tokens}`,
///   only valid on Opus 4.6 and earlier; Gemini explicit budget).
/// - `Disabled`: thinking off.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingMode {
    Adaptive,
    Enabled,
    Disabled,
}

/// Configuration for thinking/reasoning mode (unified across providers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Thinking mode (adaptive / enabled / disabled).
    pub mode: ThinkingMode,

    /// Maximum tokens to allocate for thinking/reasoning. Used by the legacy
    /// `Enabled` mode only.
    /// - OpenAI: Not used (controlled by reasoning_effort)
    /// - Anthropic: `Enabled` mode `budget_tokens` (min 1024); ignored in `Adaptive`
    ///   mode (use `effort` instead — Opus 4.7/4.8 removed fixed budgets)
    /// - Gemini: thinking_budget (-1 for dynamic, 0 to disable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<i32>,

    /// Effort level for reasoning (provider-specific interpretation)
    /// - OpenAI: `reasoning_effort` ("minimal"/"low"/"medium"/"high")
    /// - Anthropic: `output_config.effort` in `Adaptive` mode
    ///   ("low"/"medium"/"high"/"xhigh"/"max")
    /// - Gemini: maps to the thinking budget
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
            mode: ThinkingMode::Adaptive,
            budget_tokens: None,
            effort: None,
            include_thinking: true,
        }
    }
}

/// Thinking/reasoning effort level.
/// Anthropic `output_config.effort`: low | medium | high | xhigh | max
/// (Minimal→low, Dynamic→omit). OpenAI `reasoning_effort`: minimal | low | medium
/// | high (XHigh/Max→high). Gemini: maps to thinking budget.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    /// Minimal thinking (OpenAI GPT-5 only)
    Minimal,
    /// Low effort
    Low,
    /// Medium effort (recommended starting point)
    Medium,
    /// High effort
    High,
    /// Extra-high effort (Anthropic Opus 4.7/4.8 — between high and max)
    XHigh,
    /// Maximum reasoning (Anthropic Opus-tier only)
    Max,
    /// Dynamic effort (Gemini only - model decides based on complexity)
    Dynamic,
}

#[cfg(test)]
mod finish_reason_tests {
    use super::FinishReason;
    use crate::param_policy::ProviderFamily;

    // TEST-10: per-provider finish-reason tables + canonical strings.
    #[test]
    fn canonicalizes_per_provider() {
        // Anthropic.
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Anthropic, "end_turn"), "stop");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Anthropic, "stop_sequence"), "stop");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Anthropic, "tool_use"), "tool_calls");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Anthropic, "max_tokens"), "length");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Anthropic, "refusal"), "refusal");
        // OpenAI (mostly passthrough of the canonical vocabulary).
        assert_eq!(FinishReason::canonicalize(ProviderFamily::OpenAiCompat, "stop"), "stop");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::OpenAiCompat, "length"), "length");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::OpenAiCompat, "tool_calls"), "tool_calls");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::OpenAiCompat, "content_filter"), "content_filter");
        // Gemini.
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Gemini, "STOP"), "stop");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Gemini, "MAX_TOKENS"), "length");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Gemini, "SAFETY"), "content_filter");
        assert_eq!(FinishReason::canonicalize(ProviderFamily::Gemini, "RECITATION"), "content_filter");
        // Unknown token is preserved verbatim.
        assert_eq!(FinishReason::canonicalize(ProviderFamily::OpenAiCompat, "weird"), "weird");
        assert_eq!(
            FinishReason::from_provider(ProviderFamily::Anthropic, "mystery"),
            FinishReason::Other("mystery".to_string())
        );
    }
}

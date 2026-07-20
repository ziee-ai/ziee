use crate::core::Repos;
// Title generation extension implementation

use async_trait::async_trait;
use axum::response::sse::Event;
use futures_util::StreamExt;
use sqlx::PgPool;
use std::convert::Infallible;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Provider, Role};

use crate::common::AppError;
use crate::modules::chat::core::{
    extension::{ChatExtension, ExtensionAction, StreamContext},
    models::{Message, MessageContent},
    types::{MessageWithContent, streaming::SSEChatStreamEvent},
};
use crate::modules::chat::extensions::title::extension::SSEChatStreamTitleUpdatedData;

/// Token budget for the title request.
///
/// Must be generous enough for a REASONING model. Such models spend tokens on a
/// chain of thought BEFORE emitting any answer text: DeepSeek/gpt-oss-style
/// servers stream it on the `reasoning_content` channel (which the provider maps
/// to `ContentBlockDelta::ThinkingDelta`, discarded below), and OpenAI reasoning
/// models bill hidden reasoning against the very same cap (`max_tokens` is
/// remapped to `max_completion_tokens`, a COMBINED reasoning+output budget).
///
/// The previous 50-token budget was consumed entirely by reasoning on
/// `openai/gpt-oss-120b`: the stream ended with `finish_reason: "length"` having
/// emitted zero text, so title generation "failed" on every single chat through
/// that provider. 512 leaves ample room for a short reasoning preamble plus a
/// six-word title, and a title is a once-per-conversation call, so the cost is
/// negligible.
const TITLE_MAX_TOKENS: u32 = 512;

/// Maximum length of a stored title, in characters.
const TITLE_MAX_CHARS: usize = 50;

/// Stop attempting title generation once the conversation has grown past this
/// many user+assistant messages.
///
/// A failed generation deliberately leaves the title unset (we never persist a
/// wrong title), so it retries on the next turn. This bounds that retry: a
/// permanently misconfigured provider costs a few extra calls, not one per turn
/// for the life of the conversation.
const TITLE_RETRY_MESSAGE_LIMIT: usize = 6;

/// Extract the text of a message content block, if it is a text block.
///
/// Content blocks are extension types stored as JSON, so the block is
/// identified by its serialized `type` discriminant rather than a Rust variant.
fn content_block_text(content: &MessageContent) -> Option<String> {
    let data = content
        .parse_content()
        .inspect_err(|e| {
            // Not merely "this isn't a text block" — the block failed to
            // deserialize at all. Silence here would make a schema drift look
            // like "no conversation is ever titled again" with no explanation.
            tracing::debug!(
                content_type = %content.content_type,
                "title: skipping an unparseable message content block: {e}"
            );
        })
        .ok()?;
    let value = serde_json::to_value(&data).ok()?;
    if value.get("type")?.as_str()? == "text" {
        value.get("text")?.as_str().map(|s| s.to_string())
    } else {
        None
    }
}

/// True when `message` is an assistant turn that has produced user-visible
/// output — either a non-empty text answer, or a `tool_result` whose content is
/// itself the answer.
///
/// The `tool_result` arm is what allows an `audience:["user"]` tool (whose
/// result IS the final answer, bypassing a second LLM round-trip) to be titled
/// at all: the MCP extension appends those tool_results BEFORE returning
/// `CompleteWithContent`, so the turn's answer is already on the row here.
/// Without it such a conversation stays untitled forever.
///
/// It never fires on the FIRST iteration of a tool loop — streaming appends an
/// intermediate iteration's tool_results AFTER this hook returns, so the
/// assistant row carries `tool_use` blocks only. From the second iteration on it
/// can fire, since the previous iteration's results are now persisted. That is
/// intentional: the title is derived from the USER's first message, so an
/// in-loop title is identical to the one computed at turn end, just available
/// sooner. The `title.is_some()` guard keeps it single-shot either way.
fn assistant_produced_output(message: &MessageWithContent) -> bool {
    if message.message.role != "assistant" {
        return false;
    }
    message.contents.iter().any(|c| {
        content_block_text(c).is_some_and(|t| !t.trim().is_empty())
            || c.content_type == "tool_result"
    })
}

/// Decide whether the title extension should generate a title now.
///
/// Extracted as a pure function (mirroring `project::apply_project_context`) so
/// the gating logic is unit-testable without Postgres or an LLM provider.
///
/// Fires when the conversation has no title yet AND the first assistant ANSWER
/// exists. Note it deliberately does NOT count messages exactly: a tool-calling
/// turn appends its `tool_use`/`tool_result` blocks to the SAME assistant
/// message row, and requiring a visible text block means an intermediate
/// tool-call step cannot trigger a premature title.
///
/// Bounded by [`TITLE_RETRY_MESSAGE_LIMIT`] so a failing provider is retried on
/// the next few turns and then left alone.
/// True when the conversation already carries a usable title.
///
/// A whitespace-only title counts as absent: it renders as a blank sidebar row,
/// which is strictly worse than the "Untitled Conversation" placeholder.
fn has_title(existing_title: Option<&str>) -> bool {
    existing_title.is_some_and(|t| !t.trim().is_empty())
}

fn should_generate_title(history: &[MessageWithContent], existing_title: Option<&str>) -> bool {
    // Already titled — the single-shot guard.
    if has_title(existing_title) {
        return false;
    }

    let dialogue_messages = history
        .iter()
        .filter(|m| m.message.role == "user" || m.message.role == "assistant")
        .count();

    // Retry budget exhausted.
    if dialogue_messages > TITLE_RETRY_MESSAGE_LIMIT {
        return false;
    }

    let has_user = history.iter().any(|m| m.message.role == "user");
    let has_answer = history.iter().any(assistant_produced_output);

    has_user && has_answer
}

/// First text content of the first user message in the history.
fn first_user_text(history: &[MessageWithContent]) -> Option<String> {
    history
        .iter()
        .find(|m| m.message.role == "user")?
        .contents
        .iter()
        .find_map(content_block_text)
}

/// Normalize a model-generated title, or `None` when the model produced nothing
/// usable.
///
/// Returning `None` (rather than an empty string) is what stops an empty
/// generation from being persisted as a title.
fn clean_generated_title(raw: &str) -> Option<String> {
    // Collapse ALL internal whitespace (a model that ignores "respond with only
    // the title" can emit newlines; a title is a single line by definition).
    // With the larger TITLE_MAX_TOKENS budget a verbose model has room to be
    // chatty, so this matters more than it did under the old 50-token cap.
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    // Strip markdown emphasis and surrounding quotes. Each pass re-trims, so a
    // mixed wrapping like `" 'Title' "` unwraps fully.
    let cleaned = collapsed
        .trim()
        .trim_matches('"')
        .trim()
        .trim_matches('\'')
        .trim()
        .trim_matches('*')
        .trim()
        .chars()
        .take(TITLE_MAX_CHARS)
        .collect::<String>();
    let cleaned = cleaned.trim();

    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

/// True for the finish reasons meaning "the token budget ran out".
///
/// The value is the RAW provider string (canonicalization happens later, at the
/// chat SSE boundary), so each family spells it differently: OpenAI `length`,
/// Anthropic `max_tokens`, Gemini `MAX_TOKENS`.
fn is_budget_exhausted(finish_reason: &str) -> bool {
    finish_reason.eq_ignore_ascii_case("length") || finish_reason.eq_ignore_ascii_case("max_tokens")
}

/// Build the (tool-less) chat request used to generate a title.
///
/// Extracted so the token budget and prompt shape are unit-testable without a
/// provider — the budget in particular is the root-cause fix and must not be
/// silently reverted.
fn build_title_request(model_name: &str, user_content: &str) -> ChatRequest {
    let title_prompt = format!(
        "Generate a concise, descriptive title (maximum 6 words) for a conversation that starts with this message: \"{}\"\n\nRespond with only the title, no quotes or additional text.",
        user_content.chars().take(200).collect::<String>()
    );

    ChatRequest {
        model: model_name.to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: title_prompt }],
        }],
        temperature: Some(0.7),
        max_tokens: Some(TITLE_MAX_TOKENS),
        ..Default::default()
    }
}

/// Title generation extension
///
/// Generates conversation titles automatically after the first message exchange.
pub struct TitleGenerationExtension {}

impl TitleGenerationExtension {
    pub fn new(_pool: PgPool) -> Self {
        Self {}
    }

    /// Generate title using AI
    async fn generate_title_with_ai(
        &self,
        provider: &Provider,
        model_name: &str,
        user_content: &str,
    ) -> Result<String, AppError> {
        // Call AI provider and collect the stream
        let mut stream = provider
            .chat_stream(build_title_request(model_name, user_content))
            .await
            .map_err(|e| AppError::internal_error(format!("AI provider error: {}", e)))?;

        // Collect all chunks into a single string
        let mut full_content = String::new();
        let mut finish_reason: Option<String> = None;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| AppError::internal_error(format!("Stream error: {}", e)))?;

            if chunk.finish_reason.is_some() {
                finish_reason = chunk.finish_reason.clone();
            }

            // Extract text from content deltas. Reasoning (`ThinkingDelta`) is
            // deliberately NOT collected — a chain of thought makes a terrible
            // title; see TITLE_MAX_TOKENS for why the budget must accommodate it
            // anyway.
            for delta in &chunk.content {
                match delta {
                    ai_providers::ContentBlockDelta::TextDelta { delta, .. } => {
                        full_content.push_str(delta);
                    }
                    _ => {} // Ignore non-text deltas for title generation
                }
            }
        }

        clean_generated_title(&full_content).ok_or_else(|| match finish_reason.as_deref() {
            // The budget ran out before the model emitted any answer text —
            // characteristic of a reasoning model.
            Some(reason) if is_budget_exhausted(reason) => AppError::internal_error(format!(
                "generated title is empty: the model exhausted the {}-token budget \
                 (finish_reason={}) without emitting answer text",
                TITLE_MAX_TOKENS, reason
            )),
            Some(reason) => AppError::internal_error(format!(
                "generated title is empty (finish_reason={})",
                reason
            )),
            None => AppError::internal_error("generated title is empty"),
        })
    }

    /// Resolve the provider from the stream context and generate a title.
    ///
    /// Split out so that EVERY failure mode below (missing context metadata,
    /// provider lookup, the LLM call itself) lands on one soft-failure path in
    /// `after_llm_call` instead of persisting a bogus title.
    async fn resolve_and_generate(
        &self,
        context: &StreamContext,
        user_content: &str,
    ) -> Result<String, AppError> {
        // Get model name and IDs from context metadata
        let model_name = context
            .metadata
            .get("model_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Model name not in context"))?;

        // Get provider type from context
        let provider_type = context
            .metadata
            .get("provider_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Provider type not in context"))?;

        // Get provider_id from context
        let provider_id_str = context
            .metadata
            .get("provider_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::internal_error("Provider ID not in context"))?;

        let provider_id = uuid::Uuid::parse_str(provider_id_str)
            .map_err(|_| AppError::internal_error("Invalid provider ID in context"))?;

        // Fetch provider from database for api_key and base_url (not in context for security)
        let provider_info = Repos
            .llm_provider
            .get_by_id(provider_id)
            .await
            .map_err(AppError::database_error)?
            .ok_or_else(|| AppError::internal_error("Provider not found"))?;

        // Get API key and base URL
        let api_key = provider_info.api_key.as_deref().unwrap_or("");
        let base_url = provider_info.base_url.as_deref().ok_or_else(|| {
            AppError::internal_error(format!(
                "Provider '{}' has no base_url configured",
                provider_info.name
            ))
        })?;

        // Create provider for title generation
        let provider = Provider::new(provider_type, api_key, base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create provider: {}", e)))?;

        self.generate_title_with_ai(&provider, model_name, user_content)
            .await
    }

    /// Send title updated event via SSE
    fn send_title_event(
        &self,
        title: &str,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) {
        if let Some(tx) = tx {
            let event = SSEChatStreamEvent::TitleUpdated(SSEChatStreamTitleUpdatedData {
                title: title.to_string(),
            });

            if let Err(e) = tx.send(Ok(event.into())) {
                tracing::error!("ERROR: Failed to send titleUpdated event: {:?}", e);
            }
        }
    }
}

#[async_trait]
impl ChatExtension for TitleGenerationExtension {
    fn name(&self) -> &str {
        "title-generation"
    }

    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        tracing::info!("Title generation extension initialized");
        Ok(())
    }

    async fn after_llm_call(
        &self,
        context: &StreamContext,
        _final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        // Check if conversation needs a title
        let conversation = Repos
            .chat
            .core
            .get_conversation(context.conversation_id, context.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Conversation"))?;

        // Cheap guard FIRST: `get_conversation_history` is an unbounded
        // full-branch load (2 queries, every message + every content block) and
        // this hook runs on every assistant turn for the life of the
        // conversation. A titled conversation must never pay for it.
        if has_title(conversation.title.as_deref()) {
            return Ok(ExtensionAction::Complete);
        }

        let history = Repos
            .chat
            .core
            .get_conversation_history(context.branch_id)
            .await?;

        if !should_generate_title(&history, conversation.title.as_deref()) {
            return Ok(ExtensionAction::Complete);
        }

        let Some(user_content) = first_user_text(&history) else {
            // No text to summarize (e.g. an attachment-only first message).
            return Ok(ExtensionAction::Complete);
        };

        // A title is a nice-to-have: never fail the chat turn over it, and never
        // persist a placeholder derived from the user's own message. Leaving the
        // title unset means `should_generate_title` retries on the next turn.
        let title = match self.resolve_and_generate(context, &user_content).await {
            Ok(title) => title,
            Err(e) => {
                tracing::warn!(
                    conversation_id = %context.conversation_id,
                    "Title generation failed; leaving the title unset to retry on a later turn: {}",
                    e
                );
                return Ok(ExtensionAction::Complete);
            }
        };

        // Update conversation title.
        Repos
            .chat
            .core
            .update_conversation(
                context.conversation_id,
                context.user_id,
                Some(Some(title.clone())),
            )
            .await?;

        // Send title event
        self.send_title_event(&title, tx);

        Ok(ExtensionAction::Complete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::chat::core::models::MessageContent;
    use crate::modules::chat::core::models::message::Message as CoreMessage;
    use uuid::Uuid;

    fn content_block(value: serde_json::Value) -> MessageContent {
        MessageContent {
            id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
            content_type: value
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("text")
                .to_string(),
            content: value,
            sequence_order: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn message_with(role: &str, blocks: Vec<serde_json::Value>) -> MessageWithContent {
        MessageWithContent {
            message: CoreMessage {
                id: Uuid::new_v4(),
                role: role.to_string(),
                originated_from_id: Uuid::new_v4(),
                edit_count: 0,
                model_id: None,
                created_at: chrono::Utc::now(),
            },
            contents: blocks.into_iter().map(content_block).collect(),
        }
    }

    fn text(t: &str) -> serde_json::Value {
        serde_json::json!({ "type": "text", "text": t })
    }

    /// The exact shape a tool-calling turn produces: ziee's
    /// single-assistant-message architecture appends the `tool_use`,
    /// `tool_result` and final text blocks to ONE assistant message row.
    fn tool_calling_first_turn() -> Vec<MessageWithContent> {
        vec![
            message_with("user", vec![text("What is known about BRCA1?")]),
            message_with(
                "assistant",
                vec![
                    serde_json::json!({
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "biognosia_search",
                        "input": { "query": "BRCA1" }
                    }),
                    serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "content": "…results…"
                    }),
                    text("BRCA1 is a tumor suppressor gene."),
                ],
            ),
        ]
    }

    // ---- should_generate_title -------------------------------------------

    #[test]
    fn fires_on_a_tool_calling_first_turn() {
        // The regression guard for the reported bug: a first turn that made MCP
        // tool calls must still be titled.
        assert!(should_generate_title(&tool_calling_first_turn(), None));
    }

    #[test]
    fn fires_on_a_plain_first_turn() {
        // Cross-model regression guard: the already-working non-reasoning path
        // must keep firing.
        let history = vec![
            message_with("user", vec![text("hello there")]),
            message_with("assistant", vec![text("Hi! How can I help?")]),
        ];
        assert!(should_generate_title(&history, None));
    }

    #[test]
    fn does_not_fire_when_a_title_already_exists() {
        // The single-shot guard: never regenerate over an existing title.
        let history = tool_calling_first_turn();
        assert!(!should_generate_title(&history, Some("An Existing Title")));
    }

    #[test]
    fn treats_a_blank_title_as_absent() {
        let history = tool_calling_first_turn();
        assert!(should_generate_title(&history, Some("   ")));
    }

    #[test]
    fn does_not_fire_before_an_assistant_answer_exists() {
        // Only the user has spoken.
        let history = vec![message_with("user", vec![text("hello")])];
        assert!(!should_generate_title(&history, None));
    }

    #[test]
    fn does_not_fire_on_a_tool_call_step_with_no_answer_text_yet() {
        // The assistant message row is created BEFORE the tool loop runs, so
        // mid-loop it exists but carries only tool_use blocks. Requiring a
        // visible text block is what stops a premature title here.
        let history = vec![
            message_with("user", vec![text("What is known about BRCA1?")]),
            message_with(
                "assistant",
                vec![serde_json::json!({
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "biognosia_search",
                    "input": {}
                })],
            ),
        ];
        assert!(!should_generate_title(&history, None));
    }

    #[test]
    fn fires_for_an_audience_user_tool_whose_result_is_the_answer() {
        // The `audience:["user"]` shape (e.g. BioGnosia's `query_rag`): the tool
        // result IS the final answer and the LLM is bypassed, so the assistant
        // row never gets a text block for this turn. The MCP extension appends
        // the tool_result BEFORE returning CompleteWithContent, so it is present
        // when the title extension runs. Without this, such a conversation stays
        // "Untitled Conversation" forever.
        let history = vec![
            message_with("user", vec![text("What does the KB say about TP53?")]),
            message_with(
                "assistant",
                vec![
                    serde_json::json!({
                        "type": "tool_use",
                        "id": "toolu_1",
                        "name": "query_rag",
                        "input": { "query": "TP53" }
                    }),
                    serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": "toolu_1",
                        "name": "query_rag",
                        "content": "TP53 is the most frequently mutated gene…"
                    }),
                ],
            ),
        ];
        assert!(should_generate_title(&history, None));
    }

    #[test]
    fn ignores_a_whitespace_only_assistant_answer() {
        let history = vec![
            message_with("user", vec![text("hello")]),
            message_with("assistant", vec![text("   ")]),
        ];
        assert!(!should_generate_title(&history, None));
    }

    #[test]
    fn retries_on_a_later_turn_while_within_the_bound() {
        // A previous turn failed to generate (title still unset) — the next turn
        // must try again. The old `message_count != 2` guard made this
        // impossible, permanently stranding the conversation untitled.
        let history = vec![
            message_with("user", vec![text("first")]),
            message_with("assistant", vec![text("first answer")]),
            message_with("user", vec![text("second")]),
            message_with("assistant", vec![text("second answer")]),
        ];
        assert!(should_generate_title(&history, None));
    }

    #[test]
    fn still_retries_exactly_at_the_bound() {
        // Boundary guard: `>` (not `>=`) is what gives three attempts. Tightening
        // it to `>=` would silently drop the third retry, and a test that only
        // covers 4-and-8 messages would not notice.
        let mut history = Vec::new();
        for i in 0..(TITLE_RETRY_MESSAGE_LIMIT / 2) {
            history.push(message_with("user", vec![text(&format!("q{i}"))]));
            history.push(message_with("assistant", vec![text(&format!("a{i}"))]));
        }
        assert_eq!(history.len(), TITLE_RETRY_MESSAGE_LIMIT);
        assert!(should_generate_title(&history, None));
    }

    #[test]
    fn stops_retrying_past_the_bound() {
        // Bounded retry: a permanently broken provider must not cost an extra
        // LLM call on every turn forever.
        let mut history = Vec::new();
        for i in 0..4 {
            history.push(message_with("user", vec![text(&format!("q{i}"))]));
            history.push(message_with("assistant", vec![text(&format!("a{i}"))]));
        }
        assert!(history.len() > TITLE_RETRY_MESSAGE_LIMIT);
        assert!(!should_generate_title(&history, None));
    }

    #[test]
    fn system_messages_do_not_count_toward_the_retry_bound() {
        let mut history = vec![message_with("system", vec![text("sys")])];
        history.extend(tool_calling_first_turn());
        assert!(should_generate_title(&history, None));
    }

    // ---- first_user_text --------------------------------------------------

    #[test]
    fn first_user_text_reads_the_first_user_message() {
        let history = tool_calling_first_turn();
        assert_eq!(
            first_user_text(&history).as_deref(),
            Some("What is known about BRCA1?")
        );
    }

    #[test]
    fn first_user_text_skips_non_text_blocks() {
        let history = vec![message_with(
            "user",
            vec![
                serde_json::json!({ "type": "image", "url": "http://x/y.png" }),
                text("describe this"),
            ],
        )];
        assert_eq!(first_user_text(&history).as_deref(), Some("describe this"));
    }

    #[test]
    fn first_user_text_is_none_without_text_content() {
        let history = vec![message_with(
            "user",
            vec![serde_json::json!({ "type": "image", "url": "http://x/y.png" })],
        )];
        assert_eq!(first_user_text(&history), None);
    }

    // ---- clean_generated_title -------------------------------------------

    #[test]
    fn clean_title_strips_quotes_and_whitespace() {
        assert_eq!(
            clean_generated_title("  \"BRCA1 in Breast Cancer\"  ").as_deref(),
            Some("BRCA1 in Breast Cancer")
        );
        assert_eq!(
            clean_generated_title("'Single Quoted'").as_deref(),
            Some("Single Quoted")
        );
    }

    #[test]
    fn clean_title_truncates_to_the_max() {
        let long = "x".repeat(200);
        let cleaned = clean_generated_title(&long).expect("non-empty");
        assert_eq!(cleaned.chars().count(), TITLE_MAX_CHARS);
    }

    #[test]
    fn clean_title_counts_characters_not_bytes() {
        // Multibyte safety: 60 multibyte chars must truncate to 50 CHARS.
        let long = "é".repeat(60);
        let cleaned = clean_generated_title(&long).expect("non-empty");
        assert_eq!(cleaned.chars().count(), TITLE_MAX_CHARS);
    }

    // ---- build_title_request ---------------------------------------------

    #[test]
    fn title_request_carries_the_reasoning_safe_budget() {
        // Pins the root-cause fix. The former 50-token budget was consumed
        // entirely by `reasoning_content` on openai/gpt-oss-120b, the stream
        // ended with finish_reason=length having emitted no text, and the
        // conversation was permanently titled with the raw user message.
        let req = build_title_request("some-model", "What is known about BRCA1?");
        assert_eq!(req.max_tokens, Some(TITLE_MAX_TOKENS));
        assert!(
            TITLE_MAX_TOKENS >= 256,
            "budget must clear a reasoning preamble plus a short title"
        );
        assert!(req.tools.is_empty(), "title generation must not offer tools");
        assert_eq!(req.model, "some-model");
    }

    #[test]
    fn title_request_truncates_a_very_long_user_message() {
        let long = "x".repeat(5_000);
        let req = build_title_request("m", &long);
        let ContentBlock::Text { text } = &req.messages[0].content[0] else {
            panic!("expected a text block");
        };
        // 200 chars of user content plus the fixed preamble/suffix.
        assert!(text.len() < 700, "prompt must not embed the whole message");
    }

    #[test]
    fn clean_title_collapses_newlines_and_strips_markdown() {
        // The larger budget gives a verbose model room to wrap the title in
        // markdown or spread it over lines; a stored title is always one line.
        assert_eq!(
            clean_generated_title("**BRCA1 in Breast Cancer**").as_deref(),
            Some("BRCA1 in Breast Cancer")
        );
        assert_eq!(
            clean_generated_title("BRCA1\n  in   Breast\tCancer").as_deref(),
            Some("BRCA1 in Breast Cancer")
        );
    }

    #[test]
    fn clean_title_unwraps_mixed_quoting() {
        // Each strip pass re-trims, so a double-then-single wrapping unwraps
        // fully instead of leaving the inner quotes behind.
        assert_eq!(
            clean_generated_title("\" 'BRCA1 Overview' \"").as_deref(),
            Some("BRCA1 Overview")
        );
    }

    #[test]
    fn clean_title_has_no_trailing_whitespace_after_truncation() {
        // Truncation can land mid-gap; the stored value must not keep the space.
        let raw = format!("{} tail", "x".repeat(TITLE_MAX_CHARS - 1));
        let cleaned = clean_generated_title(&raw).expect("non-empty");
        assert_eq!(cleaned, cleaned.trim(), "stored title must be trimmed");
    }

    #[test]
    fn budget_exhaustion_is_recognized_across_provider_families() {
        // The raw provider string reaches us un-canonicalized, and each family
        // spells it differently. Getting this wrong loses the one diagnostic
        // that names the budget as the cause.
        assert!(is_budget_exhausted("length")); // OpenAI
        assert!(is_budget_exhausted("max_tokens")); // Anthropic
        assert!(is_budget_exhausted("MAX_TOKENS")); // Gemini
        assert!(!is_budget_exhausted("stop"));
        assert!(!is_budget_exhausted("tool_calls"));
    }

    #[test]
    fn clean_title_rejects_empty_generations() {
        // The core of the fix: an empty generation yields NO title, so the
        // caller leaves the conversation untitled and retries — it can never
        // fall back to the raw user message.
        assert_eq!(clean_generated_title(""), None);
        assert_eq!(clean_generated_title("   \n  "), None);
        assert_eq!(clean_generated_title("\"\""), None);
        assert_eq!(clean_generated_title("''"), None);
    }
}

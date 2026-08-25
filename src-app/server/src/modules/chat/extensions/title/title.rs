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
/// The original 50-token budget was consumed entirely by reasoning on
/// `openai/gpt-oss-120b`: the stream ended with `finish_reason: "length"` having
/// emitted zero text, so title generation "failed" on every single chat through
/// that provider. It was raised to 512 — which was STILL too small, and the same
/// bug shipped again: on a deployment serving `qwen3.6-35b-a3b` through an
/// OpenAI-compatible bridge, **0 of 16 conversations had a title**. Measured
/// against that exact model with this exact prompt:
///
/// | `max_tokens` | `finish_reason` | answer text | completion tokens |
/// |---|---|---|---|
/// | 512  | `length` | none | 512 (all reasoning) |
/// | 1024 | `length` | none | 1024 (all reasoning) |
/// | 2048 | `stop`   | "Creating a New Project" | 942 |
/// | 4096 | `stop`   | "Request to Create New Project" | 1138 |
///
/// 4096 leaves ~3.5x headroom over the observed reasoning length. Because
/// reasoning length is fundamentally unbounded, the constant alone is a fix for
/// today's model, not for the failure MODE — so a budget-exhausted attempt is
/// retried ONCE at [`TITLE_RETRY_MAX_TOKENS`]. A title is a once-per-conversation
/// call, so the cost is negligible either way.
const TITLE_MAX_TOKENS: u32 = 4096;

/// Budget for the single escalated retry after a budget-exhausted first attempt
/// (see [`TITLE_MAX_TOKENS`]). Deliberately bounded: one retry, then the
/// extension soft-fails and the NEXT turn retries (bounded in turn by
/// [`TITLE_RETRY_MESSAGE_LIMIT`]), so a pathological model costs a handful of
/// calls rather than an unbounded escalation.
const TITLE_RETRY_MAX_TOKENS: u32 = 8192;

/// Wall-clock bound on ONE title attempt.
///
/// Title generation is AWAITED inline in `after_llm_call`, so it sits between the
/// assistant's last token and the turn's terminal frame — the user is watching.
/// With an escalated retry that is up to two generations on that path, and the
/// provider client's own timeout is a per-read one (a slow-but-alive stream is
/// effectively unbounded). A title is a nice-to-have: past this bound, abandon it
/// and let a later turn retry rather than hold the turn open.
const TITLE_ATTEMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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

/// Decide whether a finished title attempt deserves ONE escalated retry.
///
/// Pure so the condition is directly unit-testable — and it must stay narrow:
/// retry ONLY when the model produced no usable text AND the stream ended
/// because the budget ran out. An empty completion that ended `stop` is a model
/// that genuinely had nothing to say; retrying it would burn a second call for
/// nothing and would break the deliberate "an empty generation leaves the title
/// UNSET (and retries on a LATER turn)" contract.
fn should_retry_with_larger_budget(title: Option<&str>, finish_reason: Option<&str>) -> bool {
    if title.is_some() {
        return false;
    }
    match finish_reason {
        Some(reason) => is_budget_exhausted(reason),
        // No finish reason at all: some OpenAI-compatible bridges omit it on the
        // terminal chunk. An empty answer with no stated reason is
        // indistinguishable from the budget-starvation case, and the cost of
        // being wrong is ONE extra call — versus silently reverting to "this
        // deployment is permanently untitled" for that provider family.
        None => true,
    }
}

/// The soft-failure error for an attempt that produced no usable title, naming
/// the budget that was actually in force.
fn empty_title_error(budget: u32, finish_reason: Option<&str>) -> AppError {
    match finish_reason {
        // The budget ran out before the model emitted any answer text —
        // characteristic of a reasoning model.
        Some(reason) if is_budget_exhausted(reason) => AppError::internal_error(format!(
            "generated title is empty: the model exhausted the {budget}-token budget \
             (finish_reason={reason}) without emitting answer text"
        )),
        Some(reason) => {
            AppError::internal_error(format!("generated title is empty (finish_reason={reason})"))
        }
        None => AppError::internal_error("generated title is empty"),
    }
}

/// Build the (tool-less) chat request used to generate a title.
///
/// Extracted so the token budget and prompt shape are unit-testable without a
/// provider — the budget in particular is the root-cause fix and must not be
/// silently reverted.
///
/// `max_tokens` is a parameter (rather than the constant inline) so the escalated
/// retry reissues the IDENTICAL request with only the budget raised.
fn build_title_request(model_name: &str, user_content: &str, max_tokens: u32) -> ChatRequest {
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
        max_tokens: Some(max_tokens),
        // `thinking` is deliberately left UNSET (`None`).
        //
        // Naming a conversation needs no chain of thought, so an explicit
        // `ThinkingConfig::disabled()` looks attractive — but it buys nothing and
        // costs correctness: it is inert for OpenAI-compatible endpoints (which
        // only read `thinking.effort`, never set here), a literal no-op for
        // Anthropic (`ThinkingMode::Disabled => {}`), and ACTIVE for Gemini,
        // whose adapter emits `thinkingConfig { thinkingBudget: 0 }` for ANY
        // `Some(thinking)`. Models that cannot disable thinking reject that with
        // a 400 — which this extension soft-swallows, leaving the conversation
        // permanently untitled: the exact bug being fixed, reintroduced for a
        // different provider. The budget + escalated retry carry the fix alone.
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

    /// Generate a title using AI, with ONE escalated-budget retry.
    ///
    /// A reasoning model can burn the whole output budget on hidden chain of
    /// thought and return zero answer text — the exact production failure that
    /// left a whole deployment untitled. When that happens
    /// ([`should_retry_with_larger_budget`]) the identical request is reissued
    /// once at [`TITLE_RETRY_MAX_TOKENS`] before giving up for this turn.
    async fn generate_title_with_ai(
        &self,
        provider: &Provider,
        model_name: &str,
        user_content: &str,
    ) -> Result<String, AppError> {
        let (title, finish_reason) =
            self.attempt_title(provider, model_name, user_content, TITLE_MAX_TOKENS).await?;

        if !should_retry_with_larger_budget(title.as_deref(), finish_reason.as_deref()) {
            return title.ok_or_else(|| empty_title_error(TITLE_MAX_TOKENS, finish_reason.as_deref()));
        }

        tracing::info!(
                model = %model_name,
                finish_reason = %finish_reason.as_deref().unwrap_or(""),
                "title: the {}-token budget was exhausted with no answer text; retrying once at {}",
                TITLE_MAX_TOKENS,
                TITLE_RETRY_MAX_TOKENS,
        );
        let (retry_title, retry_finish) = self
            .attempt_title(provider, model_name, user_content, TITLE_RETRY_MAX_TOKENS)
            .await?;
        retry_title.ok_or_else(|| empty_title_error(TITLE_RETRY_MAX_TOKENS, retry_finish.as_deref()))
    }

    /// ONE title call: stream it, collect the answer text, and report the
    /// cleaned title (if any) plus the stream's finish reason.
    async fn attempt_title(
        &self,
        provider: &Provider,
        model_name: &str,
        user_content: &str,
        max_tokens: u32,
    ) -> Result<(Option<String>, Option<String>), AppError> {
        // Bounded: this runs on the awaited turn-end path (see
        // TITLE_ATTEMPT_TIMEOUT). A timeout is a soft failure like any other —
        // the conversation stays untitled and a later turn retries.
        tokio::time::timeout(
            TITLE_ATTEMPT_TIMEOUT,
            self.attempt_title_inner(provider, model_name, user_content, max_tokens),
        )
        .await
        .map_err(|_| {
            AppError::internal_error(format!(
                "title generation exceeded {}s at a {max_tokens}-token budget",
                TITLE_ATTEMPT_TIMEOUT.as_secs()
            ))
        })?
    }

    async fn attempt_title_inner(
        &self,
        provider: &Provider,
        model_name: &str,
        user_content: &str,
        max_tokens: u32,
    ) -> Result<(Option<String>, Option<String>), AppError> {
        // Call AI provider and collect the stream
        let mut stream = provider
            .chat_stream(build_title_request(model_name, user_content, max_tokens))
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

        Ok((clean_generated_title(&full_content), finish_reason))
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
        self.title_if_needed(context, tx).await?;
        Ok(ExtensionAction::Complete)
    }

    /// The turn ended without an LLM call — most commonly a `manual_approve`
    /// resume whose approved `audience:["user"]` tool result IS the answer.
    /// `after_llm_call` never fires there, so without this the conversation
    /// stays untitled until the user's NEXT message.
    async fn after_llm_skipped(
        &self,
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(), AppError> {
        self.title_if_needed(context, tx).await
    }
}

impl TitleGenerationExtension {
    /// Generate and persist a title when the conversation still needs one.
    ///
    /// The single body behind BOTH `after_llm_call` and `after_llm_skipped`, so
    /// the two entry points cannot drift apart. Self-gating (`has_title` /
    /// `should_generate_title`) makes it safe to call at the end of ANY turn:
    /// a titled conversation, a turn that produced no answer, and an
    /// attachment-only first message all return without work.
    ///
    /// Reads everything it needs from `context` + the database, which is why it
    /// works identically on the LLM-skipped paths where no `Message` is
    /// available to pass in.
    async fn title_if_needed(
        &self,
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(), AppError> {
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
            return Ok(());
        }

        let history = Repos
            .chat
            .core
            .get_conversation_history(context.branch_id)
            .await?;

        if !should_generate_title(&history, conversation.title.as_deref()) {
            return Ok(());
        }

        let Some(user_content) = first_user_text(&history) else {
            // No text to summarize (e.g. an attachment-only first message).
            return Ok(());
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
                return Ok(());
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

        Ok(())
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
                completion_state: None,
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
        let req = build_title_request("some-model", "What is known about BRCA1?", TITLE_MAX_TOKENS);
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
        let req = build_title_request("m", &long, TITLE_MAX_TOKENS);
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

    // ---- the reasoning-budget fix (TEST-5 / TEST-6) -----------------------

    /// TEST-5 — the request the provider receives.
    ///
    /// Root-cause guard for the SECOND time this shipped broken: 512 tokens were
    /// measured to be entirely consumed by `qwen3.6-35b-a3b`'s reasoning
    /// preamble (`finish_reason: "length"`, zero answer text), leaving every
    /// conversation on that deployment untitled.
    #[test]
    fn title_request_is_reasoning_safe() {
        let req = build_title_request("some-model", "How do I sort a list?", TITLE_MAX_TOKENS);

        assert!(
            req.max_tokens.is_some_and(|t| t >= 4096),
            "the title budget must survive a reasoning model's preamble, got {:?}",
            req.max_tokens
        );
        assert!(
            TITLE_RETRY_MAX_TOKENS > TITLE_MAX_TOKENS,
            "the escalated retry must actually escalate"
        );
        assert!(req.tools.is_empty(), "the title call must be tool-less");
        assert!(
            req.thinking.is_none(),
            "`thinking` must stay UNSET: `Some(Disabled)` is inert on OpenAI, a \
             no-op on Anthropic, and makes Gemini emit thinkingBudget:0, which \
             models that cannot disable thinking reject with a 400 — silently \
             leaving every conversation untitled again"
        );
        // The budget is a parameter so the retry reissues the SAME request.
        assert_eq!(
            build_title_request("m", "hi", TITLE_RETRY_MAX_TOKENS).max_tokens,
            Some(TITLE_RETRY_MAX_TOKENS)
        );
    }

    /// TEST-6 — the escalated retry fires ONLY on budget exhaustion.
    #[test]
    fn retry_only_on_budget_exhaustion_with_no_text() {
        // The production failure: no text, budget ran out → retry.
        assert!(should_retry_with_larger_budget(None, Some("length")));
        assert!(should_retry_with_larger_budget(None, Some("max_tokens")));
        assert!(should_retry_with_larger_budget(None, Some("MAX_TOKENS")));

        // A model that genuinely said nothing (finish_reason `stop`) must NOT be
        // retried — that is the deliberate "empty generation leaves the title
        // UNSET, retry on a LATER turn" contract, and the stub-backed regression
        // test asserts exactly ONE title call there.
        assert!(!should_retry_with_larger_budget(None, Some("stop")));
        assert!(!should_retry_with_larger_budget(None, Some("tool_calls")));

        // A bridge that omits finish_reason entirely is indistinguishable from
        // budget starvation, so it gets the one retry rather than silently
        // reverting to "permanently untitled" for that provider family.
        assert!(should_retry_with_larger_budget(None, None));

        // Text was produced → nothing to retry, whatever the finish reason.
        assert!(!should_retry_with_larger_budget(Some("A Title"), Some("length")));
        assert!(!should_retry_with_larger_budget(Some("A Title"), Some("stop")));
    }

    /// The soft-failure message must name the budget actually in force, so a
    /// log reader can tell a first attempt from the escalated retry.
    #[test]
    fn empty_title_error_names_the_budget_in_force() {
        let e = empty_title_error(TITLE_RETRY_MAX_TOKENS, Some("length"));
        let msg = format!("{e:?}");
        assert!(
            msg.contains(&TITLE_RETRY_MAX_TOKENS.to_string()),
            "the error must name the retry budget: {msg}"
        );
        let e = empty_title_error(TITLE_MAX_TOKENS, Some("stop"));
        assert!(format!("{e:?}").contains("finish_reason=stop"));
    }
}

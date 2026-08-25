//! `ControlExtension` — attaches the app-control MCP tools to a request when the
//! deploy kill-switch is on and the model is tool-capable. Errors never break
//! chat.

use std::convert::Infallible;

use async_trait::async_trait;
use axum::response::sse::Event;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

use crate::common::AppError;
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::extension::{BeforeLlmAction, ChatExtension, StreamContext};

/// System nudge: what the tools are, the two safety rules (approval on writes,
/// scoped to the user's own authority), and how to collect missing input.
///
/// The last sentence is load-bearing: without it the model writes a numbered
/// questionnaire into the chat ("1. What's the project name? …") when a mutating
/// operation needs fields the user did not supply. It is deliberately narrow —
/// "REQUIRED values … cannot infer" — because an unqualified "ask when anything
/// is missing" makes the model stop and ask about OPTIONAL fields on a request
/// that was already actionable, which is a worse experience than the defect and
/// stalls the turn waiting on a form nobody needed.
///
/// `ask_user` is attached on every TOOL-CAPABLE turn
/// (`mcp/chat_extension/mcp.rs` pushes the elicitation server into
/// `auto_attach_builtin_ids` when `model_tools_capable` is set), and this nudge
/// is prepended only when `model_supports_tools` — the same condition, read from
/// the same memoized metadata key. So naming the tool can never point at one the
/// model does not have.
///
/// Kept short: this text is injected into EVERY tool-capable chat.
const CONTROL_NUDGE: &str = "## App control\n\
    You can operate this ziee application on the user's behalf. Call \
    `list_capabilities` to discover available operations, `describe_capability` \
    to learn an operation's inputs, and `invoke_capability` to run one. \
    Operations are filtered to those the current user appears permitted to run, \
    and every action is re-authorized when it runs — so you can never actually \
    perform something the user isn't allowed to, and an operation may still be \
    refused. State-changing actions (create/update/delete) require the user's \
    explicit approval before they run — describe what you're about to do first. \
    If an operation still needs REQUIRED values you cannot infer from the \
    conversation, collect them with `ask_user` — one property per field, with \
    its `title`, `description`, `enum`+`enumNames` where the schema declares \
    them, and a `default` where one is implied — never by asking in chat text. \
    Don't ask about optional fields the user didn't mention; just proceed.";

pub struct ControlExtension {
    enabled: bool,
}

impl ControlExtension {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait]
impl ChatExtension for ControlExtension {
    fn name(&self) -> &str {
        "control"
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // Deploy kill-switch off → never attach.
        if !self.enabled {
            return Ok(BeforeLlmAction::Continue);
        }
        // A non-tool-capable model can't call the tools.
        let tool_capable =
            crate::modules::file::available_files::model_supports_tools(&context.metadata).await;
        if !tool_capable {
            return Ok(BeforeLlmAction::Continue);
        }
        apply_control_attach(&mut context.metadata, &mut request.messages);
        Ok(BeforeLlmAction::Continue)
    }
}

/// Set the auto-attach flag + prepend the system nudge. Pure so the wire-format
/// mutation (the documented silent-failure point) is unit-testable. The flag key
/// is the shared [`super::ATTACH_FLAG`] const `auto_attach_builtin_ids` reads.
fn apply_control_attach(
    metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    messages: &mut Vec<ChatMessage>,
) {
    metadata.insert(super::ATTACH_FLAG.to_string(), serde_json::json!("true"));
    messages.insert(
        0,
        ChatMessage {
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: CONTROL_NUDGE.to_string(),
            }],
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn apply_attach_sets_shared_flag_and_prepends_nudge() {
        let mut md: HashMap<String, serde_json::Value> = HashMap::new();
        let mut msgs = vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "hi".into() }],
        }];
        apply_control_attach(&mut md, &mut msgs);

        assert_eq!(
            md.get(super::super::ATTACH_FLAG).and_then(|v| v.as_str()),
            Some("true")
        );
        assert!(matches!(msgs[0].role, Role::System));
        match &msgs[0].content[0] {
            ContentBlock::Text { text } => assert!(text.contains("list_capabilities")),
            _ => panic!("expected a text content block"),
        }
        assert!(matches!(msgs[1].role, Role::User));
    }

    /// TEST-18 — the nudge must carry the ask-with-a-form rule, not just the
    /// discovery/approval text. Without it the model asks for missing values as
    /// a prose questionnaire instead of rendering the built-in form.
    #[test]
    fn nudge_directs_missing_input_to_ask_user_not_prose() {
        let mut md: HashMap<String, serde_json::Value> = HashMap::new();
        let mut msgs = vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "hi".into() }],
        }];
        apply_control_attach(&mut md, &mut msgs);

        let ContentBlock::Text { text } = &msgs[0].content[0] else {
            panic!("expected a text content block");
        };
        assert!(text.contains("ask_user"), "nudge must name the tool: {text}");
        assert!(
            text.contains("default"),
            "nudge must ask for pre-filled defaults: {text}"
        );
        assert!(
            text.contains("chat text"),
            "nudge must forbid asking in prose: {text}"
        );
        for needle in ["title", "description", "enumNames"] {
            assert!(
                text.contains(needle),
                "nudge must name the schema key `{needle}` that makes the form good: {text}"
            );
        }
        // Narrow, not blanket: an unqualified "ask when anything is missing"
        // stalls an already-actionable request on a form for optional fields.
        assert!(
            text.contains("REQUIRED values you cannot infer"),
            "the ask rule must be scoped to required, non-inferable values: {text}"
        );
        assert!(
            text.contains("optional fields"),
            "the nudge must tell the model to proceed past optional fields: {text}"
        );
        // The filter is a best-effort visibility filter, not an authorization
        // claim — some operations carry no recoverable permission declaration.
        assert!(
            text.contains("may still be refused"),
            "the nudge must not overclaim that the offered list is exhaustively filtered: {text}"
        );
    }
}

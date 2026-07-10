//! `JsToolExtension` — attaches the built-in `run_js` tool to a request when the
//! model is tool-capable and the feature is enabled by config. Never breaks chat.

use std::convert::Infallible;
use std::sync::Arc;

use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

use crate::common::AppError;
use crate::core::config::Config;
use crate::modules::chat::core::extension::request::SendMessageRequest;
use crate::modules::chat::core::extension::{BeforeLlmAction, ChatExtension, StreamContext};

/// Generic system nudge — deliberately does NOT enumerate specific tool names
/// (the concrete `ziee.tools.*` bindings are computed at execution time), so it
/// points the model at `ziee.toolList()` for discovery.
const RUN_JS_NUDGE: &str = "## Programmatic tool calling (run_js)\n\
    When you need to call a tool many times, or filter/aggregate large tool \
    results without filling your context, call `run_js` with a JavaScript async \
    body. Inside it, call your tools as `await ziee.tools.<name>({ ...args })` \
    (call `ziee.toolList()` first to see the exact binding names + input \
    schemas) and `return` ONLY the final result — the intermediate tool results \
    stay inside the script and never reach your context. There is no \
    filesystem, network, `fetch`, `require`, or `process`; the `ziee.*` \
    functions are the only capability. A tool call that needs approval pauses \
    for the user, and a denied call throws `ToolApprovalDenied` you can catch.";

pub struct JsToolExtension {
    #[allow(dead_code)]
    pool: PgPool,
    config: Arc<Config>,
}

impl JsToolExtension {
    pub fn new(pool: PgPool, config: Arc<Config>) -> Self {
        Self { pool, config }
    }
}

#[async_trait]
impl ChatExtension for JsToolExtension {
    fn name(&self) -> &str {
        "js_tool"
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // A non-tool-capable model can't call run_js, so don't attach.
        let tool_capable =
            crate::modules::file::available_files::model_supports_tools(&context.metadata).await;
        if !tool_capable {
            return Ok(BeforeLlmAction::Continue);
        }

        if crate::modules::js_tool::is_enabled(&self.config) {
            apply_run_js_attach(&mut context.metadata, &mut request.messages);
        }

        Ok(BeforeLlmAction::Continue)
    }
}

/// Set the auto-attach flag + prepend the system nudge. Pure so the wire-format
/// mutation (the documented silent-failure point) is directly unit-testable.
fn apply_run_js_attach(
    metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    messages: &mut Vec<ChatMessage>,
) {
    metadata.insert(super::ATTACH_FLAG.to_string(), serde_json::json!("true"));
    messages.insert(
        0,
        ChatMessage {
            role: Role::System,
            content: vec![ContentBlock::Text { text: RUN_JS_NUDGE.to_string() }],
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // TEST-20: attach sets the shared flag + prepends the nudge.
    #[test]
    fn apply_attach_sets_shared_flag_and_prepends_nudge() {
        let mut md: HashMap<String, serde_json::Value> = HashMap::new();
        let mut msgs = vec![ChatMessage {
            role: Role::User,
            content: vec![ContentBlock::Text { text: "hi".into() }],
        }];
        apply_run_js_attach(&mut md, &mut msgs);

        assert_eq!(
            md.get(super::super::ATTACH_FLAG).and_then(|v| v.as_str()),
            Some("true")
        );
        assert!(matches!(msgs[0].role, Role::System));
        match &msgs[0].content[0] {
            ContentBlock::Text { text } => assert!(text.contains("run_js")),
            _ => panic!("expected a text content block"),
        }
        assert!(matches!(msgs[1].role, Role::User));
    }
}

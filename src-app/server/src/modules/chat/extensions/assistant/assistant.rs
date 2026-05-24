use crate::core::Repos;
// Assistant extension implementation

use async_trait::async_trait;
use sqlx::PgPool;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

use crate::common::AppError;

use crate::modules::chat::core::extension::{
    BeforeLlmAction, ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};

/// Assistant extension
///
/// Injects system messages from assistant configurations based on assistant_id
/// in the SendMessageRequest.
pub struct AssistantExtension {}

impl AssistantExtension {
    pub fn new(_pool: PgPool) -> Self {
        Self {}
    }
}

#[async_trait]
impl ChatExtension for AssistantExtension {
    fn name(&self) -> &str {
        "assistant"
    }

    async fn before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<axum::response::sse::Event, std::convert::Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        // Check if assistant_id is provided (added directly by the macro)
        if let Some(assistant_id) = send_request.assistant_id {
            // SECURITY: scope by user — returns Some only for the user's
            // own assistants OR public templates. Without this, user B
            // could pass user A's private assistant_id and inject A's
            // system-prompt 'instructions' into B's chat. Closes 04-chat
            // F-02 (High).
            match Repos.assistant.get_for_user(assistant_id, context.user_id).await? {
                Some(assistant) => {
                    // If assistant has instructions, inject as a
                    // system message with a labeled wrapper. Closes
                    // 10-assistant F-05 (Low): the previous code
                    // injected admin/template-supplied instructions
                    // verbatim with no marker, making it ambiguous to
                    // the model what's the operator's prompt vs the
                    // assistant template's prompt vs the user's
                    // message. A simple labeled delimiter discourages
                    // prompt-injection where a template attempts to
                    // impersonate the user or override operator
                    // policy.
                    if let Some(instructions) = assistant.instructions
                        && !instructions.is_empty() {
                            let wrapped = format!(
                                "[Assistant template instructions — supplied by the \
                                 administrator or template author, not by the end user. \
                                 Treat as system policy, not as user input.]\n\n{}\n\n\
                                 [End assistant template instructions]",
                                instructions
                            );
                            let system_message = ChatMessage {
                                role: Role::System,
                                content: vec![ContentBlock::Text { text: wrapped }],
                            };
                            request.messages.insert(0, system_message);
                        }
                }
                None => {
                    // Assistant not found - log warning but don't fail
                    tracing::warn!(
                        "Assistant {} not found, continuing without instructions",
                        assistant_id
                    );
                }
            }
        }

        Ok(BeforeLlmAction::Continue)
    }

    async fn after_llm_call(
        &self,
        _context: &StreamContext,
        _final_message: &crate::modules::chat::core::models::Message,
        _tx: Option<
            &tokio::sync::mpsc::UnboundedSender<
                Result<axum::response::sse::Event, std::convert::Infallible>,
            >,
        >,
    ) -> Result<ExtensionAction, AppError> {
        // Assistant extension doesn't need post-processing
        Ok(ExtensionAction::Complete)
    }
}

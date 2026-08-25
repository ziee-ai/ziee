use crate::core::Repos;
// Assistant extension implementation

use async_trait::async_trait;
use sqlx::PgPool;

use aide::axum::ApiRouter;
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

    /// Snapshot the assistant that was selected when this user message
    /// was sent into the `message_assistant` join table. Used by the
    /// frontend assistant extension on Edit to restore the originally-
    /// selected assistant. Replaces the `messages.assistant_id` column
    /// that chat used to own (migration 75 dropped it).
    ///
    /// Soft-fail: if the INSERT fails (DB blip), the message stays
    /// saved without attribution. Edit-restore then degrades to "use
    /// current assistant selection" — same fallback as messages from
    /// before any assistant was set.
    async fn after_user_message_created(
        &self,
        _context: &StreamContext,
        user_message: &crate::modules::chat::core::models::Message,
        send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        let Some(assistant_id) = send_request.assistant_id else {
            return Ok(());
        };
        Repos
            .chat
            .assistant
            .insert_message_assistant(user_message.id, assistant_id)
            .await
    }

    /// Register the assistant bridge's HTTP routes. Owned by the
    /// bridge so chat doesn't have to know they exist.
    /// `GET /api/messages/{id}/assistant` reads the per-message
    /// assistant attribution from `message_assistant` (migration 75),
    /// replacing the inline `messages.assistant_id` column.
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router.merge(super::message_assistant_routes::message_assistant_router())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn lazy_pool() -> PgPool {
        sqlx::PgPool::connect_lazy("postgres://u:p@127.0.0.1:1/none").expect("lazy pool")
    }

    /// Negative path of the assistant injection (assistant.rs before_llm_call):
    /// when the send carries NO `assistant_id`, the extension must inject no
    /// system message and leave the request untouched. Resolved without any DB
    /// access, so a lazy (never-connected) pool is fine.
    #[tokio::test]
    async fn before_llm_call_no_assistant_id_injects_nothing() {
        let ext = AssistantExtension::new(lazy_pool());
        let mut context = StreamContext {
            conversation_id: Uuid::new_v4(),
            branch_id: Uuid::new_v4(),
            message_id: None,
            user_id: Uuid::new_v4(),
            pool: lazy_pool(),
            metadata: HashMap::new(),
            iteration: 1,
        };
        let mut request = ChatRequest::default();
        // SendMessageRequest with no assistant_id (omitted from the JSON).
        let send: SendMessageRequest = serde_json::from_value(serde_json::json!({
            "content": "hello",
            "model_id": Uuid::new_v4().to_string(),
            "branch_id": Uuid::new_v4().to_string(),
        }))
        .expect("construct SendMessageRequest");

        let action = ext
            .before_llm_call(&mut context, &mut request, &send, None)
            .await
            .expect("before_llm_call must not error without an assistant_id");

        assert!(matches!(action, BeforeLlmAction::Continue));
        assert!(
            request.messages.is_empty(),
            "no assistant_id => no system-instruction injection"
        );
    }
}

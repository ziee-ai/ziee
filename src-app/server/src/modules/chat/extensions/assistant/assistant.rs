// Assistant extension implementation

use async_trait::async_trait;
use sqlx::PgPool;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Role};

use crate::common::AppError;
use crate::modules::assistant::repository as assistant_repo;
use crate::modules::chat::core::extension::{
    ChatExtension, ExtensionAction, SendMessageRequest, StreamContext,
};

/// Assistant extension
///
/// Injects system messages from assistant configurations based on assistant_id
/// in the SendMessageRequest.
pub struct AssistantExtension {
    pool: PgPool,
}

impl AssistantExtension {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChatExtension for AssistantExtension {
    fn name(&self) -> &str {
        "assistant"
    }

    async fn before_llm_call(
        &self,
        _context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        // Check if assistant_id is provided (added directly by the macro)
        if let Some(assistant_id) = send_request.assistant_id {
            // Fetch assistant from database
            match assistant_repo::get_assistant(&self.pool, assistant_id).await? {
                Some(assistant) => {
                    // If assistant has instructions, inject as system message
                    if let Some(instructions) = assistant.instructions {
                        if !instructions.is_empty() {
                            // Create system message
                            let system_message = ChatMessage {
                                role: Role::System,
                                content: vec![ContentBlock::Text { text: instructions }],
                            };

                            // Insert at the beginning of messages
                            request.messages.insert(0, system_message);
                        }
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

        Ok(())
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

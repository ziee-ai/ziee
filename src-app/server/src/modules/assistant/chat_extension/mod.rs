// Assistant Extension for Chat Module
//
// Injects system messages from assistant configurations based on the assistant_id
// provided in the SendMessageRequest. Also persists per-message
// assistant attribution into `message_assistant` (migration 75) via
// the `after_user_message_created` lifecycle hook — replaces the
// `messages.assistant_id` column that lived on chat's table before.

mod assistant;
pub mod extension; // Auto-discovered by build script
pub mod message_assistant_routes;
pub mod repository;

pub use repository::AssistantChatRepository;

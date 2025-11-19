// Repository layer for chat module

pub mod branches;
pub mod contents;
pub mod conversations;
pub mod core;
pub mod messages;

pub use core::ChatCoreRepository;

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{Branch, Conversation, MessageContent, MessageContentData};
use crate::modules::chat::core::types::{ConversationResponse, EditMessageRequest, EditMessageResponse, MessageWithContent};

// Include auto-generated ChatRepository with extension fields
include!(concat!(env!("OUT_DIR"), "/chat_repository.rs"));

// Re-export for convenience

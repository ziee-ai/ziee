// Repository layer for chat module

pub mod branches;
pub mod contents;
pub mod conversations;
pub mod core;
pub mod messages;

pub use core::ChatCoreRepository;

use sqlx::PgPool;

// Include auto-generated ChatRepository with extension fields
include!(concat!(env!("OUT_DIR"), "/chat_repository.rs"));

// Re-export for convenience

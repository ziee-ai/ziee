//! Memory chat-extension bridge — hook-only files.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `memory.rs` is the `ChatExtension` trait impl (before/after_llm_call
//! hooks). `retriever.rs` injects a system block before the LLM call
//! (called from `before_llm_call`). `repository.rs` holds the
//! `MemoryChatRepository` (`Repos.chat.memory`) for the
//! `conversation_memory_settings` table. `memory_mode_routes.rs`
//! exposes `GET`/`PUT /api/conversations/{id}/memory-mode`.
//!
//! The dual-use engine code (`extractor`, `dispatch`,
//! `prompts`) lives in `super::engine::*` — it's reached by the hooks
//! here AND by `memory/handlers.rs` / `embedding_worker.rs` /
//! `memory_mcp/handlers.rs` directly, so it doesn't belong inside
//! this bridge.

pub mod extension;
mod memory;
pub mod memory_mode_routes;
pub mod repository;
pub mod retriever;

pub use repository::MemoryChatRepository;

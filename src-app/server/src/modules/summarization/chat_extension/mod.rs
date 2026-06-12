//! Summarization chat-extension bridge — hook-only files.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`
//! at order 24 (BEFORE memory's order 25 — load-bearing for the
//! [System*, SummaryBlock, MemoryBlock, RecentTurns] assembly invariant).
//! `summarization.rs` is the `ChatExtension` trait impl.
//! `repository.rs` holds the `SummarizationChatRepository`
//! (`Repos.chat.summarization`) for the
//! `conversation_summarization_settings` table.
//! `summarization_mode_routes.rs` exposes
//! `GET`/`PUT /api/conversations/{id}/summarization-mode`.

pub mod extension;
pub mod repository;
mod summarization;
pub mod summarization_mode_routes;

pub use repository::SummarizationChatRepository;

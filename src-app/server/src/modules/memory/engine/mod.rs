//! Memory engine — dual-use code (hook AND manual trigger).
//!
//! `extractor.rs` runs `tokio::spawn` after the LLM finishes and writes
//! extracted facts to the user's memory store; also called from
//! `memory/handlers.rs` (manual extract trigger) and
//! `memory_mcp/handlers.rs` (MCP extract tool).
//!
//! `dispatch.rs` routes embedding calls to local (llama-server) or
//! remote (AIProvider::embeddings()) based on the configured model's
//! provider_type. Called from the bridge retriever + bridge extractor
//! AND from `memory/embedding_worker.rs`.
//!
//! `prompts.rs` holds shared prompt templates used by the extractor.
//!
//! `summarizer.rs` moved to `crate::modules::summarization::engine`
//! in migration 91 — summarization is no longer a memory submodule.

pub mod capability;
pub mod dispatch;
pub mod extractor;
pub mod prompts;

//! Memory chat extension — silent extraction (after_llm_call) +
//! retrieval (before_llm_call).
//!
//! `dispatch.rs` routes embedding calls to local (llama-server) or
//! remote (AIProvider::embeddings()) based on the configured model's
//! provider_type. `retriever.rs` injects a system block before the
//! LLM call. `extractor.rs` runs in `tokio::spawn` after the LLM
//! finishes and writes extracted facts to the user's memory store.

pub mod dispatch;
pub mod extension;
pub mod extractor;
mod memory;
pub mod prompts;
pub mod retriever;
pub mod summarizer;

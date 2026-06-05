//! Memory engine — dual-use code (hook AND manual trigger).
//!
//! `extractor.rs` runs `tokio::spawn` after the LLM finishes and writes
//! extracted facts to the user's memory store; also called from
//! `memory/handlers.rs` (manual extract trigger) and
//! `memory_mcp/handlers.rs` (MCP extract tool).
//!
//! `summarizer.rs` exposes `refresh_summary` + `apply_summary_to_history`;
//! called from the bridge's `before_llm_call` / `after_llm_call` AND
//! from `memory/handlers.rs` (manual summarize trigger).
//!
//! `dispatch.rs` routes embedding calls to local (llama-server) or
//! remote (AIProvider::embeddings()) based on the configured model's
//! provider_type. Called from the bridge retriever + bridge extractor
//! AND from `memory/embedding_worker.rs`.
//!
//! `prompts.rs` holds shared prompt templates used by extractor and
//! summarizer.

pub mod capability;
pub mod dispatch;
pub mod extractor;
pub mod prompts;
pub mod summarizer;

//! Memory chat extension — silent extraction (after_llm_call) +
//! retrieval (before_llm_call). Phase 1 ships stubs; Phases 2–3 wire
//! in real embedding/extraction logic.

pub mod extension;
mod memory;

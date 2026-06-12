// Summarization module integration tests (migration 91 extraction).
//
// Engine unit tests (decide / build / apply pure logic) live in
// `src/modules/summarization/engine/summarizer.rs::tests` — 19 tests,
// all pure, ~0ms. These integration tests cover the REST surface +
// per-conversation mode + summary endpoint + the Tier-5 real-LLM
// path (R4/R5/R6 moved from `tests/memory/real_llm_test.rs`).

mod admin_settings_test;
mod migration_seed_test;
mod per_conversation_mode_test;
mod real_llm_helpers;
mod real_llm_test;
mod summary_endpoint_test;

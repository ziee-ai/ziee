// Memory module integration tests. See individual files for details.

mod capability_filter_test;
mod combined_real_llm;
mod core_memory_test;
mod crud_test;
mod extraction_injection_test;
mod extraction_model_validation_test;
mod extraction_test;
mod memory_off_test;
mod onboarding_settings_init_test;
mod per_conversation_toggle_test;
mod pgvector_install_test;
mod real_llm_helpers;
mod real_llm_test;
mod retention_test;
// `summarization_test` moved to `tests/summarization/` (migration 91).
mod sync_emit_test;
mod recall_fts_test;
mod fts_rebuild_test;

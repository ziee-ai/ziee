//! LLM Local Runtime module integration tests
//!
//! Tests for the llm_local_runtime module

// Shared fixtures
pub mod mock_release;
pub mod test_helpers;

// Test modules
mod engine_args_test;
mod engine_download_test;
mod gold_smoke;
mod gpu_detect_test;
mod lifecycle_test;
mod model_files_real_test;
mod provider_create_test;
mod proxy_auth_test;
mod proxy_forward_test;
mod reaper_drain_test;
mod settings_test;
mod sse_logs_test;
mod supervision_test;
mod token_rotation_test;
mod validation_test;
mod version_usage_test;

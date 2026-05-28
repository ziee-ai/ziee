//! Chat module integration tests
//!
//! Comprehensive test suite for the chat module including:
//! - Permission tests (22 tests)
//! - Conversation CRUD tests (29 tests)
//! - Message operation tests (13 tests)
//! - Branch management tests (10 tests)
//! - SSE streaming tests (6 tests)
//! - Cross-user ownership tests (15 tests)
//!
//! Total: ~95 integration tests

// Helper functions used across all test files.
// `pub(crate)` so the project test module can reuse get_test_model_configs +
// create_test_model_with_config + parse_sse_stream for its Tier-3
// real-LLM tests (project/injection_test.rs).
pub(crate) mod helpers;

// Test modules
mod permissions_test;
mod conversations_test;
mod messages_test;
mod branches_test;
mod streaming_test;
mod ownership_test;
mod file_attachments_test;
mod file_attachments_real_providers_test;
mod mcp_extension_test;
mod mcp_approval_workflow_test;
mod mcp_defaults_test;
mod mcp_streaming_workflow_test;
mod mcp_loop_settings_test;
mod mcp_sampling_test;
mod mcp_elicitation_test;
mod mcp_content_test;
mod sandbox_real_llm_test;
mod test_single_assistant_message_architecture;
mod assistant_block_grouping_test;
mod append_content_ordering_test;
mod user_providers_test;

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
// `pub(crate)` so OTHER test modules (project, file) can reuse
// get_test_model_configs + create_test_model_with_config +
// parse_sse_stream for their Tier-3 real-LLM tests.
pub(crate) mod helpers;

// Test modules.
//
// `file_attachments_*` tests moved to `tests/file/`, and `mcp_*`
// tests moved to `tests/mcp/`, as part of the chat→file/mcp bridge
// extraction. What remains here tests chat's own surface only.
mod permissions_test;
mod conversations_test;
mod messages_test;
mod branches_test;
mod streaming_test;
// New fire-and-forget send + per-user chat-token stream (stub-backed,
// deterministic) and the `sync:conversation` emit coverage.
mod chat_stream_test;
mod sync_emit_test;
mod ownership_test;
mod sandbox_real_llm_test;
mod test_single_assistant_message_architecture;
mod assistant_block_grouping_test;
mod append_content_ordering_test;
mod user_providers_test;

//! Chat module integration tests
//!
//! Comprehensive test suite for the chat module including:
//! - Permission tests (22 tests)
//! - Conversation CRUD tests (29 tests)
//! - Message operation tests (13 tests)
//! - Branch management tests (10 tests)
//! - SSE streaming tests (6 tests - require live AI)
//! - Cross-user ownership tests (15 tests)
//! - Extension tests (8 tests - API contracts)
//!
//! Total: ~103 integration tests (82 from initial suite + 8 extension tests - 13 streaming)
//!
//! NOTE: Full streaming and extension testing requires live AI providers with API keys.
//! Extension tests verify API contracts and basic functionality without LLM calls.

// Helper functions used across all test files
mod helpers;

// Test modules
mod permissions_test;
mod conversations_test;
mod messages_test;
mod branches_test;
mod streaming_test;
mod ownership_test;
mod extensions_test;

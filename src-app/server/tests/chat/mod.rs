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

// Helper functions used across all test files
mod helpers;

// Test modules
mod permissions_test;
mod conversations_test;
mod messages_test;
mod branches_test;
mod streaming_test;
mod ownership_test;

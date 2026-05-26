// Integration tests for the project module.
//
// Tier 2 (CRUD, permissions, ownership, files, duplicate, MCP snapshot,
// conversation moves, project deletion behavior).
//
// Tier 3 (context injection — assistant/project stacking, file
// prepending) would require a mock LLM provider that captures the
// outgoing ChatRequest. The existing chat tests use real providers via
// `tests/.env.test` keys; until we add a programmable mock that returns
// the wire payload to the test, those tests are deferred. See
// `injection_test.rs` for the documented scaffolding.

mod conversations_test;
mod crud_test;
mod duplicate_test;
mod files_test;
mod injection_test;
mod mcp_test;
mod permissions_test;

mod helpers;

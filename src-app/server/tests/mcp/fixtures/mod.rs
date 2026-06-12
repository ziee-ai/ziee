//! Test fixtures for MCP integration tests.
//!
//! Spawns external MCP servers (currently `@modelcontextprotocol/server-everything`)
//! as child processes for empirical conformance testing.

pub mod everything_server;
pub mod mock_mcp_server;
pub mod mock_elicitation_server;
pub mod mock_get_stream_elicitation_server;

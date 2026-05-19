// MCP Elicitation support
//
// Implements the MCP 2025-03-26 elicitation protocol: MCP servers can request
// structured human input during tool execution. Unlike sampling (server → LLM),
// elicitation is server → human (interactive form).
//
// Architecture:
//   - registry.rs: global in-memory oneshot channel map (message_id → response)
//   - models.rs:   protocol request/response types
//   - handlers.rs: POST /mcp/elicitation/{message_id}/respond
//   - routes.rs:   route registration

pub mod handlers;
pub mod models;
pub mod registry;
pub mod routes;

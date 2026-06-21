//! MCP tool-call history (`mcp_tool_calls`).
//!
//! Records EVERY MCP tool-call invocation (chat / rest / always / sampling /
//! approval, including built-in/loopback servers) as a durable owner-scoped
//! row — the MCP analog of `workflow_runs`. Recording happens once, at the
//! single chokepoint `McpSession::call_tool`, using an `McpCallContext`
//! stamped onto the (ephemeral) session at creation.

pub mod handlers;
pub mod models;
pub mod prune;
pub mod record;
pub mod repository;

pub use models::*;

//! MCP Module
//!
//! Desktop-specific MCP server functionality

mod event_handlers;

pub use event_handlers::{
    backfill_system_mcp_assignments, AutoAssignMcpServerHandler,
};

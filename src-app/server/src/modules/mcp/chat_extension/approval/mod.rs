//! MCP approval workflow module

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;

// Re-export commonly used items
pub use models::*;
pub use routes::mcp_approval_router;

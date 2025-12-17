//! User MCP defaults module

pub mod handlers;
pub mod models;
pub mod repository;
pub mod routes;

// Re-export commonly used items
pub use routes::mcp_defaults_router;

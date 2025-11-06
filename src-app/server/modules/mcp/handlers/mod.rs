// MCP handlers module
// Organizes all handler functions for MCP server operations

pub mod user;
pub mod admin;
pub mod groups;

// Re-export user handlers
pub use user::*;

// Re-export admin handlers
pub use admin::*;

// Re-export group assignment handlers
pub use groups::*;

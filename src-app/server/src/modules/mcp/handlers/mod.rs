// MCP handlers module
// Organizes all handler functions for MCP server operations

pub mod user;
pub mod system;
pub mod groups;

// Re-export user handlers
pub use user::*;

// Re-export system handlers
pub use system::*;

// Re-export group assignment handlers
pub use groups::*;

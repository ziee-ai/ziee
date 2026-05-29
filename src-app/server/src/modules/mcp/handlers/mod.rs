// MCP handlers module
// Organizes all handler functions for MCP server operations

pub mod groups;
pub mod runtime;
pub mod system;
pub mod test_connection;
pub mod user;

// Re-export user handlers
pub use user::*;

// Re-export system handlers
pub use system::*;

// Re-export group assignment handlers
pub use groups::*;

// Runtime handlers are accessed via runtime:: prefix in routes

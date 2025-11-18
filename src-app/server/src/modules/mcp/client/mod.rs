pub mod traits;
pub mod stdio;
pub mod http;
pub mod sse;
pub mod session;
pub mod manager;

// Re-export main types for convenience (used via full module path in main.rs and handlers)
#[allow(unused_imports)]
pub use manager::McpSessionManager;
#[allow(unused_imports)]
pub use session::McpSession;
#[allow(unused_imports)]
pub use traits::{McpClient, Tool, Resource, ToolResult, ToolContent};

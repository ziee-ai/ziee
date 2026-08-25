pub mod auth;
pub mod errors;
pub mod http;
pub mod manager;
pub mod session;
pub mod stdio;
pub mod traits;

// Re-export main types for convenience (used via full module path in main.rs and handlers)
#[allow(unused_imports)]
pub use manager::McpSessionManager;
#[allow(unused_imports)]
pub use session::McpSession;
#[allow(unused_imports)]
pub use traits::{
    McpClient, Prompt, PromptArgument, PromptResult, Resource, Tool, ToolContent, ToolResult,
};

// MCP chat extension module

pub mod approval;
pub mod content;
pub mod defaults;
pub mod extension;
pub mod helpers;
pub mod mcp;
pub mod message_servers_routes;
pub mod office_approval;
pub mod repository;

// Re-export key types
pub use approval::{
    ApprovalMode, ToolApprovalDecision,
};
pub use extension::McpConfig;
pub use repository::McpChatRepository;

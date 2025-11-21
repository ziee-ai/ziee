// MCP chat extension module

pub mod approval;
pub mod content;
pub mod extension;
pub mod helpers;
pub mod mcp;
pub mod repository;

// Re-export key types
pub use approval::{
    ApprovalMode, ApprovalStatus, ConversationMcpSettings, ToolApprovalDecision,
    ToolUseApproval, UpsertMcpSettingsRequest,
};
pub use content::McpContentData;
pub use extension::{
    McpConfig, McpServerConfig, SendMessageRequestFields,
    SSEChatStreamEventVariants, SSEChatStreamMcpApprovalRequiredData,
    SSEChatStreamMcpToolCompleteData, SSEChatStreamMcpToolStartData,
};
pub use mcp::McpChatExtension;
pub use repository::McpChatRepository;

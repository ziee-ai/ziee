// MCP server events
// Events emitted during MCP server lifecycle for inter-module communication

use uuid::Uuid;

/// Events for MCP server operations
#[derive(Debug, Clone)]
pub enum McpServerEvent {
    /// System MCP server was deleted
    SystemServerDeleted {
        server_id: Uuid,
    },
    /// User MCP server was deleted
    UserServerDeleted {
        server_id: Uuid,
        user_id: Uuid,
    },
}

impl McpServerEvent {
    /// Create a system server deleted event
    pub fn system_server_deleted(server_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::SystemServerDeleted {
            server_id,
        })
    }

    /// Create a user server deleted event
    pub fn user_server_deleted(server_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::UserServerDeleted {
            server_id,
            user_id,
        })
    }
}

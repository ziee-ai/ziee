// MCP server events
// Event infrastructure for future use
#![allow(dead_code)]

// Events emitted during MCP server lifecycle for inter-module communication

use uuid::Uuid;

/// Events for MCP server operations
#[derive(Debug, Clone)]
pub enum McpServerEvent {
    /// System MCP server was created
    SystemServerCreated { server_id: Uuid },
    /// User MCP server was created
    UserServerCreated { server_id: Uuid, user_id: Uuid },
    /// System MCP server was updated
    SystemServerUpdated { server_id: Uuid },
    /// User MCP server was updated
    UserServerUpdated { server_id: Uuid, user_id: Uuid },
    /// System MCP server was deleted
    SystemServerDeleted { server_id: Uuid },
    /// User MCP server was deleted
    UserServerDeleted { server_id: Uuid, user_id: Uuid },
    /// Group assignments for an MCP server changed
    GroupAssignmentChanged { server_id: Uuid },
}

impl McpServerEvent {
    /// Create a system server created event
    pub fn system_server_created(server_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::SystemServerCreated { server_id })
    }

    /// Create a user server created event
    pub fn user_server_created(server_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::UserServerCreated { server_id, user_id })
    }

    /// Create a system server updated event
    pub fn system_server_updated(server_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::SystemServerUpdated { server_id })
    }

    /// Create a user server updated event
    pub fn user_server_updated(server_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::UserServerUpdated { server_id, user_id })
    }

    /// Create a system server deleted event
    pub fn system_server_deleted(server_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::SystemServerDeleted { server_id })
    }

    /// Create a user server deleted event
    pub fn user_server_deleted(server_id: Uuid, user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::UserServerDeleted { server_id, user_id })
    }

    /// Create a group assignment changed event
    pub fn group_assignment_changed(server_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::McpServer(McpServerEvent::GroupAssignmentChanged { server_id })
    }
}

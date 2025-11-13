// Hub events for inter-module communication
// Event infrastructure for future use
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the Hub module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HubEvent {
    /// Hub models were refreshed from GitHub
    ModelsRefreshed {
        old_version: String,
        new_version: String,
    },

    /// Hub assistants were refreshed from GitHub
    AssistantsRefreshed {
        old_version: String,
        new_version: String,
    },

    /// Hub MCP servers were refreshed from GitHub
    McpServersRefreshed {
        old_version: String,
        new_version: String,
    },

    /// An assistant was created from hub catalog
    AssistantCreatedFromHub { assistant_id: Uuid, hub_id: String },

    /// An MCP server was created from hub catalog
    McpServerCreatedFromHub { server_id: Uuid, hub_id: String },
}

impl HubEvent {
    /// Create a ModelsRefreshed event
    pub fn models_refreshed(old_version: String, new_version: String) -> Self {
        Self::ModelsRefreshed {
            old_version,
            new_version,
        }
    }

    /// Create an AssistantsRefreshed event
    pub fn assistants_refreshed(old_version: String, new_version: String) -> Self {
        Self::AssistantsRefreshed {
            old_version,
            new_version,
        }
    }

    /// Create a McpServersRefreshed event
    pub fn mcp_servers_refreshed(old_version: String, new_version: String) -> Self {
        Self::McpServersRefreshed {
            old_version,
            new_version,
        }
    }

    /// Create an AssistantCreatedFromHub event
    pub fn assistant_created_from_hub(assistant_id: Uuid, hub_id: String) -> Self {
        Self::AssistantCreatedFromHub {
            assistant_id,
            hub_id,
        }
    }

    /// Create an McpServerCreatedFromHub event
    pub fn mcp_server_created_from_hub(server_id: Uuid, hub_id: String) -> Self {
        Self::McpServerCreatedFromHub { server_id, hub_id }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<HubEvent> for crate::core::events::AppEvent {
    fn from(event: HubEvent) -> Self {
        crate::core::events::AppEvent::Hub(event)
    }
}

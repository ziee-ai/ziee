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

    /// An assistant was created from hub catalog. `is_template`
    /// discriminates user-scoped installs from system-wide template
    /// installs — preserved on the payload (rather than re-looked up
    /// by listeners) because the assistant row may have been deleted
    /// + re-created by the `replace_existing` re-install path before
    /// a slow listener gets here.
    AssistantCreatedFromHub {
        assistant_id: Uuid,
        hub_id: String,
        is_template: bool,
    },

    /// An MCP server was created from hub catalog. `is_system`
    /// discriminates user-scoped installs from system-wide installs —
    /// preserved on the payload (rather than re-looked up by listeners)
    /// because the server row may have been deleted + re-created by
    /// the `replace_existing` re-install path before a slow listener
    /// gets here. Mirrors `AssistantCreatedFromHub.is_template`.
    McpServerCreatedFromHub {
        server_id: Uuid,
        hub_id: String,
        is_system: bool,
    },

    /// A model download was started from hub catalog
    ModelDownloadStartedFromHub { download_id: Uuid, hub_id: String },
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
    pub fn assistant_created_from_hub(
        assistant_id: Uuid,
        hub_id: String,
        is_template: bool,
    ) -> Self {
        Self::AssistantCreatedFromHub {
            assistant_id,
            hub_id,
            is_template,
        }
    }

    /// Create an McpServerCreatedFromHub event
    pub fn mcp_server_created_from_hub(
        server_id: Uuid,
        hub_id: String,
        is_system: bool,
    ) -> Self {
        Self::McpServerCreatedFromHub {
            server_id,
            hub_id,
            is_system,
        }
    }

    /// Create a ModelDownloadStartedFromHub event
    pub fn model_download_started_from_hub(download_id: Uuid, hub_id: String) -> Self {
        Self::ModelDownloadStartedFromHub {
            download_id,
            hub_id,
        }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<HubEvent> for crate::core::events::AppEvent {
    fn from(event: HubEvent) -> Self {
        crate::core::events::AppEvent::Hub(event)
    }
}

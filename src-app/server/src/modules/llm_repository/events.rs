// LLM Repository events for inter-module communication
// Event infrastructure for future use

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::LlmRepository;

/// Events emitted by the LLM Repository module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmRepositoryEvent {
    /// A new repository was created
    Created { repository: LlmRepository },

    /// A repository was updated
    Updated { repository: LlmRepository },

    /// A repository was deleted
    Deleted { id: Uuid, name: String },

    /// An enabled repository failed its connection probe and was
    /// auto-disabled. Emitted from `connection_health`'s
    /// `enforce_on_create` and `enforce_on_update_transition` paths
    /// so the UI's list page reloads + the row's Alert renders in
    /// real time. The boot-time probe does NOT emit this — the
    /// EventBus isn't built yet at module init; mount-time refetch
    /// catches it.
    AutoDisabled { repo_id: Uuid, reason: String },
}

impl LlmRepositoryEvent {
    /// Create a Created event
    pub fn created(repository: LlmRepository) -> Self {
        Self::Created { repository }
    }

    /// Create an Updated event
    pub fn updated(repository: LlmRepository) -> Self {
        Self::Updated { repository }
    }

    /// Create a Deleted event
    pub fn deleted(id: Uuid, name: String) -> Self {
        Self::Deleted { id, name }
    }

    /// Create an AutoDisabled event
    pub fn auto_disabled(repo_id: Uuid, reason: String) -> Self {
        Self::AutoDisabled { repo_id, reason }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmRepositoryEvent> for crate::core::events::AppEvent {
    fn from(event: LlmRepositoryEvent) -> Self {
        crate::core::events::AppEvent::LlmRepository(event)
    }
}

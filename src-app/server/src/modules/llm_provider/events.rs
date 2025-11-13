// LLM Provider events for inter-module communication
// Event infrastructure for future use
#![allow(dead_code)]


use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::LlmProvider;

/// Events emitted by the LLM Provider module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmProviderEvent {
    /// A new provider was created
    Created { provider: LlmProvider },

    /// A provider was updated
    Updated { provider: LlmProvider },

    /// A provider was deleted
    Deleted { id: Uuid, name: String },

    /// Provider group assignments changed
    GroupAssignmentChanged {
        provider_id: Uuid,
        group_ids: Vec<Uuid>
    },
}

impl LlmProviderEvent {
    /// Create a Created event
    pub fn created(provider: LlmProvider) -> Self {
        Self::Created { provider }
    }

    /// Create an Updated event
    pub fn updated(provider: LlmProvider) -> Self {
        Self::Updated { provider }
    }

    /// Create a Deleted event
    pub fn deleted(id: Uuid, name: String) -> Self {
        Self::Deleted { id, name }
    }

    /// Create a GroupAssignmentChanged event
    pub fn group_assignment_changed(provider_id: Uuid, group_ids: Vec<Uuid>) -> Self {
        Self::GroupAssignmentChanged { provider_id, group_ids }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmProviderEvent> for crate::core::events::AppEvent {
    fn from(event: LlmProviderEvent) -> Self {
        crate::core::events::AppEvent::LlmProvider(event)
    }
}

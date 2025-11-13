// LLM Repository events for inter-module communication
// Event infrastructure for future use
#![allow(dead_code)]


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
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmRepositoryEvent> for crate::core::events::AppEvent {
    fn from(event: LlmRepositoryEvent) -> Self {
        crate::core::events::AppEvent::LlmRepository(event)
    }
}

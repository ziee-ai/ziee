// LLM Model events for inter-module communication
// Event infrastructure for future use

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::models::LlmModel;

/// Events emitted by the LLM Model module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmModelEvent {
    /// A new model was created
    Created { model: LlmModel },

    /// A model was updated
    Updated { model: LlmModel },

    /// A model was deleted
    Deleted { id: Uuid, name: String },
}

impl LlmModelEvent {
    /// Create a Created event
    pub fn created(model: LlmModel) -> Self {
        Self::Created { model }
    }

    /// Create an Updated event
    pub fn updated(model: LlmModel) -> Self {
        Self::Updated { model }
    }

    /// Create a Deleted event
    pub fn deleted(id: Uuid, name: String) -> Self {
        Self::Deleted { id, name }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmModelEvent> for crate::core::events::AppEvent {
    fn from(event: LlmModelEvent) -> Self {
        crate::core::events::AppEvent::LlmModel(event)
    }
}

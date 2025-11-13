// LLM Model events for inter-module communication
// Event infrastructure for future use
#![allow(dead_code)]

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

    /// A model download was started
    DownloadStarted { instance_id: Uuid, model_id: Uuid },

    /// A model download was completed
    DownloadCompleted { instance_id: Uuid, model_id: Uuid },

    /// A model download failed
    DownloadFailed {
        instance_id: Uuid,
        model_id: Uuid,
        error: String,
    },
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

    /// Create a DownloadStarted event
    pub fn download_started(instance_id: Uuid, model_id: Uuid) -> Self {
        Self::DownloadStarted {
            instance_id,
            model_id,
        }
    }

    /// Create a DownloadCompleted event
    pub fn download_completed(instance_id: Uuid, model_id: Uuid) -> Self {
        Self::DownloadCompleted {
            instance_id,
            model_id,
        }
    }

    /// Create a DownloadFailed event
    pub fn download_failed(instance_id: Uuid, model_id: Uuid, error: String) -> Self {
        Self::DownloadFailed {
            instance_id,
            model_id,
            error,
        }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmModelEvent> for crate::core::events::AppEvent {
    fn from(event: LlmModelEvent) -> Self {
        crate::core::events::AppEvent::LlmModel(event)
    }
}

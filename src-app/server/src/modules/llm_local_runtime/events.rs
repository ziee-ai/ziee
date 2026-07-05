// LLM Local Runtime events for inter-module communication
// Event infrastructure for cache invalidation and UI updates

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Events emitted by the LLM Local Runtime module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmLocalRuntimeEvent {
    /// A model instance was started
    InstanceStarted {
        instance_id: Uuid,
        model_id: Uuid,
        provider_id: Uuid,
    },

    /// A model instance was stopped
    InstanceStopped {
        instance_id: Uuid,
        model_id: Uuid,
    },

    /// A model instance was restarted
    InstanceRestarted {
        instance_id: Uuid,
        model_id: Uuid,
    },

    /// A model instance status changed
    InstanceStatusChanged {
        instance_id: Uuid,
        model_id: Uuid,
        old_status: String,
        new_status: String,
    },

    /// A runtime version was downloaded
    RuntimeVersionDownloaded {
        version_id: Uuid,
        engine: String,
        version: String,
    },

    /// A runtime version was deleted
    RuntimeVersionDeleted {
        version_id: Uuid,
        engine: String,
        version: String,
    },

    /// The system default runtime version was changed
    RuntimeVersionDefaultChanged {
        version_id: Uuid,
        engine: String,
        version: String,
    },
}

impl LlmLocalRuntimeEvent {
    /// Create an InstanceStarted event
    pub fn instance_started(instance_id: Uuid, model_id: Uuid, provider_id: Uuid) -> Self {
        Self::InstanceStarted {
            instance_id,
            model_id,
            provider_id,
        }
    }

    /// Create an InstanceStopped event
    pub fn instance_stopped(instance_id: Uuid, model_id: Uuid) -> Self {
        Self::InstanceStopped {
            instance_id,
            model_id,
        }
    }

    /// Create an InstanceRestarted event
    pub fn instance_restarted(instance_id: Uuid, model_id: Uuid) -> Self {
        Self::InstanceRestarted {
            instance_id,
            model_id,
        }
    }

    /// Create an InstanceStatusChanged event. Emitted by the admin
    /// `clear-failed` endpoint (failed -> stopped); the enum is also the vehicle
    /// for future health-state surfacing.
    pub fn instance_status_changed(
        instance_id: Uuid,
        model_id: Uuid,
        old_status: String,
        new_status: String,
    ) -> Self {
        Self::InstanceStatusChanged {
            instance_id,
            model_id,
            old_status,
            new_status,
        }
    }

    /// Create a RuntimeVersionDownloaded event
    // Scaffolding: paired with the not-yet-emitted RuntimeVersionDownloaded flow.
    #[allow(dead_code)]
    pub fn runtime_version_downloaded(version_id: Uuid, engine: String, version: String) -> Self {
        Self::RuntimeVersionDownloaded {
            version_id,
            engine,
            version,
        }
    }

    /// Create a RuntimeVersionDeleted event
    pub fn runtime_version_deleted(version_id: Uuid, engine: String, version: String) -> Self {
        Self::RuntimeVersionDeleted {
            version_id,
            engine,
            version,
        }
    }

    /// Create a RuntimeVersionDefaultChanged event
    pub fn runtime_version_default_changed(
        version_id: Uuid,
        engine: String,
        version: String,
    ) -> Self {
        Self::RuntimeVersionDefaultChanged {
            version_id,
            engine,
            version,
        }
    }
}

// Implement Into<AppEvent> for convenient event emission
impl From<LlmLocalRuntimeEvent> for crate::core::events::AppEvent {
    fn from(event: LlmLocalRuntimeEvent) -> Self {
        crate::core::events::AppEvent::LlmLocalRuntime(event)
    }
}

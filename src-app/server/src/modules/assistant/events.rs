// Assistant module events
// Event infrastructure for future use

// Events related to assistant lifecycle

use uuid::Uuid;

/// Events emitted by the assistant module
// Emit-only lifecycle events: created/updated/deleted are published but no
// subscriber reads their payloads yet, and Enabled/Disabled aren't emitted
// yet — retained as the module's event vocabulary for future subscribers.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AssistantEvent {
    /// A new assistant was created
    Created {
        assistant_id: Uuid,
        user_id: Option<Uuid>,
    },

    /// An assistant was updated
    Updated {
        assistant_id: Uuid,
        user_id: Option<Uuid>,
    },

    /// An assistant was deleted
    Deleted {
        assistant_id: Uuid,
        user_id: Option<Uuid>,
    },

    /// An assistant was enabled
    Enabled {
        assistant_id: Uuid,
        user_id: Option<Uuid>,
    },

    /// An assistant was disabled
    Disabled {
        assistant_id: Uuid,
        user_id: Option<Uuid>,
    },
}

// Emit helpers: created/updated/deleted are wired; enabled/disabled retained
// for the (not-yet-emitted) enable/disable lifecycle transitions.
#[allow(dead_code)]
impl AssistantEvent {
    /// Helper to create an AssistantCreated event wrapped in AppEvent
    pub fn created(assistant_id: Uuid, user_id: Option<Uuid>) -> crate::core::AppEvent {
        crate::core::AppEvent::Assistant(AssistantEvent::Created {
            assistant_id,
            user_id,
        })
    }

    /// Helper to create an AssistantUpdated event wrapped in AppEvent
    pub fn updated(assistant_id: Uuid, user_id: Option<Uuid>) -> crate::core::AppEvent {
        crate::core::AppEvent::Assistant(AssistantEvent::Updated {
            assistant_id,
            user_id,
        })
    }

    /// Helper to create an AssistantDeleted event wrapped in AppEvent
    pub fn deleted(assistant_id: Uuid, user_id: Option<Uuid>) -> crate::core::AppEvent {
        crate::core::AppEvent::Assistant(AssistantEvent::Deleted {
            assistant_id,
            user_id,
        })
    }

    /// Helper to create an AssistantEnabled event wrapped in AppEvent
    pub fn enabled(assistant_id: Uuid, user_id: Option<Uuid>) -> crate::core::AppEvent {
        crate::core::AppEvent::Assistant(AssistantEvent::Enabled {
            assistant_id,
            user_id,
        })
    }

    /// Helper to create an AssistantDisabled event wrapped in AppEvent
    pub fn disabled(assistant_id: Uuid, user_id: Option<Uuid>) -> crate::core::AppEvent {
        crate::core::AppEvent::Assistant(AssistantEvent::Disabled {
            assistant_id,
            user_id,
        })
    }
}

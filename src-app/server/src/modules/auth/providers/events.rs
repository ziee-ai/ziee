//! Auth-provider lifecycle events. Mirrors
//! `llm_repository/events.rs` — gives in-process Rust handlers a typed
//! hook for the same transitions the frontend hears about via sync
//! (`auth_provider.*` on the EventBus and `sync:auth_provider` via the
//! SSE stream).
//!
//! Notify-only payloads: the variants carry `{id}` / `{id, reason}` and
//! never the full row. `AuthProvider.config` holds plaintext secrets at
//! rest (masking happens only when serializing the HTTP response), so
//! the bus payload deliberately stays narrow to keep handler-side
//! logging or future fan-out from leaking credentials.

use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum AuthProviderEvent {
    /// A new provider was created. Refetch via the list endpoint for
    /// the row data (which masks secrets).
    Created { id: Uuid },

    /// A provider was updated.
    Updated { id: Uuid },

    /// A provider was deleted. Linked user accounts remain.
    Deleted { id: Uuid, name: String },

    /// An enabled provider failed its connection probe and was
    /// auto-disabled. Emitted from `health::enforce_on_update_transition`,
    /// `health::enforce_on_create_with_enabled`, and
    /// `health::record_test_outcome` — every path where the probe can
    /// flip `enabled=false` server-side.
    AutoDisabled { id: Uuid, reason: String },
}

impl AuthProviderEvent {
    pub fn created(id: Uuid) -> Self {
        Self::Created { id }
    }
    pub fn updated(id: Uuid) -> Self {
        Self::Updated { id }
    }
    pub fn deleted(id: Uuid, name: String) -> Self {
        Self::Deleted { id, name }
    }
    pub fn auto_disabled(id: Uuid, reason: String) -> Self {
        Self::AutoDisabled { id, reason }
    }
}

impl From<AuthProviderEvent> for crate::core::events::AppEvent {
    fn from(event: AuthProviderEvent) -> Self {
        crate::core::events::AppEvent::AuthProvider(event)
    }
}

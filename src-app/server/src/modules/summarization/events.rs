//! Summarization lifecycle events. Notify-only — the variants carry
//! just enough info to drive the frontend's refetch ({id} or {id, name}).
//!
//! Mirrors `auth/providers/events.rs` shape (no embedded row payload).
//! The persisted admin settings live in `summarization_admin_settings`;
//! the frontend refetches via the existing permission-checked REST
//! endpoint, so the bus payload deliberately stays narrow to keep any
//! future handler-side logging from leaking custom prompt content.

use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum SummarizationEvent {
    /// The singleton settings row was updated. `id` is the row id
    /// (always 1) for shape parity with the other settings events.
    Updated { id: Uuid },
}

impl SummarizationEvent {
    pub fn updated(id: Uuid) -> Self {
        Self::Updated { id }
    }
}

impl From<SummarizationEvent> for crate::core::events::AppEvent {
    fn from(event: SummarizationEvent) -> Self {
        crate::core::events::AppEvent::Summarization(event)
    }
}

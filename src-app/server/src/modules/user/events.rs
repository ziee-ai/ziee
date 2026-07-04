// User module events
// Event infrastructure for future use

// Events related to user lifecycle and authentication

use super::models::User;
use uuid::Uuid;

/// Events emitted by the user module
// Emit-only lifecycle events: `created` is consumed (assistant module reads
// Created), but Updated/Deleted payloads have no subscriber yet and
// LoggedIn/LoggedOut aren't emitted at all — retained as the module's event
// vocabulary for future subscribers (e.g. login audit).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UserEvent {
    /// A new user was created
    Created { user: User },

    /// An existing user was updated
    Updated { user: User },

    /// A user was deleted
    Deleted { user_id: Uuid },

    /// A user logged in
    LoggedIn { user_id: Uuid },

    /// A user logged out
    LoggedOut { user_id: Uuid },
}

// created/updated/deleted are wired; logged_in/logged_out retained for the
// not-yet-emitted auth-lifecycle events.
#[allow(dead_code)]
impl UserEvent {
    /// Helper to create a UserCreated event wrapped in AppEvent
    pub fn created(user: User) -> crate::core::AppEvent {
        crate::core::AppEvent::User(UserEvent::Created { user })
    }

    /// Helper to create a UserUpdated event wrapped in AppEvent
    pub fn updated(user: User) -> crate::core::AppEvent {
        crate::core::AppEvent::User(UserEvent::Updated { user })
    }

    /// Helper to create a UserDeleted event wrapped in AppEvent
    pub fn deleted(user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::User(UserEvent::Deleted { user_id })
    }

    /// Helper to create a UserLoggedIn event wrapped in AppEvent
    pub fn logged_in(user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::User(UserEvent::LoggedIn { user_id })
    }

    /// Helper to create a UserLoggedOut event wrapped in AppEvent
    pub fn logged_out(user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::User(UserEvent::LoggedOut { user_id })
    }
}

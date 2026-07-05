// User module events
// Event infrastructure for future use

// Events related to user lifecycle and authentication

use super::models::User;
use uuid::Uuid;

/// Events emitted by the user module
// Emit-only lifecycle events: `created` is consumed (assistant module reads
// Created); Updated/Deleted payloads are emitted but have no subscriber yet —
// retained as the module's event vocabulary. (LoggedIn/LoggedOut variants were
// removed: they were never emitted anywhere and had no subscriber — dead
// vocabulary for a login-audit feature that isn't wired. Re-add them alongside
// their emit site + subscriber if/when that feature lands.)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum UserEvent {
    /// A new user was created
    Created { user: User },

    /// An existing user was updated
    Updated { user: User },

    /// A user was deleted
    Deleted { user_id: Uuid },
}

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
}

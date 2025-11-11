// User module events
// Events related to user lifecycle and authentication

use uuid::Uuid;
use super::models::User;

/// Events emitted by the user module
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

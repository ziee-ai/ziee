// Auth module events
// Events related to authentication and authorization
// These are part of the event system infrastructure for future use

use uuid::Uuid;

/// Events emitted by the auth module
// Future event-infrastructure: no emitter/subscriber wired yet (kept as the
// intended auth-event vocabulary). Narrow allow instead of the old
// module-level blanket so new dead code in this file is still caught.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AuthEvent {
    /// User successfully authenticated
    UserAuthenticated { user_id: Uuid, provider: String },

    /// User authentication failed
    AuthenticationFailed { username: String, reason: String },

    /// User session refreshed
    SessionRefreshed { user_id: Uuid },

    /// User session expired
    SessionExpired { user_id: Uuid },
}

#[allow(dead_code)] // future event-infrastructure constructors; see enum above
impl AuthEvent {
    /// Helper to create a UserAuthenticated event wrapped in AppEvent
    pub fn user_authenticated(user_id: Uuid, provider: String) -> crate::core::AppEvent {
        crate::core::AppEvent::Auth(AuthEvent::UserAuthenticated { user_id, provider })
    }

    /// Helper to create an AuthenticationFailed event wrapped in AppEvent
    pub fn authentication_failed(username: String, reason: String) -> crate::core::AppEvent {
        crate::core::AppEvent::Auth(AuthEvent::AuthenticationFailed { username, reason })
    }

    /// Helper to create a SessionRefreshed event wrapped in AppEvent
    pub fn session_refreshed(user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Auth(AuthEvent::SessionRefreshed { user_id })
    }

    /// Helper to create a SessionExpired event wrapped in AppEvent
    pub fn session_expired(user_id: Uuid) -> crate::core::AppEvent {
        crate::core::AppEvent::Auth(AuthEvent::SessionExpired { user_id })
    }
}

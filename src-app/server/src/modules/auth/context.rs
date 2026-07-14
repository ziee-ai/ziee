//! Injected seams for the auth (+ co-located user) module — Chunk BG.
//!
//! The auth module used to reach a set of app-global singletons directly (the
//! global repository aggregator, the in-process event bus, the sync-publish
//! functions, the at-rest secret key, and the SSRF URL-validator helpers).
//! Those globals are what would keep a future `ziee-auth` crate from
//! compiling against only `ziee-core` / `ziee-framework` / `ziee-identity`.
//!
//! This module inverts the dependency: the auth module now depends on the
//! **abstractions declared here** (`AuthEventSink`, `AuthSyncSink`) and takes a
//! per-request [`AuthContext`] handle carrying a `PgPool` + those sinks. The
//! APP installs concrete implementations (backed by the real event bus / sync
//! publish) once at boot and layers a single `Extension<AuthContext>` onto the
//! router — see `core::events` for the installed impls and `lib.rs` / `main.rs`
//! for the wiring. Behaviour is byte-identical; only the coupling direction
//! changed.

use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::auth::providers::events::AuthProviderEvent;
use crate::modules::auth::{AuthRepository, SessionSettingsRepository};
use crate::modules::sync::{Audience, SyncAction, SyncEntity};
use crate::modules::user::events::UserEvent;
use crate::modules::user::{GroupRepository, UserRepository};

/// Emit in-process domain events. Wired app-side to the real event bus
/// (the impl wraps each module event into the app-aggregate event enum).
pub trait AuthEventSink: Send + Sync {
    /// Fire a user-lifecycle event (created / updated / deleted).
    fn emit_user(&self, ev: UserEvent);
    /// Fire an auth-provider-lifecycle event.
    fn emit_auth_provider(&self, ev: AuthProviderEvent);
}

/// Publish cross-device sync notifications. Wired app-side to
/// `crate::modules::sync::{publish, publish_session_to_users}` — the auth
/// module no longer names the global sync-publish functions. It still uses the
/// app's `SyncEntity` / `Audience` value types (those become app-extensible
/// in Chunk B5); BG only removes the direct global-function call.
pub trait AuthSyncSink: Send + Sync {
    /// Notify-and-refetch for a single entity to the chosen audience.
    fn publish(
        &self,
        entity: SyncEntity,
        action: SyncAction,
        id: Uuid,
        audience: Audience,
        origin: Option<Uuid>,
    );
    /// Fan a `Session` permissions-changed signal out to many users at once.
    fn publish_session_to_users(&self, user_ids: &[Uuid], origin: Option<Uuid>);
}

/// Per-request dependency handle the auth + user handlers pull from
/// `Extension<AuthContext>` instead of reaching app globals. Cheaply
/// cloneable (everything behind `Arc`).
#[derive(Clone)]
pub struct AuthContext {
    pool: Arc<PgPool>,
    /// The at-rest secret storage key (app copies it from
    /// the app's at-rest secret key at install time).
    secret_key: Option<String>,
    /// Domain-event sink (app installs an event-bus-backed impl).
    pub events: Arc<dyn AuthEventSink>,
    /// Cross-device sync sink (app installs a `sync::publish`-backed impl).
    pub sync: Arc<dyn AuthSyncSink>,
}

impl AuthContext {
    /// Assemble the handle from a pool + the installed sinks. Called once at
    /// boot by the app wiring.
    pub fn new(
        pool: Arc<PgPool>,
        secret_key: Option<String>,
        events: Arc<dyn AuthEventSink>,
        sync: Arc<dyn AuthSyncSink>,
    ) -> Self {
        Self {
            pool,
            secret_key,
            events,
            sync,
        }
    }

    /// The shared connection pool (replaces `Repos.pool()`).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// The at-rest secret storage key (replaces the global storage-key read).
    pub fn secret_key(&self) -> Option<&str> {
        self.secret_key.as_deref()
    }

    /// A fresh auth repository bound to the pool (replaces `Repos.auth`).
    /// Repositories are stateless pool wrappers, so per-call construction is
    /// behaviourally identical to the cached global accessor.
    pub fn auth(&self) -> AuthRepository {
        AuthRepository::new((*self.pool).clone())
    }

    /// A fresh user repository (replaces `Repos.user`).
    pub fn user(&self) -> UserRepository {
        UserRepository::new((*self.pool).clone())
    }

    /// A fresh group repository (replaces `Repos.group`).
    pub fn group(&self) -> GroupRepository {
        GroupRepository::new((*self.pool).clone())
    }

    /// A fresh session-settings repository (replaces `Repos.session_settings`).
    pub fn session_settings(&self) -> SessionSettingsRepository {
        SessionSettingsRepository::new((*self.pool).clone())
    }
}

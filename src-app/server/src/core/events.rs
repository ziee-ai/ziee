// Event system for inter-module communication
// Allows modules to react to events without tight coupling
//
// Each module defines its own events in modules/{module}/events.rs
// This core file aggregates all module events into a single AppEvent enum
//
// Event infrastructure - currently unused but part of the core architecture

use sqlx::PgPool;
use std::sync::Arc;

// The domain-free `EventHandler` trait moved to `ziee-framework` (Chunk B2) —
// its `handle` takes the event type-erased (`&dyn Any`) so the framework stays
// app-agnostic. Re-exported here so `crate::core::events::EventHandler` /
// `crate::core::EventHandler` call sites are unchanged. The domain-coupled
// `AppEvent` enum + the `EventBus` dispatcher stay app-side (they move in B5).
pub use ziee_framework::EventHandler;

/// Main application event enum
/// Each module contributes a variant containing its module-specific events
// Variant payloads are constructed at emit sites but not yet destructured by
// any handler — core event-bus scaffolding wired ahead of consumers.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User module events (user lifecycle, authentication)
    User(crate::modules::user::events::UserEvent),

    /// Auth module events (authentication, authorization)
    Auth(crate::modules::auth::events::AuthEvent),

    /// Assistant module events (assistant lifecycle)
    Assistant(crate::modules::assistant::events::AssistantEvent),

    /// MCP Server module events (server lifecycle)
    McpServer(crate::modules::mcp::events::McpServerEvent),

    /// LLM Provider module events (provider lifecycle, group assignments)
    LlmProvider(crate::modules::llm_provider::events::LlmProviderEvent),

    /// LLM Model module events (model lifecycle, downloads)
    LlmModel(crate::modules::llm_model::events::LlmModelEvent),

    /// LLM Repository module events (repository lifecycle)
    LlmRepository(crate::modules::llm_repository::events::LlmRepositoryEvent),

    /// Hub module events (catalog refreshes, entity creation from hub)
    Hub(crate::modules::hub::events::HubEvent),

    /// LLM Local Runtime module events (instances, versions)
    LlmLocalRuntime(crate::modules::llm_local_runtime::events::LlmLocalRuntimeEvent),

    /// Project module events (project lifecycle: created, updated, deleted,
    /// conversation attach/detach). File-project events live under
    /// `FileProject` — file module owns the `project_files` join table after
    /// the project↔file inversion.
    Project(crate::modules::project::events::ProjectEvent),

    /// File module's project-extension events (project_files lifecycle:
    /// file attached to / detached from a project). Owned by the file
    /// module per the project↔file inversion.
    FileProject(crate::modules::file::project_extension::events::FileProjectEvent),

    /// Auth provider admin lifecycle: create/update/delete plus the
    /// `AutoDisabled` signal a probe failure flips an enabled row to
    /// disabled. Mirrors `LlmRepository`.
    AuthProvider(crate::modules::auth::providers::events::AuthProviderEvent),

    /// Summarization admin settings updated. Notify-only — no row
    /// payload. The frontend refetches via the existing REST endpoint.
    Summarization(crate::modules::summarization::events::SummarizationEvent),
    // Add new module events here as the application grows
}

/// Hard cap on concurrent in-flight emit_async tasks. Closes
/// 14-core F-15 (Medium): the original `emit_async` spawned an
/// unbounded tokio task per emission. Under burst load (a chat storm
/// triggering 1000s of LlmModel/Assistant events) the spawn rate had
/// no ceiling → memory bloats with pending closures + handler-task
/// state. The semaphore bounds the burst; on saturation we DROP the
/// new event with a tracing::warn rather than blocking the producer
/// (matches the original fire-and-forget contract). Operators see the
/// warn and know to scale handlers or move heavy work off-bus.
const EVENT_BUS_MAX_INFLIGHT: usize = 1024;

/// Event bus manages event handler registration and event dispatch
pub struct EventBus {
    handlers: Vec<Arc<dyn EventHandler>>,
    pool: Arc<PgPool>,
    inflight: Arc<tokio::sync::Semaphore>,
}

impl EventBus {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            handlers: Vec::new(),
            pool,
            inflight: Arc::new(tokio::sync::Semaphore::new(EVENT_BUS_MAX_INFLIGHT)),
        }
    }

    /// Register an event handler
    pub fn register(&mut self, handler: Arc<dyn EventHandler>) {
        tracing::debug!("Registering event handler: {}", handler.handler_name());
        self.handlers.push(handler);
    }

    /// Emit an event to all registered handlers (blocking)
    /// Waits for all handlers to complete before returning
    pub async fn emit(&self, event: AppEvent) {
        tracing::debug!("Emitting event: {:?}", event);

        for handler in &self.handlers {
            // `&AppEvent` erases to the framework handler's `&dyn Any` param;
            // each handler downcasts back to `AppEvent`.
            if let Err(e) = handler
                .handle(&event as &(dyn std::any::Any + Send + Sync), &self.pool)
                .await
            {
                tracing::error!(
                    "Event handler '{}' failed for event {:?}: {}",
                    handler.handler_name(),
                    event,
                    e
                );
            }
        }
    }

    /// Emit event in background (non-blocking).
    ///
    /// Returns immediately without waiting for handlers to complete.
    /// Bounded by `EVENT_BUS_MAX_INFLIGHT`: when the semaphore is
    /// saturated, the new event is DROPPED with a tracing::warn (we
    /// preserve the fire-and-forget contract — producers shouldn't
    /// stall on event-bus backpressure). Closes 14-core F-15.
    pub fn emit_async(&self, event: AppEvent) {
        let handlers = self.handlers.clone();
        let pool = self.pool.clone();
        let inflight = self.inflight.clone();

        tracing::debug!("Emitting event asynchronously: {:?}", event);

        // try_acquire is non-blocking — exactly the right primitive
        // for "drop on saturation". A truly blocking acquire would
        // turn fire-and-forget emitters into a sync stall point.
        let permit = match inflight.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!(
                    event = ?event,
                    cap = EVENT_BUS_MAX_INFLIGHT,
                    "EventBus inflight cap reached; dropping event"
                );
                return;
            }
        };

        tokio::spawn(async move {
            for handler in handlers {
                if let Err(e) = handler
                    .handle(&event as &(dyn std::any::Any + Send + Sync), &pool)
                    .await
                {
                    tracing::error!(
                        "Event handler '{}' failed for event {:?}: {}",
                        handler.handler_name(),
                        event,
                        e
                    );
                }
            }
            // permit drops here, freeing one slot.
            drop(permit);
        });
    }

    /// Get the number of registered handlers
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            handlers: self.handlers.clone(),
            pool: self.pool.clone(),
            // Share the semaphore: every clone of the bus participates
            // in the same global inflight cap, otherwise N clones × N
            // permits = unbounded again.
            inflight: self.inflight.clone(),
        }
    }
}

// ─────────────────────── Chunk BG: auth-seam impls ───────────────────────
//
// The auth (+ user) module declares the `AuthEventSink` / `AuthSyncSink` /
// `OutboundHttp` abstractions (see `modules::auth::context`) and no longer
// names the `EventBus`, `sync::publish`, or `url_validator` globals. The APP
// installs these concrete impls — the only place the global singletons and the
// app-aggregate `AppEvent` are named for the auth event/sync/outbound paths —
// and hands the assembled `AuthContext` to the router as an extension.

use crate::modules::auth::context::{AuthContext, AuthEventSink, AuthSyncSink};
use crate::modules::auth::providers::events::AuthProviderEvent;
use crate::modules::sync::{Audience, SyncAction, SyncEntity};
use crate::modules::user::events::UserEvent;

/// `EventBus`-backed event sink. Wraps each module event into the
/// app-aggregate `AppEvent` and fires it fire-and-forget — byte-identical to
/// the former `event_bus.emit_async(UserEvent::created(..))` call sites.
struct EventBusAuthSink {
    bus: Arc<EventBus>,
}

impl AuthEventSink for EventBusAuthSink {
    fn emit_user(&self, ev: UserEvent) {
        self.bus.emit_async(AppEvent::User(ev));
    }
    fn emit_auth_provider(&self, ev: AuthProviderEvent) {
        self.bus.emit_async(AppEvent::AuthProvider(ev));
    }
}

/// `sync::publish`-backed sync sink — the single place the auth/user sync
/// notifications reach the global publish functions.
struct PublishSyncSink;

impl AuthSyncSink for PublishSyncSink {
    fn publish(
        &self,
        entity: SyncEntity,
        action: SyncAction,
        id: uuid::Uuid,
        audience: Audience,
        origin: Option<uuid::Uuid>,
    ) {
        crate::modules::sync::publish(entity, action, id, audience, origin);
    }
    fn publish_session_to_users(&self, user_ids: &[uuid::Uuid], origin: Option<uuid::Uuid>) {
        crate::modules::sync::publish_session_to_users(user_ids, origin);
    }
}

/// Assemble the [`AuthContext`] the auth/user handlers pull from the request
/// extensions, installing the app-backed sinks. Called once at boot by
/// `lib.rs` / `main.rs` (and thus the desktop embed).
pub fn build_auth_context(pool: Arc<PgPool>, event_bus: Arc<EventBus>) -> AuthContext {
    AuthContext::new(
        pool,
        crate::core::secrets::storage_key().map(|s| s.to_string()),
        Arc::new(EventBusAuthSink { bus: event_bus }),
        Arc::new(PublishSyncSink),
    )
}

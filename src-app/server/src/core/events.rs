// Event system for inter-module communication
// Allows modules to react to events without tight coupling
//
// Each module defines its own events in modules/{module}/events.rs
// This core file aggregates all module events into a single AppEvent enum
//
// Event infrastructure - currently unused but part of the core architecture
#![allow(dead_code)]

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;

use crate::common::AppError;

/// Main application event enum
/// Each module contributes a variant containing its module-specific events
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
    // Add new module events here as the application grows
}

/// Trait for handling application events
/// Modules implement this to react to events they care about
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    /// Return error to log it, but won't stop other handlers from running
    async fn handle(&self, event: &AppEvent, pool: &PgPool) -> Result<(), AppError>;

    /// Name for logging and debugging
    fn handler_name(&self) -> &'static str;
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
            if let Err(e) = handler.handle(&event, &self.pool).await {
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
                if let Err(e) = handler.handle(&event, &pool).await {
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

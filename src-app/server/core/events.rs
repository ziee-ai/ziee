// Event system for inter-module communication
// Allows modules to react to events without tight coupling

use async_trait::async_trait;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::user::User;

/// Events that can be emitted throughout the application
#[derive(Debug, Clone)]
pub enum AppEvent {
    // User lifecycle events
    UserCreated { user: User },
    UserUpdated { user: User },
    UserDeleted { user_id: Uuid },

    // Authentication events
    UserLoggedIn { user_id: Uuid },
    UserLoggedOut { user_id: Uuid },

    // Assistant events (for future use)
    AssistantCreated { assistant_id: Uuid, user_id: Option<Uuid> },
    AssistantDeleted { assistant_id: Uuid, user_id: Option<Uuid> },

    // Add more events as needed...
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

/// Event bus manages event handler registration and event dispatch
pub struct EventBus {
    handlers: Vec<Arc<dyn EventHandler>>,
    pool: Arc<PgPool>,
}

impl EventBus {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self {
            handlers: Vec::new(),
            pool,
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

    /// Emit event in background (non-blocking)
    /// Returns immediately without waiting for handlers to complete
    pub fn emit_async(&self, event: AppEvent) {
        let handlers = self.handlers.clone();
        let pool = self.pool.clone();

        tracing::debug!("Emitting event asynchronously: {:?}", event);

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
        }
    }
}

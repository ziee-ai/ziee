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

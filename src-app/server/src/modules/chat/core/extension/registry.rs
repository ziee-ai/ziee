// Extension registry for chat module
//
// Provides a plugin system for extending chat functionality without modifying base code.

use async_trait::async_trait;
use axum::response::sse::Event;
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use ai_providers::ChatRequest;

use crate::common::AppError;
use crate::modules::chat::core::models::{content::MessageContentData, Message};
use super::request::SendMessageRequest;

/// Action returned by extensions after LLM call completes
#[derive(Debug, Clone)]
pub enum ExtensionAction {
    /// Stop streaming, conversation turn is complete (default behavior)
    Complete,

    /// Continue with another LLM call (for tool execution, etc.)
    /// Extension provides content to send back to LLM as user message
    Continue {
        /// Content blocks to send as user message (tool results, etc.)
        user_message_content: Vec<MessageContentData>,
    },
}

/// Context passed to extension hooks during streaming
#[derive(Clone)]
pub struct StreamContext {
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Option<Uuid>,
    pub user_id: Uuid,
    pub pool: PgPool,
    pub metadata: HashMap<String, serde_json::Value>,
    /// Current iteration number (1-indexed, for tool calling loops)
    pub iteration: u32,
}

/// Extension trait for chat functionality
#[async_trait]
pub trait ChatExtension: Send + Sync {
    /// Extension name
    fn name(&self) -> &str;

    /// Called before sending request to LLM
    /// Extensions can read SendMessageRequest.extensions and modify ChatRequest
    /// (e.g., add tools, inject context, modify parameters)
    async fn before_llm_call(
        &self,
        _context: &mut StreamContext,
        _request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        Ok(())
    }

    /// Called after LLM response stream completes
    /// Extensions can perform post-processing (e.g., generate title, execute tools)
    /// Returns action to take: Complete (stop) or Continue (make another LLM call)
    async fn after_llm_call(
        &self,
        _context: &StreamContext,
        _final_message: &Message,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        Ok(ExtensionAction::Complete)
    }

    /// Initialize extension (called once at startup)
    async fn initialize(&self, _pool: &PgPool) -> Result<(), AppError> {
        Ok(())
    }
}

/// Registry for managing chat extensions
pub struct ExtensionRegistry {
    extensions: Vec<Arc<dyn ChatExtension>>,
}

impl ExtensionRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    /// Register an extension
    pub fn register(&mut self, extension: Arc<dyn ChatExtension>) {
        println!("Registering chat extension: {}", extension.name());
        self.extensions.push(extension);
    }

    /// Initialize all registered extensions
    pub async fn initialize_all(&self, pool: &PgPool) -> Result<(), AppError> {
        for ext in &self.extensions {
            ext.initialize(pool).await?;
        }
        Ok(())
    }

    /// Call before_llm_call on all extensions
    pub async fn call_before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        for ext in &self.extensions {
            ext.before_llm_call(context, request, send_request).await?;
        }
        Ok(())
    }

    /// Call after_llm_call on all extensions
    /// Returns first Continue action encountered, or Complete if all extensions return Complete
    pub async fn call_after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        for ext in &self.extensions {
            let action = ext.after_llm_call(context, final_message, tx).await?;

            // If any extension returns Continue, stop iterating and return it
            if matches!(action, ExtensionAction::Continue { .. }) {
                return Ok(action);
            }
        }

        // All extensions returned Complete
        Ok(ExtensionAction::Complete)
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

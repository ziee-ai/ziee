// Chat extension infrastructure
#![allow(dead_code)]

// Extension registry for chat module
//
// Provides a plugin system for extending chat functionality without modifying base code.

use aide::axum::ApiRouter;
use async_trait::async_trait;
use axum::response::sse::Event;
use linkme::distributed_slice;
use sqlx::PgPool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use ai_providers::{ChatRequest, ContentBlock};

use super::request::SendMessageRequest;
use crate::common::AppError;
use crate::modules::chat::core::models::{Message, content::MessageContentData};

/// Extension registration entry for distributed collection
#[derive(Debug, Clone, Copy)]
pub struct ExtensionEntry {
    pub name: &'static str,
    pub order: i32,
    pub factory: fn(PgPool) -> Arc<dyn ChatExtension>,
}

/// Distributed slice for collecting all chat extensions
#[distributed_slice]
pub static CHAT_EXTENSIONS: [ExtensionEntry] = [..];

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

    // ========== ROUTE REGISTRATION ==========

    /// Register custom routes for this extension
    /// Extensions can add their own API endpoints (e.g., file upload, tool approval)
    /// Routes are typically nested under /chat/<extension-name>/
    fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        router // Default: no routes
    }

    // ========== CONTENT TYPE HANDLING ==========

    /// Returns content types this extension handles
    /// Example: ["file", "tool_approval"]
    /// When content of these types is processed, the extension's hooks will be called
    fn handled_content_types(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Process content before sending to LLM
    /// Called when preparing chat history for LLM request
    /// Extension can transform content (e.g., file → text description, image → alt text)
    /// Return Some(ContentBlock) to replace content, None to use default conversion
    async fn process_content_for_llm(
        &self,
        _content: &MessageContentData,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        Ok(None) // Default: no transformation
    }

    /// Process content after retrieving from database
    /// Called when loading message history
    /// Extension can enrich content (e.g., add download URLs, resolve references)
    /// Modifies content in-place
    async fn process_content_from_db(
        &self,
        _content: &mut MessageContentData,
        _context: &StreamContext,
    ) -> Result<(), AppError> {
        Ok(()) // Default: no processing
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

    // ========== ROUTE REGISTRATION ==========

    /// Register routes from all extensions
    /// Collects routes from all extensions and merges them into the router
    pub fn register_routes(&self, router: ApiRouter) -> ApiRouter {
        self.extensions
            .iter()
            .fold(router, |router, ext| ext.register_routes(router))
    }

    // ========== CONTENT TYPE HANDLING ==========

    /// Find extension that handles given content type
    /// Returns first extension that declares it handles this content type
    pub fn get_handler_for_content_type(
        &self,
        content_type: &str,
    ) -> Option<&Arc<dyn ChatExtension>> {
        self.extensions
            .iter()
            .find(|ext| ext.handled_content_types().contains(&content_type))
    }

    /// Process content for LLM across all extensions
    /// Finds handler for content type and calls process_content_for_llm
    /// Returns transformed ContentBlock if extension provides one, None otherwise
    pub async fn process_content_for_llm(
        &self,
        content: &MessageContentData,
        context: &StreamContext,
    ) -> Result<Option<ContentBlock>, AppError> {
        let content_type = content.content_type();
        if let Some(handler) = self.get_handler_for_content_type(content_type) {
            handler.process_content_for_llm(content, context).await
        } else {
            Ok(None)
        }
    }

    /// Process content from database across all extensions
    /// Finds handler for content type and calls process_content_from_db
    /// Modifies content in-place
    pub async fn process_content_from_db(
        &self,
        content: &mut MessageContentData,
        context: &StreamContext,
    ) -> Result<(), AppError> {
        let content_type = content.content_type();
        if let Some(handler) = self.get_handler_for_content_type(content_type) {
            handler.process_content_from_db(content, context).await
        } else {
            Ok(())
        }
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

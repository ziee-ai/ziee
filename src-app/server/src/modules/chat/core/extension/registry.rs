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
use crate::modules::chat::core::types::streaming::ContentBlockDelta;

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
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
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

    /// Process a streaming delta during LLM response
    /// Called for each content delta that core streaming doesn't handle
    /// Extensions can convert ai-providers deltas to their own ContentBlockDelta variants
    /// Return Some(ContentBlockDelta) to accumulate and stream, None to ignore
    async fn process_delta(
        &self,
        _delta: &ai_providers::ContentBlockDelta,
        _context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        Ok(None) // Default: don't handle this delta
    }

    /// Accumulate a delta in extension-specific storage
    /// Called during streaming for deltas that this extension handles
    /// Extensions maintain their own accumulation state
    async fn accumulate_delta(
        &self,
        _delta: &ContentBlockDelta,
        _context: &StreamContext,
    ) -> Result<(), AppError> {
        Ok(()) // Default: no accumulation
    }

    /// Get accumulated content from extension
    /// Called during finalize to retrieve accumulated content blocks
    /// Returns Vec of (index, MessageContentData) tuples
    async fn get_accumulated_content(
        &self,
        _context: &StreamContext,
    ) -> Result<Vec<(usize, MessageContentData)>, AppError> {
        Ok(Vec::new()) // Default: no content
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

    // ========== CONTENT CONVERSION (for Extension variant) ==========

    /// Convert Extension variant to ContentBlock
    /// Called when Extension content needs to be converted for LLM
    /// Return None if this extension doesn't handle the extension_name
    fn convert_extension_content(
        &self,
        _extension_name: &str,
        _content: &serde_json::Value,
    ) -> Option<ContentBlock> {
        None // Default: doesn't handle extension content
    }

    /// Convert ContentBlock to Extension variant
    /// Called when ContentBlock from LLM needs to be stored as extension content
    /// Return None if this extension doesn't handle this ContentBlock type
    fn convert_from_content_block(&self, _block: &ContentBlock) -> Option<MessageContentData> {
        None // Default: doesn't handle conversion
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
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(), AppError> {
        for ext in &self.extensions {
            ext.before_llm_call(context, request, send_request, tx).await?;
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

    /// Process delta through extensions
    /// Returns first successful conversion, or None if no extension handles this delta
    pub async fn process_delta(
        &self,
        delta: &ai_providers::ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        for ext in &self.extensions {
            if let Some(converted) = ext.process_delta(delta, context).await? {
                return Ok(Some(converted));
            }
        }
        Ok(None)
    }

    /// Accumulate delta across all extensions
    pub async fn accumulate_delta(
        &self,
        delta: &ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<(), AppError> {
        for ext in &self.extensions {
            ext.accumulate_delta(delta, context).await?;
        }
        Ok(())
    }

    /// Get accumulated content from all extensions
    /// Returns combined content from all extensions
    pub async fn get_accumulated_content(
        &self,
        context: &StreamContext,
    ) -> Result<Vec<(usize, MessageContentData)>, AppError> {
        let mut all_content = Vec::new();
        for ext in &self.extensions {
            let ext_content = ext.get_accumulated_content(context).await?;
            all_content.extend(ext_content);
        }
        Ok(all_content)
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

    // ========== EXTENSION CONTENT CONVERSION ==========

    /// Convert Extension variant to ContentBlock
    /// Delegates to the appropriate extension based on extension_name
    pub fn convert_to_content_block(
        &self,
        extension_name: &str,
        content: &serde_json::Value,
    ) -> Option<ContentBlock> {
        for ext in &self.extensions {
            if let Some(block) = ext.convert_extension_content(extension_name, content) {
                return Some(block);
            }
        }
        None
    }

    /// Convert ContentBlock to MessageContentData (potentially Extension variant)
    /// Tries each extension until one successfully converts the block
    pub fn convert_from_content_block(&self, block: &ContentBlock) -> Option<MessageContentData> {
        for ext in &self.extensions {
            if let Some(content) = ext.convert_from_content_block(block) {
                return Some(content);
            }
        }
        None
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

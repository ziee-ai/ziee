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
    /// Extension provides content to append to SAME assistant message
    Continue {
        /// Content blocks to append to assistant message (tool results, etc.)
        /// These are appended to the existing assistant message, NOT sent as new user message
        assistant_message_content: Vec<MessageContentData>,
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

    // ========== MESSAGE CREATION CONTROL ==========

    /// Check if a user message should be created for this request
    /// Extensions can return false to prevent user message creation
    /// Example: MCP extension returns false when resuming with tool approvals
    /// Default: true (always create user message)
    fn should_create_user_message(&self, _request: &SendMessageRequest) -> bool {
        true
    }

    /// Provide an existing assistant message for continuation/resumption
    /// Extensions can return Some(message_id) to reuse an existing assistant message
    /// Example: MCP extension returns last assistant message when resuming with tool approvals
    /// Default: None (create new assistant message)
    async fn provide_assistant_message(
        &self,
        _request: &SendMessageRequest,
        _branch_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        Ok(None)
    }

    /// Provide additional content blocks for user message creation
    /// Called BEFORE user message is created, allowing extensions to contribute content
    ///
    /// # Arguments
    /// * `context` - Stream context (conversation_id, branch_id, user_id, pool)
    /// * `send_request` - Original send message request with extension fields
    /// * `text_content` - The primary text content from user
    ///
    /// # Returns
    /// Vector of MessageContentData to be included in user message
    /// - Content will be created with sequence_order starting at 1 (text is at 0)
    /// - Return empty vec if no additional content
    ///
    /// # Example
    /// ```rust
    /// // File extension adds file_attachment content blocks
    /// async fn provide_user_message_content(
    ///     &self,
    ///     context: &StreamContext,
    ///     send_request: &SendMessageRequest,
    ///     _text_content: &str,
    /// ) -> Result<Vec<MessageContentData>, AppError> {
    ///     if let Some(file_ids) = &send_request.file_ids {
    ///         return self.create_file_content_blocks(file_ids, context.user_id).await;
    ///     }
    ///     Ok(Vec::new())
    /// }
    /// ```
    async fn provide_user_message_content(
        &self,
        _context: &StreamContext,
        _send_request: &SendMessageRequest,
        _text_content: &str,
    ) -> Result<Vec<MessageContentData>, AppError> {
        Ok(Vec::new()) // Default: no additional content
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

    // ========== CONTENT CONVERSION ==========

    /// Convert Extension content to ContentBlock for LLM
    /// Return None if this extension doesn't handle this content type
    fn convert_extension_content(&self, _content: &serde_json::Value) -> Option<ContentBlock> {
        None // Default: doesn't handle extension content
    }

    /// Convert ContentBlock from LLM to MessageContentData (Extension variant)
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

    // ========== MESSAGE CREATION CONTROL ==========

    /// Check if user message should be created by consulting all extensions
    /// Returns false if ANY extension says no
    /// Example: MCP extension returns false when resuming with tool approvals
    pub fn should_create_user_message(&self, request: &SendMessageRequest) -> bool {
        for ext in &self.extensions {
            if !ext.should_create_user_message(request) {
                return false;
            }
        }
        true
    }

    /// Get assistant message from extensions for continuation/resumption
    /// Returns first Some(message_id) from any extension
    /// Example: MCP extension provides last assistant message when resuming with tool approvals
    pub async fn provide_assistant_message(
        &self,
        request: &SendMessageRequest,
        branch_id: Uuid,
    ) -> Result<Option<Uuid>, AppError> {
        for ext in &self.extensions {
            if let Some(message_id) = ext.provide_assistant_message(request, branch_id).await? {
                return Ok(Some(message_id));
            }
        }
        Ok(None)
    }

    /// Collect user message content from all extensions
    /// Calls provide_user_message_content on all extensions and combines results
    /// Returns combined vector of MessageContentData from all extensions
    /// Example: File extension adds file_attachment content blocks
    pub async fn collect_user_message_content(
        &self,
        context: &StreamContext,
        send_request: &SendMessageRequest,
        text_content: &str,
    ) -> Result<Vec<MessageContentData>, AppError> {
        let mut all_content = Vec::new();

        for ext in &self.extensions {
            let ext_content = ext
                .provide_user_message_content(context, send_request, text_content)
                .await?;
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
        if let Some(handler) = self.get_handler_for_content_type(&content_type) {
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
        if let Some(handler) = self.get_handler_for_content_type(&content_type) {
            handler.process_content_from_db(content, context).await
        } else {
            Ok(())
        }
    }

    // ========== EXTENSION CONTENT CONVERSION ==========

    /// Convert Extension content to ContentBlock for LLM
    /// Tries each extension until one successfully converts
    pub fn convert_extension_to_content_block(
        &self,
        content: &serde_json::Value,
    ) -> Option<ContentBlock> {
        for ext in &self.extensions {
            if let Some(block) = ext.convert_extension_content(content) {
                return Some(block);
            }
        }
        None
    }

    /// Convert ContentBlock to MessageContentData (Extension variant)
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

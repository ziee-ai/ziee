// Chat extension infrastructure

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
use ziee_framework::entity_extension::ExtensionRegistry as GenericExtensionRegistry;

/// Extension registration entry for distributed collection.
///
/// Now an alias over the generic `ziee_framework` primitive (gap G8): the
/// `{name, order, factory}` shape + `#[distributed_slice]` mechanics are
/// domain-agnostic and shared with the project registry (and CytoAnalyst's
/// `study`). Registration sites still construct `ExtensionEntry { name, order,
/// factory }` via this alias — unchanged.
pub type ExtensionEntry =
    ziee_framework::ExtensionEntry<dyn ChatExtension, Arc<crate::core::config::Config>>;

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

    /// Stop streaming and emit the provided text directly to the user, bypassing the LLM.
    /// Used when a tool result is already a final user-facing answer
    /// (signaled by MCP-spec `annotations.audience: ["user"]` on a content block).
    /// The text is streamed as a text delta and appended to the assistant message in the DB.
    CompleteWithContent {
        text: String,
    },
}

/// Action returned by extensions BEFORE LLM call
/// Allows extensions to skip the LLM call entirely (e.g., when tool is denied)
#[derive(Debug, Clone, Default)]
pub enum BeforeLlmAction {
    /// Continue with LLM call (default behavior)
    #[default]
    Continue,
    /// Skip LLM call, complete the turn gracefully
    Complete,
    /// Skip LLM call and emit the provided text directly to the user.
    /// Used when an approved tool returns content with MCP-spec
    /// `annotations.audience: ["user"]` — the text is streamed as a text
    /// delta and appended to the assistant message in the DB.
    CompleteWithContent {
        text: String,
    },
}

/// Context passed to extension hooks during streaming
#[derive(Clone)]
pub struct StreamContext {
    pub conversation_id: Uuid,
    pub branch_id: Uuid,
    pub message_id: Option<Uuid>,
    pub user_id: Uuid,
    // Provided to extensions via StreamContext; not read in-crate today.
    #[allow(dead_code)]
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
    ///
    /// Returns BeforeLlmAction to control whether LLM should be called:
    /// - Continue: Proceed with LLM call (default)
    /// - Complete: Skip LLM call and complete the turn gracefully
    async fn before_llm_call(
        &self,
        _context: &mut StreamContext,
        _request: &mut ChatRequest,
        _send_request: &SendMessageRequest,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        Ok(BeforeLlmAction::Continue)
    }

    /// Fires AFTER the user message is committed to the database and
    /// BEFORE `before_llm_call`. Extensions that need to persist
    /// per-message state into their own tables (e.g. mcp's per-message
    /// server-list snapshot used to restore the original selection on
    /// edit) write the rows here.
    ///
    /// Runs OUTSIDE the message-INSERT transaction. A failure here
    /// leaves the message saved without the extension's bookkeeping —
    /// acceptable for audit-trail / restore-context use cases that
    /// degrade gracefully to "use current state" when no record
    /// exists (which is the same as messages from before the
    /// extension started tracking).
    async fn after_user_message_created(
        &self,
        _context: &StreamContext,
        _user_message: &Message,
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

    /// Called when a turn ends WITHOUT the provider ever being called — i.e. an
    /// extension returned [`BeforeLlmAction::Complete`] or
    /// [`BeforeLlmAction::CompleteWithContent`] from `before_llm_call`.
    ///
    /// `after_llm_call` cannot cover those paths: its ONLY call site is
    /// `DeltaAccumulator::finalize`, and the accumulator is never constructed
    /// when the provider is skipped, so the streaming loop `break`s straight out.
    /// An extension whose work must observe every COMPLETED turn (rather than
    /// every LLM round-trip) therefore implements this in addition to
    /// `after_llm_call`. That gap is what left a `manual_approve` conversation
    /// permanently untitled: the approved tool's `audience:["user"]` result IS
    /// the answer, so the turn completes without an LLM call and the title
    /// extension never ran.
    ///
    /// Deliberately NOT called on normal turn ends — `after_llm_call` already
    /// covers those, and firing both would double-invoke every implementor.
    ///
    /// Returns no action: the turn is already ending and its content is already
    /// persisted, so there is no control flow left to influence.
    async fn after_llm_skipped(
        &self,
        _context: &StreamContext,
        _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<(), AppError> {
        Ok(())
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
    // Extension-lifecycle hook: overridable by extensions and driven by
    // `ExtensionRegistry::initialize_all`, which the chat module runs once at
    // startup (see chat/mod.rs::init).
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

    /// Should this content block be DROPPED entirely from assistant-message
    /// forwarding to the LLM? Used for extension-contributed variants that
    /// represent UI-only artifacts (e.g., file extension's `FileAttachment`
    /// blocks produced from MCP tool results — the LLM already saw them
    /// described in the ToolResult content and embedding them inline as
    /// images would confuse it).
    ///
    /// Default `Ok(false)` — most extensions don't need to skip. Override
    /// for the rare cases where chat would otherwise have to know an
    /// extension's variant name to filter it.
    async fn should_skip_in_assistant_forwarding(
        &self,
        _content: &MessageContentData,
        _context: &StreamContext,
    ) -> Result<bool, AppError> {
        Ok(false)
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
    // Extension content-conversion hook; driven by the registry aggregator of
    // the same name, which the stream persist path (`DeltaAccumulator::finalize`)
    // calls to turn accumulated provider blocks back into MessageContentData.
    fn convert_from_content_block(&self, _block: &ContentBlock) -> Option<MessageContentData> {
        None // Default: doesn't handle conversion
    }
}

/// Registry for managing chat extensions.
///
/// A thin newtype over the generic `ziee_framework` registry primitive (gap
/// G8): the storage + `register` + `iter` + route-fold mechanics are shared with
/// the project registry; only chat's domain fan-out methods (streaming deltas,
/// message-creation control, content conversion) live here. Chat has no
/// in-transaction lifecycle hooks (it streams), so it uses `iter()` +
/// `fold_routes` but not the `fire_in_tx` combinator.
pub struct ExtensionRegistry {
    inner: GenericExtensionRegistry<dyn ChatExtension>,
}

impl ExtensionRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            inner: GenericExtensionRegistry::new(),
        }
    }

    /// Register an extension
    pub fn register(&mut self, extension: Arc<dyn ChatExtension>) {
        tracing::info!("Registering chat extension: {}", extension.name());
        self.inner.register(extension);
    }

    /// Initialize all registered extensions. Run once at startup by
    /// `chat/mod.rs::init`, driving each extension's `initialize` hook.
    pub async fn initialize_all(&self, pool: &PgPool) -> Result<(), AppError> {
        for ext in self.inner.iter() {
            ext.initialize(pool).await?;
        }
        Ok(())
    }

    /// Call before_llm_call on all extensions
    /// Returns first Complete action encountered, or Continue if all extensions return Continue
    pub async fn call_before_llm_call(
        &self,
        context: &mut StreamContext,
        request: &mut ChatRequest,
        send_request: &SendMessageRequest,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<BeforeLlmAction, AppError> {
        for ext in self.inner.iter() {
            let action = ext.before_llm_call(context, request, send_request, tx).await?;

            // If any extension returns Complete or CompleteWithContent, stop iterating and return it
            if matches!(action, BeforeLlmAction::Complete | BeforeLlmAction::CompleteWithContent { .. }) {
                tracing::info!("Extension {} requested to skip LLM call", ext.name());
                return Ok(action);
            }
        }
        Ok(BeforeLlmAction::Continue)
    }

    /// Call after_llm_call on all extensions
    ///
    /// Returns the first `Continue`/`CompleteWithContent` action encountered (it
    /// decides the turn's control flow), or `Complete` if every extension
    /// returned `Complete`.
    ///
    /// The control-flow decision is CAPTURED, not short-circuited: later
    /// extensions still run for their side effects. Returning early instead
    /// silently disabled every extension ordered after the deciding one — which
    /// is how a conversation using an `audience:["user"]` tool ended up
    /// permanently untitled. The MCP extension (order 30) returns
    /// `CompleteWithContent` for such a tool, so the title extension (order 80)
    /// was never reached on ANY turn of that conversation.
    ///
    /// Only the FIRST such action is honored; a later extension cannot override
    /// the turn's control flow, so this cannot change how a turn terminates.
    pub async fn call_after_llm_call(
        &self,
        context: &StreamContext,
        final_message: &Message,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) -> Result<ExtensionAction, AppError> {
        let mut decided: Option<ExtensionAction> = None;

        for ext in self.inner.iter() {
            let action = ext.after_llm_call(context, final_message, tx).await?;

            if decided.is_none()
                && matches!(
                    action,
                    ExtensionAction::Continue { .. } | ExtensionAction::CompleteWithContent { .. }
                )
            {
                decided = Some(action);
            }
        }

        Ok(decided.unwrap_or(ExtensionAction::Complete))
    }

    /// Call `after_llm_skipped` on all extensions, in registration (`order`)
    /// sequence.
    ///
    /// Runs on the turn-ending paths where the provider was never called, so
    /// `finalize` — and therefore [`Self::call_after_llm_call`] — never ran.
    ///
    /// Unlike `call_after_llm_call`, a failing extension does NOT abort the
    /// fan-out and does NOT surface an error to the caller: it is logged and the
    /// remaining extensions still run. By the time this fires the user's answer
    /// is already persisted AND streamed, so failing the turn over a
    /// post-processing error (a title provider being down, say) would break a
    /// turn that otherwise succeeded. `call_after_llm_call` can afford `?`
    /// because it runs inside `finalize`, whose caller already degrades to
    /// `ExtensionAction::Complete` on error.
    pub async fn call_after_llm_skipped(
        &self,
        context: &StreamContext,
        tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
    ) {
        for ext in self.inner.iter() {
            if let Err(e) = ext.after_llm_skipped(context, tx).await {
                tracing::error!(
                    "Extension {} failed in after_llm_skipped (turn already \
                     completed; continuing): {}",
                    ext.name(),
                    e
                );
            }
        }
    }

    /// Process delta through extensions
    /// Returns first successful conversion, or None if no extension handles this delta
    pub async fn process_delta(
        &self,
        delta: &ai_providers::ContentBlockDelta,
        context: &StreamContext,
    ) -> Result<Option<ContentBlockDelta>, AppError> {
        for ext in self.inner.iter() {
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
        for ext in self.inner.iter() {
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
        for ext in self.inner.iter() {
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
        for ext in self.inner.iter() {
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
        for ext in self.inner.iter() {
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

        for ext in self.inner.iter() {
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
        self.inner
            .fold_routes(router, |router, ext| ext.register_routes(router))
    }

    // ========== CONTENT TYPE HANDLING ==========

    /// Find extension that handles given content type
    /// Returns first extension that declares it handles this content type
    pub fn get_handler_for_content_type(
        &self,
        content_type: &str,
    ) -> Option<&Arc<dyn ChatExtension>> {
        self.inner
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

    /// Should this content block be DROPPED from assistant-message forwarding?
    /// Asks any extension that handles this content's type. Default false.
    /// Used by the streaming pipeline to skip variants that extensions mark
    /// as UI-only (e.g. `FileAttachment` blocks from MCP tool results).
    pub async fn should_skip_in_assistant_forwarding(
        &self,
        content: &MessageContentData,
        context: &StreamContext,
    ) -> Result<bool, AppError> {
        let content_type = content.content_type();
        if let Some(handler) = self.get_handler_for_content_type(&content_type) {
            handler
                .should_skip_in_assistant_forwarding(content, context)
                .await
        } else {
            Ok(false)
        }
    }

    /// Fan-out the `after_user_message_created` hook to every extension.
    /// Runs each in sequence (not parallel) so an early failure can be
    /// observed deterministically; a single extension's failure
    /// propagates up and aborts subsequent invocations. Used by the
    /// streaming pipeline to give extensions a chance to write per-
    /// message state into their own tables right after the user
    /// message commits.
    pub async fn after_user_message_created(
        &self,
        context: &StreamContext,
        user_message: &Message,
        send_request: &SendMessageRequest,
    ) -> Result<(), AppError> {
        for handler in self.inner.iter() {
            handler
                .after_user_message_created(context, user_message, send_request)
                .await?;
        }
        Ok(())
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
        for ext in self.inner.iter() {
            if let Some(block) = ext.convert_extension_content(content) {
                return Some(block);
            }
        }
        None
    }

    /// Convert ContentBlock to MessageContentData (Extension variant)
    /// Tries each extension until one successfully converts the block.
    /// Called by `DeltaAccumulator::finalize` (stream persist path) to convert
    /// accumulated provider blocks back into persistable MessageContentData.
    pub fn convert_from_content_block(&self, block: &ContentBlock) -> Option<MessageContentData> {
        for ext in self.inner.iter() {
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

#[cfg(test)]
mod after_llm_call_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Records whether it ran, and returns a scripted action.
    struct ProbeExtension {
        name: &'static str,
        action: fn() -> ExtensionAction,
        ran: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ChatExtension for ProbeExtension {
        fn name(&self) -> &str {
            self.name
        }

        async fn after_llm_call(
            &self,
            _context: &StreamContext,
            _final_message: &Message,
            _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
        ) -> Result<ExtensionAction, AppError> {
            self.ran.store(true, Ordering::SeqCst);
            Ok((self.action)())
        }
    }

    fn probe(
        name: &'static str,
        action: fn() -> ExtensionAction,
    ) -> (Arc<dyn ChatExtension>, Arc<AtomicBool>) {
        let ran = Arc::new(AtomicBool::new(false));
        (
            Arc::new(ProbeExtension {
                name,
                action,
                ran: ran.clone(),
            }),
            ran,
        )
    }

    fn context(pool: PgPool) -> StreamContext {
        StreamContext {
            conversation_id: uuid::Uuid::new_v4(),
            branch_id: uuid::Uuid::new_v4(),
            message_id: None,
            user_id: uuid::Uuid::new_v4(),
            pool,
            metadata: std::collections::HashMap::new(),
            iteration: 1,
        }
    }

    fn message() -> Message {
        Message {
            id: uuid::Uuid::new_v4(),
            role: "assistant".to_string(),
            originated_from_id: uuid::Uuid::new_v4(),
            edit_count: 0,
            model_id: None,
            created_at: chrono::Utc::now(),
        }
    }

    /// A deciding extension must NOT silently disable the ones after it.
    ///
    /// This is the regression guard for conversations using an
    /// `audience:["user"]` tool: the MCP extension (order 30) returns
    /// `CompleteWithContent`, and the title extension (order 80) was never
    /// reached, so such a conversation stayed permanently untitled.
    #[sqlx::test]
    async fn later_extensions_still_run_after_a_deciding_action(pool: PgPool) {
        for decider in [
            (|| ExtensionAction::CompleteWithContent {
                text: "final".to_string(),
            }) as fn() -> ExtensionAction,
            (|| ExtensionAction::Continue {
                assistant_message_content: Vec::new(),
            }) as fn() -> ExtensionAction,
        ] {
            let mut registry = ExtensionRegistry::new();
            let (first, first_ran) = probe("first", || ExtensionAction::Complete);
            let (deciding, deciding_ran) = probe("deciding", decider);
            let (later, later_ran) = probe("later", || ExtensionAction::Complete);
            registry.register(first);
            registry.register(deciding);
            registry.register(later);

            let action = registry
                .call_after_llm_call(&context(pool.clone()), &message(), None)
                .await
                .expect("hook must not error");

            assert!(first_ran.load(Ordering::SeqCst));
            assert!(deciding_ran.load(Ordering::SeqCst));
            assert!(
                later_ran.load(Ordering::SeqCst),
                "an extension ordered AFTER the deciding one must still run"
            );
            // …and the turn's control flow is still the deciding extension's.
            assert!(matches!(
                action,
                ExtensionAction::CompleteWithContent { .. } | ExtensionAction::Continue { .. }
            ));
        }
    }

    /// A later extension cannot hijack control flow from the first decider.
    #[sqlx::test]
    async fn the_first_deciding_action_wins(pool: PgPool) {
        let mut registry = ExtensionRegistry::new();
        let (first, _) = probe("first", || ExtensionAction::Continue {
            assistant_message_content: Vec::new(),
        });
        let (second, second_ran) = probe("second", || ExtensionAction::CompleteWithContent {
            text: "hijack".to_string(),
        });
        registry.register(first);
        registry.register(second);

        let action = registry
            .call_after_llm_call(&context(pool), &message(), None)
            .await
            .expect("hook must not error");

        assert!(second_ran.load(Ordering::SeqCst), "it still runs");
        assert!(
            matches!(action, ExtensionAction::Continue { .. }),
            "but the FIRST decider owns the turn's control flow"
        );
    }

    /// All-Complete still yields Complete.
    #[sqlx::test]
    async fn all_complete_yields_complete(pool: PgPool) {
        let mut registry = ExtensionRegistry::new();
        let (a, _) = probe("a", || ExtensionAction::Complete);
        let (b, _) = probe("b", || ExtensionAction::Complete);
        registry.register(a);
        registry.register(b);

        let action = registry
            .call_after_llm_call(&context(pool), &message(), None)
            .await
            .expect("hook must not error");
        assert!(matches!(action, ExtensionAction::Complete));
    }

    // ── after_llm_skipped: the turn-end pass for LLM-skipped turns ──────────

    /// Records that it ran, in order, and optionally fails.
    struct SkipProbe {
        name: &'static str,
        fails: bool,
        order_log: Arc<std::sync::Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl ChatExtension for SkipProbe {
        fn name(&self) -> &str {
            self.name
        }

        async fn after_llm_skipped(
            &self,
            _context: &StreamContext,
            _tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>>,
        ) -> Result<(), AppError> {
            self.order_log.lock().unwrap().push(self.name);
            if self.fails {
                return Err(AppError::internal_error("probe failure"));
            }
            Ok(())
        }
    }

    /// An extension that implements NOTHING — proves the trait's default
    /// `after_llm_skipped` is a harmless no-op, which is what keeps the other 18
    /// extensions unaffected by the new hook.
    struct InertProbe;

    #[async_trait]
    impl ChatExtension for InertProbe {
        fn name(&self) -> &str {
            "inert"
        }
    }

    /// TEST-2: every extension runs, in registration (order) sequence.
    ///
    /// Unlike `call_before_llm_call`, this must NOT short-circuit — the whole
    /// point is to reach the extensions ordered after MCP.
    #[sqlx::test]
    async fn skipped_hook_runs_every_extension_in_order(pool: PgPool) {
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = ExtensionRegistry::new();
        for name in ["first", "second", "third"] {
            registry.register(Arc::new(SkipProbe {
                name,
                fails: false,
                order_log: log.clone(),
            }));
        }

        registry
            .call_after_llm_skipped(&context(pool), None)
            .await;

        assert_eq!(
            *log.lock().unwrap(),
            vec!["first", "second", "third"],
            "all extensions must run, in registration order"
        );
    }

    /// TEST-3: one extension failing must not stop the others, and must not
    /// surface an error.
    ///
    /// By the time this hook fires the user's answer is already persisted AND
    /// streamed, so failing the turn over a post-processing error (a title
    /// provider being down) would break a turn that otherwise succeeded.
    /// `call_after_llm_call` can afford `?` because its caller degrades to
    /// `Complete`; this one has no such caller.
    #[sqlx::test]
    async fn skipped_hook_swallows_an_extension_error_and_keeps_going(pool: PgPool) {
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = ExtensionRegistry::new();
        registry.register(Arc::new(SkipProbe {
            name: "before",
            fails: false,
            order_log: log.clone(),
        }));
        registry.register(Arc::new(SkipProbe {
            name: "boom",
            fails: true,
            order_log: log.clone(),
        }));
        registry.register(Arc::new(SkipProbe {
            name: "after",
            fails: false,
            order_log: log.clone(),
        }));

        // Returns unit: there is deliberately no error for the caller to handle.
        registry
            .call_after_llm_skipped(&context(pool), None)
            .await;

        assert_eq!(
            *log.lock().unwrap(),
            vec!["before", "boom", "after"],
            "an extension ordered AFTER a failing one must still run"
        );
    }

    /// TEST-4: the default impl is a no-op, so extensions that do not opt in are
    /// unaffected.
    #[sqlx::test]
    async fn skipped_hook_default_impl_is_a_noop(pool: PgPool) {
        let log = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut registry = ExtensionRegistry::new();
        registry.register(Arc::new(InertProbe));
        registry.register(Arc::new(SkipProbe {
            name: "implementor",
            fails: false,
            order_log: log.clone(),
        }));

        registry
            .call_after_llm_skipped(&context(pool), None)
            .await;

        assert_eq!(
            *log.lock().unwrap(),
            vec!["implementor"],
            "the inert extension contributes nothing and does not error"
        );
    }
}

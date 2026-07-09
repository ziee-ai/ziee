//! js_tool chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `js_tool.rs` is the `ChatExtension` impl: its `before_llm_call` sets the
//! `attach_run_js_mcp` metadata flag (read by `auto_attach_builtin_ids` in
//! `mcp/chat_extension/mcp.rs`) when the model is tool-capable and the feature
//! is enabled by config.

pub mod extension;
mod js_tool;

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer (the documented
/// silent-failure point).
pub const ATTACH_FLAG: &str = "attach_run_js_mcp";

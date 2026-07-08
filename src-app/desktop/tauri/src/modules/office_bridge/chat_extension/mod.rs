//! office_bridge chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `office_bridge.rs` is the `ChatExtension` impl: its `before_llm_call` sets
//! the `attach_office_bridge_mcp` metadata flag (read by
//! `auto_attach_builtin_ids` in `mcp/chat_extension/mcp.rs`) when the
//! office_bridge built-in server is enabled/available and the model is
//! tool-capable.

pub mod extension;
mod office_bridge;

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer (the documented
/// silent-failure point).
pub const ATTACH_FLAG: &str = "attach_office_bridge_mcp";

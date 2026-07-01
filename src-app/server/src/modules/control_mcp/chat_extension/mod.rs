//! control_mcp chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `control.rs` is the `ChatExtension` impl: its `before_llm_call` sets the
//! `attach_control_mcp` metadata flag (read by `auto_attach_builtin_ids` in
//! `mcp/chat_extension/mcp.rs`) when the deploy kill-switch is on and the model
//! is tool-capable.

pub mod control;
pub mod extension;

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer (the documented
/// silent-failure point).
pub const ATTACH_FLAG: &str = "attach_control_mcp";

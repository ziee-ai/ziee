//! lit_search chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `lit_search.rs` is the `ChatExtension` impl: its `before_llm_call` sets the
//! `attach_lit_search_mcp` metadata flag (read by `auto_attach_builtin_ids` in
//! `mcp/chat_extension/mcp.rs`) when literature search is enabled and the model
//! is tool-capable.

pub mod extension;
mod lit_search;

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer.
pub const ATTACH_FLAG: &str = "attach_lit_search_mcp";

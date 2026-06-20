//! citations chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `citations.rs` is the `ChatExtension` impl: its `before_llm_call` sets the
//! `attach_citations_mcp` metadata flag (read by `auto_attach_builtin_ids` in
//! `mcp/chat_extension/mcp.rs`) for tool-capable models. Citations is per-user
//! and always available (no admin enable / provider config gate), so the only
//! gate is tool-capability.

mod citations;
pub mod extension;

/// Metadata flag set by this extension's `before_llm_call` and read by
/// `mcp::chat_extension::auto_attach_builtin_ids`. Shared as one const so a typo
/// can't silently desync the producer from the consumer (the documented
/// silent-failure point in CLAUDE.md).
pub const ATTACH_FLAG: &str = "attach_citations_mcp";

//! knowledge_base chat-extension bridge.
//!
//! `extension.rs` registers via `linkme::distributed_slice(CHAT_EXTENSIONS)`.
//! `knowledge_base.rs` is the `ChatExtension` impl: its `before_llm_call` sets
//! the `attach_knowledge_base_mcp` flag (read by `auto_attach_builtin_ids` in
//! `mcp/chat_extension/mcp.rs`) for tool-capable models WHEN ≥1 KB is attached
//! to the conversation, and injects a one-line note listing the attached KBs +
//! the grounded-answer nudge.

mod knowledge_base;
pub mod extension;

/// Shared flag const (producer here, consumer in `mcp::chat_extension`).
pub const ATTACH_FLAG: &str = "attach_knowledge_base_mcp";

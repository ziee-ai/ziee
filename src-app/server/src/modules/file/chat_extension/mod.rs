//! Chat-extension bridge for the file module.
//!
//! Self-registers into chat's `CHAT_EXTENSIONS` linkme slice (see
//! `extension.rs`). The chat module never imports this folder —
//! discovery is purely link-time via the linkme attribute.
//!
//! Moved here from `chat/extensions/file/` as part of the file/project/mcp
//! bridge extraction (chat knows nothing about files; the file module
//! owns its bridge directly).

pub mod extension;
pub mod file;
pub mod types;

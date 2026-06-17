//! Chat-extension bridge for the bio_mcp module.
//!
//! Self-registers into chat's `CHAT_EXTENSIONS` linkme slice (see
//! `extension.rs`). On tool-capable models where the admin has enabled the
//! bio row, it flags `attach_bio_mcp` (consumed by `mcp::chat_extension`'s
//! `auto_attach_builtin_ids`) and injects a one-line untrusted-content
//! guard. Mirrors the file/memory bridges.

pub mod bio;
pub mod extension;

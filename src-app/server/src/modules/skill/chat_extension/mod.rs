//! Skill chat-extension bridge — Path B progressive disclosure.
//!
//! The extension does NOT load SKILL.md bodies or references. It only
//! injects a short system-message listing so the model knows which
//! skills are available; the model decides when to call the built-in
//! `skill_mcp` server (`load_skill` / `read_skill_file`) to read the
//! body on demand. Token cost: ~50–100 tokens per skill (the Agent
//! Skills 1536-char cap on `description + when_to_use` keeps the
//! listing small even with 20+ installed skills).

pub mod extension;

/// Metadata flag the extension sets when ≥1 skill is available to a
/// tool-capable conversation; read by `auto_attach_builtin_ids` in
/// `mcp/chat_extension/mcp.rs` to attach the built-in `skill_mcp` server so the
/// model can actually call the `load_skill` / `read_skill_file` tools the
/// injected listing tells it to use.
pub const ATTACH_FLAG: &str = "attach_skill_mcp";

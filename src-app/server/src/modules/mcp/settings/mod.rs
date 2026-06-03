// Unified MCP-settings module.
//
// One Rust type (`McpSettings`) + one scope discriminator (`McpScope`)
// + one repository (`McpSettingsRepository`) cover both conversation-scoped
// and project-scoped MCP configuration. Backed by the `mcp_settings`
// table (migration 78) which holds both scopes via nullable FKs +
// CHECK constraint.
//
// Used by mcp/chat_extension/ for per-conversation settings AND by
// mcp/project_extension/ for per-project defaults. Snapshot (project
// settings → new conversation row on attach) is a single
// `INSERT … SELECT` on this table.

pub mod models;
pub mod repository;

pub use models::{McpScope, McpSettings};
pub use repository::McpSettingsRepository;

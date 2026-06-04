// MCP module's contribution to the project-extension system.
//
// This subfolder owns the project↔mcp relationship:
//   - Routes mounted at `/api/projects/{id}/mcp-settings` (GET + PUT).
//   - The validation that user-supplied server_ids are accessible.
//   - Lifecycle hooks: `on_conversation_attached` snapshots the project's
//     mcp_settings row into a conversation-scoped row;
//     `on_conversation_detached` deletes it; `on_project_duplicated`
//     clones the project's settings to the duplicate.
//
// Acid-test invariant: the project module imports NOTHING from here.
// Discovery is via the `PROJECT_EXTENSIONS` distributed slice (see
// `extension.rs`). Deleting this folder leaves project compiling — the
// /mcp-settings routes disappear (correctly) and the lifecycle hooks
// fan-out to one fewer extension.

pub mod extension;
pub mod handlers;
pub mod models;
pub mod routes;

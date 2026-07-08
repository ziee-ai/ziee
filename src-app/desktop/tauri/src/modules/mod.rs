//! Desktop Modules
//!
//! Feature modules:
//! - auth: Desktop authentication and user management
//! - backend: Backend server lifecycle and status routes
//! - llm_provider: LLM provider event handlers
//! - magic_link: One-time login tokens for tunnel users
//! - mcp: MCP server event handlers (auto-assign-to-all-groups)
//! - remote_access: ngrok tunnel control + lifecycle
//! - settings: Desktop-specific settings management
//! - tray: System tray integration
//! - tunnel_auth: /auth/config + /auth/login-password-only
//! - updater: Auto-update via HTTP routes

pub mod auth;
pub mod backend;
pub mod host_mount;
pub mod office_bridge;
pub mod llm_provider;
pub mod magic_link;
pub mod mcp;
pub mod remote_access;
pub mod settings;
pub mod tray;
pub mod tunnel_auth;
pub mod updater;

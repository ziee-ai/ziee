//! Admin-controlled policy for what regular users may install as MCP
//! servers (allowed transports + the sandbox flavor force-applied to
//! user-installed stdio servers).
//!
//! The policy is a singleton (`mcp_user_policy` row, id=1) seeded by
//! migration 84 to a permissive default (`['http','stdio']` with the
//! `full` sandbox flavor). Admins edit via PUT /api/mcp/user-policy
//! (perm `McpUserPolicyEdit`); reads are gated on `McpServersRead`.
//!
//! User create/update handlers in `mcp/handlers/user.rs` enforce the
//! policy at write time:
//!   - reject 422 `MCP_TRANSPORT_NOT_ALLOWED` when the requested
//!     transport isn't in the policy's allow-list,
//!   - for stdio, force-set `run_in_sandbox = true` and
//!     `sandbox_flavor = policy.user_stdio_sandbox_flavor`,
//!     ignoring whatever the client sent.

pub mod enforce;
pub mod handlers;
pub mod repository;
pub mod types;

pub use enforce::{enforce_on_user_create, enforce_on_user_transport_change};
pub use repository::load;

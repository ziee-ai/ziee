//! Permission keys for the control_mcp module.
//!
//! `control::use` gates ACCESS to the control surface (the JSON-RPC handler).
//! It is granted to the default Users group by migration 126, so every user can
//! reach the tools. The ACTUAL per-action authorization is enforced downstream:
//! each `invoke_capability` dispatches to the real REST route carrying the
//! caller's JWT, so the target route's own `RequirePermissions` re-authorizes
//! from the DB exactly as if the user had made the call from the UI.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in control tools (`list_capabilities` / `describe_capability` /
/// `invoke_capability`). Granted to the default Users group by migration 126.
pub struct ControlUse;
impl PermissionCheck for ControlUse {
    const NAME: &'static str = "ControlUse";
    const PERMISSION: &'static str = "control::use";
    const DESCRIPTION: &'static str =
        "Use the built-in app-control tools to discover and invoke ziee's own API.";
    const MODULE: &'static str = "control";
}

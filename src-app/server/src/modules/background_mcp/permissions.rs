//! Permission key for the background_mcp module.
//!
//! `background::use` gates ACCESS to the background-run surface (the JSON-RPC
//! handler). It is granted to the default Users group by
//! `migrations/202607191000_background_grant_permissions.sql`, so every user can
//! reach the tools. Ownership is enforced downstream: every run row is
//! owner-scoped (`insert_background_run` stamps `user_id`; `check_status` /
//! `collect_result` fetch via `find_run_for_owner`, so a cross-user `run_id`
//! yields 404 and never leaks another user's run — DEC-36 / CODING_GUIDELINES §1).

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in background-run tools (`spawn_background` / `check_status` /
/// `collect_result`). Granted to the default Users group by
/// `202607191000_background_grant_permissions.sql`.
pub struct BackgroundUse;
impl PermissionCheck for BackgroundUse {
    const NAME: &'static str = "BackgroundUse";
    const PERMISSION: &'static str = "background::use";
    const DESCRIPTION: &'static str =
        "Use the built-in background-run tools to spawn, check, and collect detached sub-agent work.";
    const MODULE: &'static str = "background";
}

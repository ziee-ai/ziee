//! Permission key for the server_update module.
//!
//! Admin-only — Administrators have `server_update::read` implicitly via the
//! `*` wildcard, so no grant migration is needed (same as the code_sandbox
//! resource-limits permissions). The `<module>::<action>` prefix matches every
//! peer module (code_sandbox::, hardware::, …).

use crate::modules::permissions::types::PermissionCheck;

/// Read the cached server update-availability status.
pub struct ServerUpdateRead;

impl PermissionCheck for ServerUpdateRead {
    const NAME: &'static str = "ServerUpdateRead";
    const PERMISSION: &'static str = "server_update::read";
    const DESCRIPTION: &'static str = "View the cached server update-availability status.";
    const MODULE: &'static str = "server_update";
}

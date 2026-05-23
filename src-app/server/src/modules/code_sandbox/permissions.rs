//! Permission keys for the code_sandbox module.
//!
//! Implements `PermissionCheck` so handlers can use
//! `RequirePermissions<(CodeSandboxExecute,)>` as an axum extractor.
//! That single extractor does (validated by the existing pattern in
//! `crate::modules::file::handlers`):
//!   1. extracts the Bearer JWT from Authorization,
//!   2. verifies signature + audience + issuer against JwtService,
//!   3. loads the user from the DB,
//!   4. confirms the user has `code_sandbox::execute` in their
//!      direct permissions OR via any of their groups,
//!   5. rejects with 401/403 otherwise.
//!
//! Migration 35 grants this permission to the default Users group.

use crate::modules::permissions::types::PermissionCheck;

/// Permission required to invoke any code_sandbox tool.
pub struct CodeSandboxExecute;

impl PermissionCheck for CodeSandboxExecute {
    const NAME: &'static str = "CodeSandboxExecute";
    const PERMISSION: &'static str = "code_sandbox::execute";
    const DESCRIPTION: &'static str = "Invoke code_sandbox tools (read/write/execute in the sandbox)";
    const MODULE: &'static str = "code_sandbox";
}

/// Read access to environment metadata + prefetch task state +
/// SSE progress streams. Sufficient to render the admin UI's
/// "Sandbox Environments" page without being able to spend
/// bandwidth on a new download.
pub struct CodeSandboxEnvironmentsRead;

impl PermissionCheck for CodeSandboxEnvironmentsRead {
    const NAME: &'static str = "CodeSandboxEnvironmentsRead";
    const PERMISSION: &'static str = "code_sandbox::environments::read";
    const DESCRIPTION: &'static str =
        "List available sandbox environments and watch prefetch progress.";
    const MODULE: &'static str = "code_sandbox";
}

/// Write access — triggers a network download. Split from Read so
/// operators on metered connections can grant Read to a wider audience
/// while keeping Manage on a smaller admin group.
pub struct CodeSandboxEnvironmentsManage;

impl PermissionCheck for CodeSandboxEnvironmentsManage {
    const NAME: &'static str = "CodeSandboxEnvironmentsManage";
    const PERMISSION: &'static str = "code_sandbox::environments::manage";
    const DESCRIPTION: &'static str =
        "Trigger pre-fetch + cache management of sandbox rootfs environments.";
    const MODULE: &'static str = "code_sandbox";
}

/// Read the runtime-configured resource-limits singleton (Plan 1 §6). Admin
/// surface; not granted to regular Users (their requests already run within
/// whatever the operator configured).
pub struct CodeSandboxResourceLimitsRead;

impl PermissionCheck for CodeSandboxResourceLimitsRead {
    const NAME: &'static str = "CodeSandboxResourceLimitsRead";
    const PERMISSION: &'static str = "code_sandbox::resource_limits::read";
    const DESCRIPTION: &'static str =
        "Read the sandbox resource limits configuration.";
    const MODULE: &'static str = "code_sandbox";
}

/// Mutate the runtime-configured resource-limits singleton. Admin-only;
/// Administrators have it implicitly via the `*` wildcard (no explicit grant
/// migration needed).
pub struct CodeSandboxResourceLimitsManage;

impl PermissionCheck for CodeSandboxResourceLimitsManage {
    const NAME: &'static str = "CodeSandboxResourceLimitsManage";
    const PERMISSION: &'static str = "code_sandbox::resource_limits::manage";
    const DESCRIPTION: &'static str =
        "Update the sandbox memory/CPU/PID caps + per-exec timeout + idle-evict policy.";
    const MODULE: &'static str = "code_sandbox";
}

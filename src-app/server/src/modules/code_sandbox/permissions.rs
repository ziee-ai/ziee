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

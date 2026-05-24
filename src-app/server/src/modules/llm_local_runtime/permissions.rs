#[allow(unused_imports)]
use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// Local LLM Runtime Management Permissions
// =====================================================

/// Permission to view local runtime instances and their status
pub struct LocalRuntimeRead;
impl PermissionCheck for LocalRuntimeRead {
    const NAME: &'static str = "LocalRuntimeRead";
    const PERMISSION: &'static str = "llm_local_runtime::read";
    const DESCRIPTION: &'static str = "View local LLM runtime instances and their status";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to start/stop/restart local runtime instances
pub struct LocalRuntimeManage;
impl PermissionCheck for LocalRuntimeManage {
    const NAME: &'static str = "LocalRuntimeManage";
    const PERMISSION: &'static str = "llm_local_runtime::manage";
    const DESCRIPTION: &'static str = "Start, stop, and restart local LLM runtime instances";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to view instance logs
pub struct LocalRuntimeLogs;
impl PermissionCheck for LocalRuntimeLogs {
    const NAME: &'static str = "LocalRuntimeLogs";
    const PERMISSION: &'static str = "llm_local_runtime::logs";
    const DESCRIPTION: &'static str = "View runtime instance logs";
    const MODULE: &'static str = "llm_local_runtime";
}

// =====================================================
// Runtime Version Management Permissions
// =====================================================

/// Permission to view runtime versions.
///
/// Distinct permission string from `LocalRuntimeRead` (which gates the
/// per-instance status endpoint). The audit's 02-permissions F-10
/// flagged the collision — a single-permission grant intended for
/// version-catalogue reading would also grant access to live instance
/// telemetry. Splitting the string forces explicit grants.
pub struct RuntimeVersionRead;
impl PermissionCheck for RuntimeVersionRead {
    const NAME: &'static str = "RuntimeVersionRead";
    const PERMISSION: &'static str = "llm_local_runtime::versions_read";
    const DESCRIPTION: &'static str = "View runtime versions and check for updates";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to download/create runtime versions
pub struct RuntimeVersionCreate;
impl PermissionCheck for RuntimeVersionCreate {
    const NAME: &'static str = "RuntimeVersionCreate";
    const PERMISSION: &'static str = "llm_local_runtime::create";
    const DESCRIPTION: &'static str = "Download and register new runtime versions";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to update runtime version settings (e.g., set default)
pub struct RuntimeVersionUpdate;
impl PermissionCheck for RuntimeVersionUpdate {
    const NAME: &'static str = "RuntimeVersionUpdate";
    const PERMISSION: &'static str = "llm_local_runtime::update";
    const DESCRIPTION: &'static str = "Update runtime version settings and defaults";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to delete runtime versions
pub struct RuntimeVersionDelete;
impl PermissionCheck for RuntimeVersionDelete {
    const NAME: &'static str = "RuntimeVersionDelete";
    const PERMISSION: &'static str = "llm_local_runtime::delete";
    const DESCRIPTION: &'static str = "Delete runtime versions";
    const MODULE: &'static str = "llm_local_runtime";
}

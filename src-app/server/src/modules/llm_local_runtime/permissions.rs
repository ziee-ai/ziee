#[allow(unused_imports)]
use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// Local LLM Runtime Management Permissions
// =====================================================

/// Permission to view local runtime instances and their status
pub struct LocalRuntimeRead;
impl PermissionCheck for LocalRuntimeRead {
    const NAME: &'static str = "LocalRuntimeRead";
    const PERMISSION: &'static str = "local_runtime::read";
    const DESCRIPTION: &'static str = "View local LLM runtime instances and their status";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to start/stop/restart local runtime instances
pub struct LocalRuntimeManage;
impl PermissionCheck for LocalRuntimeManage {
    const NAME: &'static str = "LocalRuntimeManage";
    const PERMISSION: &'static str = "local_runtime::manage";
    const DESCRIPTION: &'static str = "Start, stop, and restart local LLM runtime instances";
    const MODULE: &'static str = "llm_local_runtime";
}

/// Permission to view instance logs
pub struct LocalRuntimeLogs;
impl PermissionCheck for LocalRuntimeLogs {
    const NAME: &'static str = "LocalRuntimeLogs";
    const PERMISSION: &'static str = "local_runtime::logs";
    const DESCRIPTION: &'static str = "View runtime instance logs";
    const MODULE: &'static str = "llm_local_runtime";
}

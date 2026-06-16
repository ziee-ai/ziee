//! Workflow permissions — mirrors the skill split + adds `execute`
//! (the run-the-DAG action, see plan §3).

use crate::modules::permissions::PermissionCheck;

pub struct WorkflowsRead;
impl PermissionCheck for WorkflowsRead {
    const NAME: &'static str = "WorkflowsRead";
    const PERMISSION: &'static str = "workflows::read";
    const DESCRIPTION: &'static str = "View installed workflows";
    const MODULE: &'static str = "workflow";
}

pub struct WorkflowsInstall;
impl PermissionCheck for WorkflowsInstall {
    const NAME: &'static str = "WorkflowsInstall";
    const PERMISSION: &'static str = "workflows::install";
    const DESCRIPTION: &'static str = "Install user-scope workflows";
    const MODULE: &'static str = "workflow";
}

pub struct WorkflowsManage;
impl PermissionCheck for WorkflowsManage {
    const NAME: &'static str = "WorkflowsManage";
    const PERMISSION: &'static str = "workflows::manage";
    const DESCRIPTION: &'static str = "Edit / delete own user-scope workflows";
    const MODULE: &'static str = "workflow";
}

pub struct WorkflowsManageSystem;
impl PermissionCheck for WorkflowsManageSystem {
    const NAME: &'static str = "WorkflowsManageSystem";
    const PERMISSION: &'static str = "workflows::manage_system";
    const DESCRIPTION: &'static str = "Install / edit / delete system-scope workflows (admin)";
    const MODULE: &'static str = "workflow";
}

pub struct WorkflowsAssignToGroups;
impl PermissionCheck for WorkflowsAssignToGroups {
    const NAME: &'static str = "WorkflowsAssignToGroups";
    const PERMISSION: &'static str = "workflows::assign_to_groups";
    const DESCRIPTION: &'static str = "Manage group assignments for system-scope workflows";
    const MODULE: &'static str = "workflow";
}

pub struct WorkflowsExecute;
impl PermissionCheck for WorkflowsExecute {
    const NAME: &'static str = "WorkflowsExecute";
    const PERMISSION: &'static str = "workflows::execute";
    const DESCRIPTION: &'static str = "Kick off a workflow run";
    const MODULE: &'static str = "workflow";
}

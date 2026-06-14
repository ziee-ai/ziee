//! Skill permissions — mirrors the MCP user/system split.
//!
//! - `read` / `install` — default for any authenticated user (per plan §3).
//! - `manage` — edit / delete OWN user-scope items.
//! - `manage_system` — install / edit / delete system-scope items (admin).
//! - `assign_to_groups` — manage `group_skills` rows (admin).
//!
//! Administrators auto-grant all five via the `*` wildcard.

use crate::modules::permissions::PermissionCheck;

pub struct SkillsRead;
impl PermissionCheck for SkillsRead {
    const NAME: &'static str = "SkillsRead";
    const PERMISSION: &'static str = "skills::read";
    const DESCRIPTION: &'static str = "View installed skills";
    const MODULE: &'static str = "skill";
}

pub struct SkillsInstall;
impl PermissionCheck for SkillsInstall {
    const NAME: &'static str = "SkillsInstall";
    const PERMISSION: &'static str = "skills::install";
    const DESCRIPTION: &'static str = "Install user-scope skills (from hub or local import)";
    const MODULE: &'static str = "skill";
}

pub struct SkillsManage;
impl PermissionCheck for SkillsManage {
    const NAME: &'static str = "SkillsManage";
    const PERMISSION: &'static str = "skills::manage";
    const DESCRIPTION: &'static str = "Edit / delete own user-scope skills";
    const MODULE: &'static str = "skill";
}

pub struct SkillsManageSystem;
impl PermissionCheck for SkillsManageSystem {
    const NAME: &'static str = "SkillsManageSystem";
    const PERMISSION: &'static str = "skills::manage_system";
    const DESCRIPTION: &'static str = "Install / edit / delete system-scope skills (admin)";
    const MODULE: &'static str = "skill";
}

pub struct SkillsAssignToGroups;
impl PermissionCheck for SkillsAssignToGroups {
    const NAME: &'static str = "SkillsAssignToGroups";
    const PERMISSION: &'static str = "skills::assign_to_groups";
    const DESCRIPTION: &'static str = "Manage group assignments for system-scope skills";
    const MODULE: &'static str = "skill";
}

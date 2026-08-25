//! Permission keys for the agent module.
//!
//! Admin-only surface: Administrators hold both implicitly via the `*`
//! wildcard, so there is NO grant migration (mirrors
//! `code_sandbox::resource_limits::{read,manage}`).

use crate::modules::permissions::types::PermissionCheck;

/// Read the deployment-wide agent policy singleton.
pub struct AgentSettingsRead;
impl PermissionCheck for AgentSettingsRead {
    const NAME: &'static str = "AgentSettingsRead";
    const PERMISSION: &'static str = "agent::settings::read";
    const DESCRIPTION: &'static str =
        "Read the deployment-wide agent policy (sandbox/approval mode, reviewer, token caps, fan-out).";
    const MODULE: &'static str = "agent";
}

/// Mutate the deployment-wide agent policy singleton. Admin-only;
/// Administrators have it implicitly via the `*` wildcard (no explicit grant
/// migration needed).
pub struct AgentSettingsManage;
impl PermissionCheck for AgentSettingsManage {
    const NAME: &'static str = "AgentSettingsManage";
    const PERMISSION: &'static str = "agent::settings::manage";
    const DESCRIPTION: &'static str =
        "Update the deployment-wide agent policy (sandbox/approval mode, reviewer, token caps, fan-out).";
    const MODULE: &'static str = "agent";
}

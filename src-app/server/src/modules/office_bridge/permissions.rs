//! Permission keys for the office_bridge module.

use crate::modules::permissions::types::PermissionCheck;

/// Use the built-in office-bridge tools (enumerate / read / act on open Office
/// documents). Granted to the default Users group by migration 133.
pub struct OfficeBridgeUse;
impl PermissionCheck for OfficeBridgeUse {
    const NAME: &'static str = "OfficeBridgeUse";
    const PERMISSION: &'static str = "office_bridge::use";
    const DESCRIPTION: &'static str = "Use the office-bridge tools for open Office documents.";
    const MODULE: &'static str = "office_bridge";
}

/// Read deployment-wide office-bridge settings.
pub struct OfficeBridgeAdminRead;
impl PermissionCheck for OfficeBridgeAdminRead {
    const NAME: &'static str = "OfficeBridgeAdminRead";
    const PERMISSION: &'static str = "office_bridge::admin::read";
    const DESCRIPTION: &'static str = "Read office-bridge settings (enable, port, connection state).";
    const MODULE: &'static str = "office_bridge";
}

/// Mutate deployment-wide office-bridge settings.
pub struct OfficeBridgeManage;
impl PermissionCheck for OfficeBridgeManage {
    const NAME: &'static str = "OfficeBridgeManage";
    const PERMISSION: &'static str = "office_bridge::admin::manage";
    const DESCRIPTION: &'static str = "Update office-bridge settings (enable, port).";
    const MODULE: &'static str = "office_bridge";
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// TEST-4 — the three `PermissionCheck` impls expose the exact permission
    /// strings the migrations (132/133) and the handlers gate on. Drift here
    /// would leave the migration granting / the extractor checking a string
    /// nobody else uses.
    #[test]
    fn permission_strings_are_exact() {
        assert_eq!(OfficeBridgeUse::PERMISSION, "office_bridge::use");
        assert_eq!(OfficeBridgeAdminRead::PERMISSION, "office_bridge::admin::read");
        assert_eq!(OfficeBridgeManage::PERMISSION, "office_bridge::admin::manage");
    }

    /// TEST-4 — every office_bridge permission reports the `office_bridge` module.
    #[test]
    fn permission_modules_are_office_bridge() {
        for module in [
            OfficeBridgeUse::MODULE,
            OfficeBridgeAdminRead::MODULE,
            OfficeBridgeManage::MODULE,
        ] {
            assert_eq!(module, "office_bridge");
        }
    }

    /// TEST-4 — the `NAME` constants are distinct (each impl is a separate key).
    #[test]
    fn permission_names_are_distinct() {
        let names = [
            OfficeBridgeUse::NAME,
            OfficeBridgeAdminRead::NAME,
            OfficeBridgeManage::NAME,
        ];
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "permission NAME constants must be distinct");
    }
}

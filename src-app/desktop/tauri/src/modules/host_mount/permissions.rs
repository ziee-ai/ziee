//! Host-mount permissions (desktop-owned), modeled on `remote_access`.
//!
//! Administrators receive both via the `*` wildcard; the explicit grants in
//! migration 10000000000005 are forward-looking.

use ziee::permissions::PermissionCheck;

/// Read host-mount configuration (scope mount lists + the deployment policy).
pub struct HostMountRead;
impl PermissionCheck for HostMountRead {
    const NAME: &'static str = "HostMountRead";
    const PERMISSION: &'static str = "host_mount::read";
    const DESCRIPTION: &'static str = "Read host-folder mount configuration and policy.";
    const MODULE: &'static str = "host_mount";
}

/// Create/update/clear host-folder mounts and edit the deployment policy.
pub struct HostMountManage;
impl PermissionCheck for HostMountManage {
    const NAME: &'static str = "HostMountManage";
    const PERMISSION: &'static str = "host_mount::manage";
    const DESCRIPTION: &'static str =
        "Configure host-folder mounts on projects/conversations and the host-mount policy.";
    const MODULE: &'static str = "host_mount";
}

//! Permission keys for the remote_access module.
//!
//! These are granted to the Administrators system group by migration
//! 65. Admin already has the `*` wildcard so the explicit grant is
//! forward-looking — lets us drop the wildcard later without breaking
//! remote-access.

use ziee::permissions::PermissionCheck;

/// Read remote-access settings + tunnel status.
pub struct RemoteAccessRead;
impl PermissionCheck for RemoteAccessRead {
    const NAME: &'static str = "RemoteAccessRead";
    const PERMISSION: &'static str = "remote_access::read";
    const DESCRIPTION: &'static str =
        "Read remote-access settings, tunnel status, and current public URL.";
    const MODULE: &'static str = "remote_access";
}

/// Modify remote-access settings, start / stop the tunnel, issue
/// magic-link tokens.
pub struct RemoteAccessManage;
impl PermissionCheck for RemoteAccessManage {
    const NAME: &'static str = "RemoteAccessManage";
    const PERMISSION: &'static str = "remote_access::manage";
    const DESCRIPTION: &'static str =
        "Save the ngrok auth token / custom domain, toggle auto-start, \
         toggle password authentication, start/stop the tunnel, and issue \
         magic-link login tokens.";
    const MODULE: &'static str = "remote_access";
}

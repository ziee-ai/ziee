//! Permission keys for the scheduler module.

use crate::modules::permissions::types::PermissionCheck;

/// Create, run, test, and manage YOUR OWN scheduled tasks. Granted to the
/// default Users group by migration 135. The task's TARGET execution is
/// re-checked downstream (workflow `workflows::execute` / model access at
/// spawn time), so this grants only the scheduling capability, not a bypass of
/// what the user could already run by hand.
pub struct SchedulerUse;
impl PermissionCheck for SchedulerUse {
    const NAME: &'static str = "SchedulerUse";
    const PERMISSION: &'static str = "scheduler::use";
    const DESCRIPTION: &'static str =
        "Create, run, test, and manage your own scheduled/recurring tasks.";
    const MODULE: &'static str = "scheduler";
}

/// Read the deployment-wide scheduler admin settings (quota / cadence floor /
/// failure cap / notification retention). Admin-only via the Administrators
/// `*` wildcard.
pub struct SchedulerAdminRead;
impl PermissionCheck for SchedulerAdminRead {
    const NAME: &'static str = "SchedulerAdminRead";
    const PERMISSION: &'static str = "scheduler::admin::read";
    const DESCRIPTION: &'static str = "View deployment-wide scheduler settings.";
    const MODULE: &'static str = "scheduler";
}

/// Modify the deployment-wide scheduler admin settings. Admin-only via the
/// Administrators `*` wildcard.
pub struct SchedulerAdminManage;
impl PermissionCheck for SchedulerAdminManage {
    const NAME: &'static str = "SchedulerAdminManage";
    const PERMISSION: &'static str = "scheduler::admin::manage";
    const DESCRIPTION: &'static str = "Change deployment-wide scheduler settings.";
    const MODULE: &'static str = "scheduler";
}

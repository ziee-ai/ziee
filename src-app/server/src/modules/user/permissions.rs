// User permissions

use crate::modules::permissions::{PermissionCheck, PermissionInfo};

// =====================================================
// Profile Permissions (user's own profile)
// =====================================================

/// Permission for users to view their own profile
pub struct ProfileRead;
impl PermissionCheck for ProfileRead {
    const NAME: &'static str = "ProfileRead";
    const PERMISSION: &'static str = "profile::read";
    const DESCRIPTION: &'static str = "View own profile information";
    const MODULE: &'static str = "user";
}

/// Permission for users to edit their own profile
pub struct ProfileEdit;
impl PermissionCheck for ProfileEdit {
    const NAME: &'static str = "ProfileEdit";
    const PERMISSION: &'static str = "profile::edit";
    const DESCRIPTION: &'static str = "Edit own profile information";
    const MODULE: &'static str = "user";
}

// =====================================================
// User Management Permissions
// =====================================================

/// Permission to view user information and list users
pub struct UsersRead;
impl PermissionCheck for UsersRead {
    const NAME: &'static str = "UsersRead";
    const PERMISSION: &'static str = "users::read";
    const DESCRIPTION: &'static str = "View user information and list users";
    const MODULE: &'static str = "user";
}

/// Permission to create new user accounts
pub struct UsersCreate;
impl PermissionCheck for UsersCreate {
    const NAME: &'static str = "UsersCreate";
    const PERMISSION: &'static str = "users::create";
    const DESCRIPTION: &'static str = "Create new user accounts";
    const MODULE: &'static str = "user";
}

/// Permission to edit existing user information
pub struct UsersEdit;
impl PermissionCheck for UsersEdit {
    const NAME: &'static str = "UsersEdit";
    const PERMISSION: &'static str = "users::edit";
    const DESCRIPTION: &'static str = "Edit existing user information";
    const MODULE: &'static str = "user";
}

/// Permission to delete user accounts
pub struct UsersDelete;
impl PermissionCheck for UsersDelete {
    const NAME: &'static str = "UsersDelete";
    const PERMISSION: &'static str = "users::delete";
    const DESCRIPTION: &'static str = "Delete user accounts";
    const MODULE: &'static str = "user";
}

/// Permission to reset user passwords
pub struct UsersResetPassword;
impl PermissionCheck for UsersResetPassword {
    const NAME: &'static str = "UsersResetPassword";
    const PERMISSION: &'static str = "users::reset_password";
    const DESCRIPTION: &'static str = "Reset user passwords";
    const MODULE: &'static str = "user";
}

/// Permission to toggle user active status
pub struct UsersToggleStatus;
impl PermissionCheck for UsersToggleStatus {
    const NAME: &'static str = "UsersToggleStatus";
    const PERMISSION: &'static str = "users::toggle_status";
    const DESCRIPTION: &'static str = "Enable or disable user accounts";
    const MODULE: &'static str = "user";
}

// =====================================================
// Group Management Permissions
// =====================================================

/// Permission to view groups
pub struct GroupsRead;
impl PermissionCheck for GroupsRead {
    const NAME: &'static str = "GroupsRead";
    const PERMISSION: &'static str = "groups::read";
    const DESCRIPTION: &'static str = "View groups and group information";
    const MODULE: &'static str = "user";
}

/// Permission to create new groups
pub struct GroupsCreate;
impl PermissionCheck for GroupsCreate {
    const NAME: &'static str = "GroupsCreate";
    const PERMISSION: &'static str = "groups::create";
    const DESCRIPTION: &'static str = "Create new groups";
    const MODULE: &'static str = "user";
}

/// Permission to edit existing groups
pub struct GroupsEdit;
impl PermissionCheck for GroupsEdit {
    const NAME: &'static str = "GroupsEdit";
    const PERMISSION: &'static str = "groups::edit";
    const DESCRIPTION: &'static str = "Edit existing group information and permissions";
    const MODULE: &'static str = "user";
}

/// Permission to delete groups
pub struct GroupsDelete;
impl PermissionCheck for GroupsDelete {
    const NAME: &'static str = "GroupsDelete";
    const PERMISSION: &'static str = "groups::delete";
    const DESCRIPTION: &'static str = "Delete non-system groups";
    const MODULE: &'static str = "user";
}

/// Permission to assign users to groups
pub struct GroupsAssignUsers;
impl PermissionCheck for GroupsAssignUsers {
    const NAME: &'static str = "GroupsAssignUsers";
    const PERMISSION: &'static str = "groups::assign_users";
    const DESCRIPTION: &'static str = "Assign users to groups and remove users from groups";
    const MODULE: &'static str = "user";
}

// =====================================================
// Helper Function to Collect All Permissions
// =====================================================

/// Get all user module permissions
// Introspection entry point (the sole in-crate consumer of the permission
// `to_info()` / `PermissionInfo` API). Not wired to a `/permissions` handler
// yet; retained so the introspection subsystem stays exercised end-to-end.
#[allow(dead_code)]
pub fn all_permissions() -> Vec<PermissionInfo> {
    vec![
        ProfileRead::to_info(),
        ProfileEdit::to_info(),
        UsersRead::to_info(),
        UsersCreate::to_info(),
        UsersEdit::to_info(),
        UsersDelete::to_info(),
        UsersResetPassword::to_info(),
        UsersToggleStatus::to_info(),
        GroupsRead::to_info(),
        GroupsCreate::to_info(),
        GroupsEdit::to_info(),
        GroupsDelete::to_info(),
        GroupsAssignUsers::to_info(),
    ]
}


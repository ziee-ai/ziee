// Permission system infrastructure
#![allow(dead_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// =====================================================
// Permission Trait
// =====================================================

/// Trait for compile-time permission definitions
/// Each module implements this trait for their permissions
pub trait PermissionCheck: Send + Sync + 'static {
    /// Short name for the permission (e.g., "UsersRead")
    const NAME: &'static str;

    /// The permission string (e.g., "users::read")
    const PERMISSION: &'static str;

    /// Human-readable description for documentation
    const DESCRIPTION: &'static str;

    /// The module this permission belongs to
    const MODULE: &'static str;

    /// Extract resource name from permission (e.g., "users" from "users::read")
    fn resource() -> &'static str {
        Self::PERMISSION.split("::").next().unwrap_or("")
    }

    /// Extract action name from permission (e.g., "read" from "users::read")
    fn action() -> &'static str {
        Self::PERMISSION.split("::").last().unwrap_or("")
    }

    /// Convert to PermissionInfo for API response
    fn to_info() -> PermissionInfo {
        PermissionInfo {
            permission: Self::PERMISSION.to_string(),
            description: Self::DESCRIPTION.to_string(),
            module: Self::MODULE.to_string(),
            resource: Self::resource().to_string(),
            action: Self::action().to_string(),
        }
    }
}

// =====================================================
// Permission List Trait (for multiple permissions)
// =====================================================

/// Trait for permission lists (single or multiple permissions)
/// This allows RequirePermissions to accept both single permissions
/// and tuples of multiple permissions.
pub trait PermissionList: Send + Sync + 'static {
    /// Get all permission names
    fn names() -> Vec<&'static str>;

    /// Get all permission strings
    fn permissions() -> Vec<&'static str>;

    /// Get all permission descriptions
    fn descriptions() -> Vec<&'static str>;

    /// Get formatted description for OpenAPI docs
    fn format_description() -> String {
        let perms = Self::permissions();
        let descs = Self::descriptions();

        if perms.len() == 1 {
            format!(
                "\n\n**Required Permission:** `{}`\n\n{}",
                perms[0], descs[0]
            )
        } else {
            let mut result = String::from("\n\n**Required Permissions (ALL):**\n");
            for (perm, desc) in perms.iter().zip(descs.iter()) {
                result.push_str(&format!("- `{}` - {}\n", perm, desc));
            }
            result
        }
    }
}

// Implement PermissionList for single permission
impl<P: PermissionCheck> PermissionList for (P,) {
    fn names() -> Vec<&'static str> {
        vec![P::NAME]
    }

    fn permissions() -> Vec<&'static str> {
        vec![P::PERMISSION]
    }

    fn descriptions() -> Vec<&'static str> {
        vec![P::DESCRIPTION]
    }
}

// Implement PermissionList for 2 permissions
impl<P1, P2> PermissionList for (P1, P2)
where
    P1: PermissionCheck,
    P2: PermissionCheck,
{
    fn names() -> Vec<&'static str> {
        vec![P1::NAME, P2::NAME]
    }

    fn permissions() -> Vec<&'static str> {
        vec![P1::PERMISSION, P2::PERMISSION]
    }

    fn descriptions() -> Vec<&'static str> {
        vec![P1::DESCRIPTION, P2::DESCRIPTION]
    }
}

// Implement PermissionList for 3 permissions
impl<P1, P2, P3> PermissionList for (P1, P2, P3)
where
    P1: PermissionCheck,
    P2: PermissionCheck,
    P3: PermissionCheck,
{
    fn names() -> Vec<&'static str> {
        vec![P1::NAME, P2::NAME, P3::NAME]
    }

    fn permissions() -> Vec<&'static str> {
        vec![P1::PERMISSION, P2::PERMISSION, P3::PERMISSION]
    }

    fn descriptions() -> Vec<&'static str> {
        vec![P1::DESCRIPTION, P2::DESCRIPTION, P3::DESCRIPTION]
    }
}

// Implement PermissionList for 4 permissions
impl<P1, P2, P3, P4> PermissionList for (P1, P2, P3, P4)
where
    P1: PermissionCheck,
    P2: PermissionCheck,
    P3: PermissionCheck,
    P4: PermissionCheck,
{
    fn names() -> Vec<&'static str> {
        vec![P1::NAME, P2::NAME, P3::NAME, P4::NAME]
    }

    fn permissions() -> Vec<&'static str> {
        vec![
            P1::PERMISSION,
            P2::PERMISSION,
            P3::PERMISSION,
            P4::PERMISSION,
        ]
    }

    fn descriptions() -> Vec<&'static str> {
        vec![
            P1::DESCRIPTION,
            P2::DESCRIPTION,
            P3::DESCRIPTION,
            P4::DESCRIPTION,
        ]
    }
}

// =====================================================
// Permission Info (for API responses)
// =====================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PermissionInfo {
    /// The permission string (e.g., "users::read")
    pub permission: String,
    /// Human-readable description
    pub description: String,
    /// The module this permission belongs to
    pub module: String,
    /// The resource being accessed
    pub resource: String,
    /// The action being performed
    pub action: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UsersRead;
    impl PermissionCheck for UsersRead {
        const NAME: &'static str = "UsersRead";
        const PERMISSION: &'static str = "users::read";
        const DESCRIPTION: &'static str = "Read users";
        const MODULE: &'static str = "users";
    }

    // A permission with a namespaced action to exercise the split logic.
    struct CoreMemoryWrite;
    impl PermissionCheck for CoreMemoryWrite {
        const NAME: &'static str = "CoreMemoryWrite";
        const PERMISSION: &'static str = "memory::core::write";
        const DESCRIPTION: &'static str = "Write core memory";
        const MODULE: &'static str = "memory";
    }

    struct UsersDelete;
    impl PermissionCheck for UsersDelete {
        const NAME: &'static str = "UsersDelete";
        const PERMISSION: &'static str = "users::delete";
        const DESCRIPTION: &'static str = "Delete users";
        const MODULE: &'static str = "users";
    }

    struct GroupsRead;
    impl PermissionCheck for GroupsRead {
        const NAME: &'static str = "GroupsRead";
        const PERMISSION: &'static str = "groups::read";
        const DESCRIPTION: &'static str = "Read groups";
        const MODULE: &'static str = "groups";
    }

    /// `PermissionList` for a 3-tuple collects all three permissions' name /
    /// permission / description in order — the RequirePermissions<(A,B,C)> path.
    #[test]
    fn permission_list_three_tuple_collects_all() {
        type Three = (UsersRead, CoreMemoryWrite, UsersDelete);
        assert_eq!(
            <Three as PermissionList>::permissions(),
            vec!["users::read", "memory::core::write", "users::delete"]
        );
        assert_eq!(
            <Three as PermissionList>::names(),
            vec!["UsersRead", "CoreMemoryWrite", "UsersDelete"]
        );
        assert_eq!(
            <Three as PermissionList>::descriptions(),
            vec!["Read users", "Write core memory", "Delete users"]
        );
    }

    /// `PermissionList` for a 4-tuple — RequirePermissions<(A,B,C,D)>.
    #[test]
    fn permission_list_four_tuple_collects_all() {
        type Four = (UsersRead, CoreMemoryWrite, UsersDelete, GroupsRead);
        assert_eq!(
            <Four as PermissionList>::permissions(),
            vec!["users::read", "memory::core::write", "users::delete", "groups::read"]
        );
        assert_eq!(<Four as PermissionList>::names().len(), 4);
        assert_eq!(<Four as PermissionList>::descriptions().len(), 4);
    }

    #[test]
    fn resource_is_first_segment_action_is_last() {
        assert_eq!(UsersRead::resource(), "users");
        assert_eq!(UsersRead::action(), "read");
        // For a 3-segment permission, resource() takes the FIRST segment and
        // action() the LAST.
        assert_eq!(CoreMemoryWrite::resource(), "memory");
        assert_eq!(CoreMemoryWrite::action(), "write");
    }

    #[test]
    fn to_info_projects_all_fields() {
        let info = UsersRead::to_info();
        assert_eq!(info.permission, "users::read");
        assert_eq!(info.description, "Read users");
        assert_eq!(info.module, "users");
        assert_eq!(info.resource, "users");
        assert_eq!(info.action, "read");
    }

    #[test]
    fn to_info_serializes_to_expected_json_shape() {
        let info = CoreMemoryWrite::to_info();
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["permission"], "memory::core::write");
        assert_eq!(v["module"], "memory");
        assert_eq!(v["resource"], "memory");
        assert_eq!(v["action"], "write");
    }
}

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
mod permission_check_tests {
    use super::{PermissionCheck, PermissionInfo};

    struct TwoSeg;
    impl PermissionCheck for TwoSeg {
        const NAME: &'static str = "UsersRead";
        const PERMISSION: &'static str = "users::read";
        const DESCRIPTION: &'static str = "Read users";
        const MODULE: &'static str = "users";
    }

    struct ThreeSeg;
    impl PermissionCheck for ThreeSeg {
        const NAME: &'static str = "CodeSandboxResourceLimitsManage";
        const PERMISSION: &'static str = "code_sandbox::resource_limits::manage";
        const DESCRIPTION: &'static str = "Manage limits";
        const MODULE: &'static str = "code_sandbox";
    }

    #[test]
    fn resource_is_first_segment_action_is_last() {
        assert_eq!(TwoSeg::resource(), "users");
        assert_eq!(TwoSeg::action(), "read");
        // For a 3-segment permission, resource = first, action = last.
        assert_eq!(ThreeSeg::resource(), "code_sandbox");
        assert_eq!(ThreeSeg::action(), "manage");
    }

    #[test]
    fn to_info_projects_all_fields() {
        let info: PermissionInfo = TwoSeg::to_info();
        assert_eq!(info.permission, "users::read");
        assert_eq!(info.description, "Read users");
        assert_eq!(info.module, "users");
        assert_eq!(info.resource, "users");
        assert_eq!(info.action, "read");
    }

    // --- RequirePermissions tuple (PermissionList) AND-combination ---
    //
    // RequirePermissions<L> requires ALL of `L::permissions()`; the extractor
    // grants access only when the caller holds every entry. The tuple impls for
    // 3 and 4 permissions (types.rs:119-168) were previously untested — these
    // assert that a 3- and 4-tuple surface the COMPLETE, ordered permission set
    // (so the extractor AND-checks all of them) and that the OpenAPI
    // `format_description` advertises every one under the "ALL" header.
    use super::PermissionList;

    struct PRead;
    impl PermissionCheck for PRead {
        const NAME: &'static str = "ProjectsRead";
        const PERMISSION: &'static str = "projects::read";
        const DESCRIPTION: &'static str = "Read projects";
        const MODULE: &'static str = "projects";
    }
    struct PWrite;
    impl PermissionCheck for PWrite {
        const NAME: &'static str = "ProjectsEdit";
        const PERMISSION: &'static str = "projects::edit";
        const DESCRIPTION: &'static str = "Edit projects";
        const MODULE: &'static str = "projects";
    }
    struct PDelete;
    impl PermissionCheck for PDelete {
        const NAME: &'static str = "ProjectsDelete";
        const PERMISSION: &'static str = "projects::delete";
        const DESCRIPTION: &'static str = "Delete projects";
        const MODULE: &'static str = "projects";
    }
    struct PShare;
    impl PermissionCheck for PShare {
        const NAME: &'static str = "ProjectsShare";
        const PERMISSION: &'static str = "projects::share";
        const DESCRIPTION: &'static str = "Share projects";
        const MODULE: &'static str = "projects";
    }

    #[test]
    fn three_tuple_yields_all_three_permissions_in_order() {
        type Required = (PRead, PWrite, PDelete);
        assert_eq!(
            Required::permissions(),
            vec!["projects::read", "projects::edit", "projects::delete"],
        );
        assert_eq!(
            Required::names(),
            vec!["ProjectsRead", "ProjectsEdit", "ProjectsDelete"],
        );
        assert_eq!(
            Required::descriptions(),
            vec!["Read projects", "Edit projects", "Delete projects"],
        );

        // The required set is the full AND-set: holding any 2 of the 3 is not
        // a superset, so the extractor would reject — assert all 3 are present
        // and none is dropped.
        let required = Required::permissions();
        assert_eq!(required.len(), 3, "a 3-tuple must require exactly 3 perms");
        for p in ["projects::read", "projects::edit", "projects::delete"] {
            assert!(required.contains(&p), "missing required perm {p}");
        }
    }

    #[test]
    fn four_tuple_yields_all_four_permissions_in_order() {
        type Required = (PRead, PWrite, PDelete, PShare);
        assert_eq!(
            Required::permissions(),
            vec![
                "projects::read",
                "projects::edit",
                "projects::delete",
                "projects::share",
            ],
        );
        assert_eq!(Required::permissions().len(), 4);
        assert_eq!(Required::descriptions().len(), 4);
    }

    #[test]
    fn multi_permission_format_description_lists_all_under_all_header() {
        // 3+ tuples render the multi-permission "ALL" form (not the single
        // "Required Permission" form), advertising every required permission.
        let doc = <(PRead, PWrite, PDelete) as PermissionList>::format_description();
        assert!(
            doc.contains("**Required Permissions (ALL):**"),
            "3-tuple must use the ALL header, got: {doc}",
        );
        for line in [
            "- `projects::read` - Read projects",
            "- `projects::edit` - Edit projects",
            "- `projects::delete` - Delete projects",
        ] {
            assert!(doc.contains(line), "format_description missing line: {line}");
        }
        // The single-permission phrasing must NOT appear for a multi-tuple.
        assert!(!doc.contains("**Required Permission:**"));
    }
}

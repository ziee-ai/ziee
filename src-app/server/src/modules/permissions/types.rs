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

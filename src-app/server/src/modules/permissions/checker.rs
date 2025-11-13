use crate::modules::user::models::{Group, User};

// =====================================================
// Permission Checking Logic
// =====================================================

/// Check if user has permission via UNION of user permissions and group permissions
/// This is the primary permission check function used by the extractors
pub fn check_permission_union(user: &User, groups: &[Group], required_permission: &str) -> bool {
    // First check user's direct permissions
    if check_permissions_array(&user.permissions, required_permission) {
        return true;
    }

    // Then check all active group permissions
    for group in groups {
        if !group.is_active {
            continue;
        }

        if check_permissions_array(&group.permissions, required_permission) {
            return true;
        }
    }

    false
}

/// Check if a permission array contains the required permission
/// Supports exact match, wildcards, and hierarchical wildcards
fn check_permissions_array(permissions: &[String], required_permission: &str) -> bool {
    // Check exact match
    if permissions.contains(&required_permission.to_string()) {
        return true;
    }

    // Check full wildcard
    if permissions.contains(&"*".to_string()) {
        return true;
    }

    // Check hierarchical wildcards
    // "users::*" matches "users::read", "users::edit"
    // "config::auth::*" matches "config::auth::read", "config::auth::edit"
    let parts: Vec<&str> = required_permission.split("::").collect();
    for i in 1..parts.len() {
        let prefix = parts[0..i].join("::");
        let wildcard = format!("{}::*", prefix);
        if permissions.contains(&wildcard) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn create_test_user_with_permissions(permissions: Vec<&str>) -> User {
        User {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            email_verified: true,
            password_hash: Some("hash".to_string()),
            display_name: Some("Test User".to_string()),
            avatar_url: None,
            is_active: true,
            is_admin: false,
            permissions: permissions.into_iter().map(String::from).collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        }
    }

    fn create_test_group(permissions: Vec<&str>) -> Group {
        Group {
            id: Uuid::new_v4(),
            name: "testgroup".to_string(),
            description: Some("Test Group".to_string()),
            permissions: permissions.into_iter().map(String::from).collect(),
            is_system: false,
            is_active: true,
            is_default: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_user_permission_only() {
        let user = create_test_user_with_permissions(vec!["users::read"]);
        let groups = vec![];
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(!check_permission_union(&user, &groups, "users::edit"));
    }

    #[test]
    fn test_group_permission_only() {
        let user = create_test_user_with_permissions(vec![]);
        let groups = vec![create_test_group(vec!["users::read"])];
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(!check_permission_union(&user, &groups, "users::edit"));
    }

    #[test]
    fn test_permission_union() {
        // User has users::read, group has users::edit
        let user = create_test_user_with_permissions(vec!["users::read"]);
        let groups = vec![create_test_group(vec!["users::edit"])];

        // Should have BOTH permissions via union
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(check_permission_union(&user, &groups, "users::edit"));
        assert!(!check_permission_union(&user, &groups, "users::delete"));
    }

    #[test]
    fn test_wildcard_all_user_level() {
        let user = create_test_user_with_permissions(vec!["*"]);
        let groups = vec![];
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(check_permission_union(&user, &groups, "anything::else"));
    }

    #[test]
    fn test_wildcard_all_group_level() {
        let user = create_test_user_with_permissions(vec![]);
        let groups = vec![create_test_group(vec!["*"])];
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(check_permission_union(&user, &groups, "anything::else"));
    }

    #[test]
    fn test_resource_wildcard() {
        let user = create_test_user_with_permissions(vec!["users::*"]);
        let groups = vec![];
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(check_permission_union(&user, &groups, "users::edit"));
        assert!(!check_permission_union(&user, &groups, "groups::read"));
    }

    #[test]
    fn test_hierarchical_wildcard() {
        let user = create_test_user_with_permissions(vec![]);
        let groups = vec![create_test_group(vec!["config::auth::*"])];
        assert!(check_permission_union(&user, &groups, "config::auth::read"));
        assert!(check_permission_union(&user, &groups, "config::auth::edit"));
        assert!(!check_permission_union(
            &user,
            &groups,
            "config::proxy::read"
        ));
    }

    #[test]
    fn test_inactive_group_ignored() {
        let user = create_test_user_with_permissions(vec![]);
        let mut group = create_test_group(vec!["*"]);
        group.is_active = false;
        let groups = vec![group];
        assert!(!check_permission_union(&user, &groups, "users::read"));
    }

    #[test]
    fn test_multiple_groups() {
        let user = create_test_user_with_permissions(vec!["chat::read"]);
        let groups = vec![
            create_test_group(vec!["users::read"]),
            create_test_group(vec!["groups::edit"]),
        ];

        // Has permissions from user and both groups
        assert!(check_permission_union(&user, &groups, "chat::read"));
        assert!(check_permission_union(&user, &groups, "users::read"));
        assert!(check_permission_union(&user, &groups, "groups::edit"));
        assert!(!check_permission_union(&user, &groups, "config::read"));
    }

    #[test]
    fn test_no_permissions() {
        let user = create_test_user_with_permissions(vec![]);
        let groups = vec![];
        assert!(!check_permission_union(&user, &groups, "users::read"));
    }
}

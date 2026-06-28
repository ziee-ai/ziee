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
            password_changed_at: None,
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

    // audit id all-d6976975c860 — permission exhaustion with large sets. The
    // linear scan in check_permissions_array must still resolve correctly when a
    // user/group carries thousands of permissions: an exact match deep in the
    // list, a miss against the whole set, and a hierarchical wildcard buried
    // among many unrelated entries.
    #[test]
    fn test_large_permission_set_exact_and_miss() {
        // 5000 distinct non-matching permissions spread across user + groups.
        let bulk: Vec<String> = (0..4000).map(|i| format!("mod{i}::action{i}")).collect();
        let mut user_perms = bulk.clone();
        user_perms.push("target::read".to_string()); // the needle, last in the list
        let user = create_test_user_with_permissions(
            user_perms.iter().map(String::as_str).collect(),
        );
        let group_perms: Vec<String> =
            (4000..5000).map(|i| format!("grp{i}::x")).collect();
        let groups = vec![create_test_group(
            group_perms.iter().map(String::as_str).collect(),
        )];

        // Exact match present despite the size of the set.
        assert!(check_permission_union(&user, &groups, "target::read"));
        // A permission absent from BOTH large sets is denied.
        assert!(!check_permission_union(&user, &groups, "target::write"));
        assert!(!check_permission_union(&user, &groups, "nowhere::at::all"));
    }

    #[test]
    fn test_large_permission_set_hierarchical_wildcard_buried() {
        let mut bulk: Vec<String> =
            (0..3000).map(|i| format!("noise{i}::leaf")).collect();
        // A hierarchical wildcard buried in the middle of the haystack.
        bulk.insert(1500, "config::auth::*".to_string());
        let group = create_test_group(bulk.iter().map(String::as_str).collect());
        let user = create_test_user_with_permissions(vec![]);
        let groups = vec![group];

        assert!(check_permission_union(&user, &groups, "config::auth::read"));
        assert!(check_permission_union(&user, &groups, "config::auth::edit"));
        // Sibling namespace not covered by config::auth::*.
        assert!(!check_permission_union(&user, &groups, "config::proxy::read"));
    }

    // audit id all-e6ee49d03464 — deeply nested (4+ level) hierarchical
    // wildcards. The prefix loop (checker.rs:45-52) handles arbitrary depth;
    // existing tests stop at 3 levels (config::auth::*). Pin that a 4-level
    // wildcard matches a 4-level (and deeper) permission, and that a wildcard
    // one level too shallow/specific does NOT over- or under-match.
    #[test]
    fn deeply_nested_wildcard_matches_four_plus_levels() {
        let user = create_test_user_with_permissions(vec!["a::b::c::*"]);
        let groups = vec![];
        // 4-level exact-prefix match.
        assert!(check_permission_union(&user, &groups, "a::b::c::read"));
        // Deeper (5-level) still under the wildcard prefix.
        assert!(check_permission_union(&user, &groups, "a::b::c::d::execute"));
        // A sibling at the 3rd level is NOT covered.
        assert!(!check_permission_union(&user, &groups, "a::b::x::read"));
        // The wildcard does NOT grant the bare prefix as a leaf permission.
        assert!(!check_permission_union(&user, &groups, "a::b"));

        // A deeper wildcard must not match a shallower required permission.
        let deep = create_test_user_with_permissions(vec!["a::b::c::d::*"]);
        assert!(!check_permission_union(&deep, &groups, "a::b::c::read"));
        assert!(check_permission_union(&deep, &groups, "a::b::c::d::read"));
    }
}

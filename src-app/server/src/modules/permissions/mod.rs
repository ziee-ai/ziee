// =====================================================
// Permissions Module
// =====================================================
//
// This module provides a type-safe, extractor-based permission system
// that supports:
// - Two-level permissions (user-level + group-level with union)
// - Wildcard support (*, resource::*, namespace::resource::*)
// - Root admin bypass (is_admin = true)
// - Module-owned permission declarations
//
// Usage:
// ```rust
// use crate::modules::permissions::{RequirePermissions, PermissionCheck};
// use crate::modules::user::permissions::UsersRead;
//
// async fn list_users(
//     RequirePermissions::<UsersRead> { user, .. }: RequirePermissions<UsersRead>,
//     State(pool): State<PgPool>,
// ) -> ApiResult<Json<UserListResponse>> {
//     // User is authenticated and authorized with users::read permission
// }
// ```

pub mod checker;
pub mod extractors;
pub mod openapi;
pub mod types;

// Re-export main types
pub use extractors::RequirePermissions;
pub use openapi::with_permission;
pub use types::{PermissionCheck, PermissionInfo};

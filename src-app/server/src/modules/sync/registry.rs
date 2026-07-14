//! Sync registry — ziee's concrete instantiation of the framework core.
//!
//! Chunk B5 moved the per-user SSE connection registry (caps / pruning /
//! self-echo + the `Owner`/`Perm`/`Everyone` delivery routing) into
//! `ziee_framework::sync`, generic over the app's per-connection permission
//! snapshot (`Principal`). What stays here is the app-owned half:
//!
//! - [`SyncConnPrincipal`] — ziee's concrete snapshot (`User` + its active
//!   `Group`s). Its `Principal` impl is byte-equivalent to
//!   `check_permission_union(user, groups, ..)`: direct permissions UNION each
//!   ACTIVE group's permissions, admin excluded (a call-site short-circuit).
//! - The process-wide singleton [`registry()`], pinned to `SyncConnPrincipal`.
//!
//! The registry TYPE + all delivery logic + its unit tests live in the
//! framework; the wire types (`SyncEvent`/`SyncSseEvent`) stay in `event.rs`.

use lazy_static::lazy_static;

use ziee_framework::sync::SyncRegistry;
use ziee_identity::Principal;

use crate::modules::user::models::{Group, User};

// Re-export the framework registry surface the handler consumes, so
// `super::registry::{ClientConn, SYNC_CHANNEL_CAPACITY}` keeps resolving.
pub use ziee_framework::sync::{ClientConn, SYNC_CHANNEL_CAPACITY};

/// ziee's per-connection permission snapshot: the acting `User` plus its groups,
/// captured at connect and refreshed by the handler's periodic re-check. The
/// `Principal` impl reproduces `check_permission_union` exactly (direct perms
/// UNION each ACTIVE group's perms; the admin flag is applied by the framework
/// router as a separate short-circuit, matching the former
/// `conn.is_admin || check_permission_union(..)`).
pub struct SyncConnPrincipal {
    pub user: User,
    pub groups: Vec<Group>,
}

impl Principal for SyncConnPrincipal {
    fn is_admin(&self) -> bool {
        self.user.is_admin
    }

    fn direct_permissions(&self) -> &[String] {
        &self.user.permissions
    }

    fn active_group_permissions(&self) -> Vec<&[String]> {
        self.groups
            .iter()
            .filter(|g| g.is_active)
            .map(|g| g.permissions.as_slice())
            .collect()
    }
}

lazy_static! {
    static ref REGISTRY: SyncRegistry<SyncConnPrincipal> = SyncRegistry::new();
}

/// Process-wide singleton registry, keyed to ziee's `SyncConnPrincipal`.
pub fn registry() -> &'static SyncRegistry<SyncConnPrincipal> {
    &REGISTRY
}

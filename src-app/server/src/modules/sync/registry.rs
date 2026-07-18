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

// Chunk sdk-surfaces moved the SSE subscribe handler into
// `ziee_framework::sync::sync_routes`, so ziee no longer names `ClientConn` /
// `SYNC_CHANNEL_CAPACITY` directly (the framework handler consumes them). What
// ziee still owns is its concrete `SyncConnPrincipal` + the singleton `registry()`
// the `SyncSurface` impl (`event.rs`) returns to the framework.

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

/// Build the snapshot from an authenticated `User` + its groups. The
/// framework's mountable `sync_routes()` (chunk sdk-surfaces) constructs the
/// per-connection principal through this `From`, linking ziee's
/// `ZieeIdentityResolver::{User, Group}` to `SyncConnPrincipal` at the mount
/// site (`sync_routes::<ZieeIdentityResolver, SyncEntity>()`).
impl From<(User, Vec<Group>)> for SyncConnPrincipal {
    fn from((user, groups): (User, Vec<Group>)) -> Self {
        Self { user, groups }
    }
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

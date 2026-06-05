//! In-process, per-user SSE connection registry for realtime sync.
//!
//! Unlike the global broadcast pool used by download/hardware SSE (every
//! connected client receives every event), this registry is **keyed by
//! user** so a `sync::publish` targets exactly one user's connections, a
//! permission-holding subset, or everyone — a change to user A's data is
//! never delivered to user B. Single-process / single-Postgres today; a
//! future multi-instance deployment would fan out via LISTEN/NOTIFY.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use axum::http::StatusCode;
use axum::response::sse::Event;
use lazy_static::lazy_static;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::permissions::checker::check_permission_union;
use crate::modules::user::models::{Group, User};

use super::event::{Audience, SyncEvent, SyncSseEvent};

/// Global cap on concurrent sync SSE connections across all users.
const GLOBAL_MAX_CONNECTIONS: usize = 512;
/// Per-user cap (multiple tabs/devices). Bounds a single account from
/// exhausting the global pool.
const PER_USER_MAX_CONNECTIONS: usize = 12;

type ConnId = Uuid;

/// One live SSE connection's server-side state. `user`/`groups`/`is_admin`
/// are the permission snapshot captured at connect (refreshed by the
/// handler's periodic re-check) and consulted when routing
/// `Permission`-audience events.
pub struct ClientConn {
    pub user_id: Uuid,
    pub is_admin: bool,
    pub user: User,
    pub groups: Vec<Group>,
    pub sender: UnboundedSender<Result<Event, axum::Error>>,
}

struct RegistryInner {
    clients: HashMap<ConnId, ClientConn>,
    by_user: HashMap<Uuid, HashSet<ConnId>>,
}

pub struct SyncRegistry {
    inner: Mutex<RegistryInner>,
}

lazy_static! {
    static ref REGISTRY: SyncRegistry = SyncRegistry {
        inner: Mutex::new(RegistryInner {
            clients: HashMap::new(),
            by_user: HashMap::new(),
        }),
    };
}

/// Process-wide singleton registry.
pub fn registry() -> &'static SyncRegistry {
    &REGISTRY
}

impl SyncRegistry {
    /// Register a new connection. Returns a 429 `AppError` when a global
    /// or per-user connection cap is hit.
    pub fn register(&self, conn_id: ConnId, conn: ClientConn) -> Result<(), AppError> {
        let mut inner = self.inner.lock().unwrap();

        if inner.clients.len() >= GLOBAL_MAX_CONNECTIONS {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "SYNC_GLOBAL_LIMIT",
                "Realtime sync is at capacity; retry shortly",
            ));
        }
        let user_count = inner.by_user.get(&conn.user_id).map_or(0, |s| s.len());
        if user_count >= PER_USER_MAX_CONNECTIONS {
            return Err(AppError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "SYNC_USER_LIMIT",
                "Too many open sync connections for this account",
            ));
        }

        inner.by_user.entry(conn.user_id).or_default().insert(conn_id);
        inner.clients.insert(conn_id, conn);
        Ok(())
    }

    /// Remove a connection (called on stream termination).
    pub fn unregister(&self, conn_id: ConnId) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(conn) = inner.clients.remove(&conn_id) {
            if let Some(set) = inner.by_user.get_mut(&conn.user_id) {
                set.remove(&conn_id);
                if set.is_empty() {
                    inner.by_user.remove(&conn.user_id);
                }
            }
        }
    }

    /// Refresh a connection's permission snapshot (the periodic re-check).
    pub fn refresh(&self, conn_id: ConnId, user: User, groups: Vec<Group>) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(conn) = inner.clients.get_mut(&conn_id) {
            conn.is_admin = user.is_admin;
            conn.user = user;
            conn.groups = groups;
        }
    }

    /// Route one event to the connections its audience permits, skipping
    /// the originating connection (self-echo suppression).
    pub fn deliver(&self, audience: Audience, event: SyncEvent, origin_conn: Option<ConnId>) {
        let sse: Event = SyncSseEvent::Sync(event).into();
        let inner = self.inner.lock().unwrap();

        let send = |conn_id: &ConnId, conn: &ClientConn| {
            if Some(*conn_id) == origin_conn {
                return;
            }
            // Errors mean the receiver was dropped; the ConnGuard on the
            // stream unregisters it, so we just skip here.
            let _ = conn.sender.send(Ok(sse.clone()));
        };

        match audience {
            Audience::Owner(uid) => {
                if let Some(set) = inner.by_user.get(&uid) {
                    for cid in set {
                        if let Some(conn) = inner.clients.get(cid) {
                            send(cid, conn);
                        }
                    }
                }
            }
            Audience::Permission(perm) => {
                for (cid, conn) in inner.clients.iter() {
                    if conn.is_admin || check_permission_union(&conn.user, &conn.groups, perm) {
                        send(cid, conn);
                    }
                }
            }
            Audience::Everyone => {
                for (cid, conn) in inner.clients.iter() {
                    send(cid, conn);
                }
            }
        }
    }

    /// Number of live connections (test/diagnostic helper).
    #[allow(dead_code)]
    pub fn connection_count(&self) -> usize {
        self.inner.lock().unwrap().clients.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::sync::event::{SyncAction, SyncEntity, SyncEvent};
    use tokio::sync::mpsc::UnboundedReceiver;

    fn empty_registry() -> SyncRegistry {
        SyncRegistry {
            inner: Mutex::new(RegistryInner {
                clients: HashMap::new(),
                by_user: HashMap::new(),
            }),
        }
    }

    fn fake_user(id: Uuid, is_admin: bool, permissions: Vec<String>) -> User {
        User {
            id,
            username: "t".into(),
            email: "t@example.com".into(),
            email_verified: true,
            password_hash: None,
            display_name: None,
            avatar_url: None,
            is_active: true,
            is_admin,
            permissions,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_login_at: None,
            password_changed_at: None,
        }
    }

    type Rx = UnboundedReceiver<Result<Event, axum::Error>>;

    /// Build a ClientConn + its receiver. `groups` defaults to empty (most
    /// tests drive permissions via the user's direct `permissions`).
    fn conn(user: User) -> (ClientConn, Rx) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let c = ClientConn {
            user_id: user.id,
            is_admin: user.is_admin,
            user,
            groups: Vec::new(),
            sender: tx,
        };
        (c, rx)
    }

    fn ev() -> SyncEvent {
        SyncEvent {
            entity: SyncEntity::Project,
            action: SyncAction::Create,
            id: Uuid::new_v4(),
        }
    }

    /// A delivered message is `Ok(_)` on try_recv; a non-delivery is Empty.
    fn got(rx: &mut Rx) -> bool {
        rx.try_recv().is_ok()
    }

    #[test]
    fn owner_audience_isolates_users() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let (ca, mut rxa) = conn(fake_user(a, false, vec![]));
        let (cb, mut rxb) = conn(fake_user(b, false, vec![]));
        let (ida, idb) = (Uuid::new_v4(), Uuid::new_v4());
        reg.register(ida, ca).unwrap();
        reg.register(idb, cb).unwrap();

        reg.deliver(Audience::Owner(a), ev(), None);

        assert!(got(&mut rxa), "owner A must receive their own event");
        assert!(!got(&mut rxb), "user B must NOT receive user A's event");
    }

    #[test]
    fn origin_connection_is_skipped_but_other_tabs_are_not() {
        let reg = empty_registry();
        let a = Uuid::new_v4();
        let (c1, mut rx1) = conn(fake_user(a, false, vec![]));
        let (c2, mut rx2) = conn(fake_user(a, false, vec![]));
        let (id1, id2) = (Uuid::new_v4(), Uuid::new_v4());
        reg.register(id1, c1).unwrap();
        reg.register(id2, c2).unwrap();

        // Mutation originated on conn1.
        reg.deliver(Audience::Owner(a), ev(), Some(id1));

        assert!(!got(&mut rx1), "originating tab must be skipped (self-echo)");
        assert!(got(&mut rx2), "the user's OTHER tab must still update");
    }

    #[test]
    fn permission_audience_excludes_non_holders_includes_holders_and_admins() {
        let reg = empty_registry();
        let (c_admin, mut rx_admin) = conn(fake_user(Uuid::new_v4(), true, vec![]));
        let (c_holder, mut rx_holder) =
            conn(fake_user(Uuid::new_v4(), false, vec!["x::read".into()]));
        let (c_other, mut rx_other) = conn(fake_user(Uuid::new_v4(), false, vec![]));
        reg.register(Uuid::new_v4(), c_admin).unwrap();
        reg.register(Uuid::new_v4(), c_holder).unwrap();
        reg.register(Uuid::new_v4(), c_other).unwrap();

        reg.deliver(Audience::Permission("x::read"), ev(), None);

        assert!(got(&mut rx_admin), "admin (wildcard) must receive");
        assert!(got(&mut rx_holder), "perm holder must receive");
        assert!(!got(&mut rx_other), "non-holder must NOT receive");
    }

    #[test]
    fn everyone_audience_reaches_all_connections() {
        let reg = empty_registry();
        let (c1, mut rx1) = conn(fake_user(Uuid::new_v4(), false, vec![]));
        let (c2, mut rx2) = conn(fake_user(Uuid::new_v4(), false, vec![]));
        reg.register(Uuid::new_v4(), c1).unwrap();
        reg.register(Uuid::new_v4(), c2).unwrap();

        reg.deliver(Audience::Everyone, ev(), None);

        assert!(got(&mut rx1));
        assert!(got(&mut rx2));
    }

    #[test]
    fn per_user_cap_rejects_excess_connections() {
        let reg = empty_registry();
        let uid = Uuid::new_v4();
        for _ in 0..PER_USER_MAX_CONNECTIONS {
            let (c, _rx) = conn(fake_user(uid, false, vec![]));
            reg.register(Uuid::new_v4(), c).unwrap();
        }
        let (overflow, _rx) = conn(fake_user(uid, false, vec![]));
        assert!(
            reg.register(Uuid::new_v4(), overflow).is_err(),
            "the (cap+1)th connection for one user must be refused (429)"
        );
    }

    #[test]
    fn unregister_cleans_up_indexes() {
        let reg = empty_registry();
        let uid = Uuid::new_v4();
        let (c, _rx) = conn(fake_user(uid, false, vec![]));
        let id = Uuid::new_v4();
        reg.register(id, c).unwrap();
        assert_eq!(reg.connection_count(), 1);

        reg.unregister(id);
        assert_eq!(reg.connection_count(), 0);
        // The per-user index entry is removed when its last conn leaves, so a
        // later Owner delivery is a no-op (and doesn't panic).
        reg.deliver(Audience::Owner(uid), ev(), None);
    }

    #[test]
    fn refresh_updates_permission_snapshot() {
        let reg = empty_registry();
        let uid = Uuid::new_v4();
        let (c, mut rx) = conn(fake_user(uid, false, vec![]));
        let id = Uuid::new_v4();
        reg.register(id, c).unwrap();

        // Before refresh: no perm → excluded from a Permission audience.
        reg.deliver(Audience::Permission("x::read"), ev(), None);
        assert!(!got(&mut rx));

        // After a re-check grants the perm, the same connection is included.
        reg.refresh(id, fake_user(uid, false, vec!["x::read".into()]), Vec::new());
        reg.deliver(Audience::Permission("x::read"), ev(), None);
        assert!(got(&mut rx));
    }
}

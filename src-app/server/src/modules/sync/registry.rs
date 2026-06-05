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

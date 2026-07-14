//! SSE subscribe endpoint + router for realtime cross-device sync.

use std::sync::Arc;
use std::time::Duration;

use aide::axum::{ApiRouter, routing::get_with};
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Extension,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use uuid::Uuid;

use crate::common::ApiResult;
use crate::core::Repos;
use crate::modules::auth::jwt::JwtService;
use crate::modules::permissions::{
    checker::check_permission_union, extractors::RequirePermissions, with_permission,
};
use crate::modules::user::permissions::ProfileRead;

use super::event::{SyncConnectedData, SyncSseEvent};
use super::registry::{ClientConn, SyncConnPrincipal, SYNC_CHANNEL_CAPACITY, registry};

/// Re-resolve `is_active` + group permissions this often while a stream
/// is open, so a deactivation / permission change is picked up within the
/// window (bounded staleness) without a per-event DB hit.
const RECHECK_INTERVAL: Duration = Duration::from_secs(60);

/// Resolve the re-check interval. A debug-only env override
/// (`SYNC_RECHECK_TICK_MS`, compiled out of release builds via
/// `cfg!(debug_assertions)` — same testability-seam pattern as
/// `LLM_RUNTIME_REAPER_TICK_MS`) lets the mid-stream deactivation /
/// permission-revocation integration test observe the teardown in
/// milliseconds instead of the 60s production cadence. Ignored in release.
fn recheck_interval() -> Duration {
    #[cfg(debug_assertions)]
    if let Ok(ms) = std::env::var("SYNC_RECHECK_TICK_MS") {
        if let Ok(n) = ms.parse::<u64>() {
            if n > 0 {
                return Duration::from_millis(n);
            }
        }
    }
    RECHECK_INTERVAL
}

/// GET /api/sync/subscribe — per-user realtime change stream.
#[debug_handler]
pub async fn subscribe_sync(
    auth: RequirePermissions<(ProfileRead,)>,
    Extension(jwt): Extension<Arc<JwtService>>,
    headers: HeaderMap,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let user = auth.user.clone();
    let user_id = user.id;
    let groups = auth.groups.clone();

    // Bound the stream by the access token's expiry: when it lapses the
    // client reconnects with a fresh token, which re-runs the auth
    // extractor (re-checking is_active + permissions from scratch).
    let exp_unix = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| JwtService::extract_token_from_header(h).ok())
        .and_then(|t| jwt.validate_access_token(t).ok())
        .map(|c| c.exp);

    let conn_id = Uuid::new_v4();
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(SYNC_CHANNEL_CAPACITY);

    registry()
        .register(
            conn_id,
            ClientConn {
                user_id,
                principal: SyncConnPrincipal { user, groups },
                sender: tx.clone(),
            },
        )
        .map_err(|e| e.to_api_error())?;

    // Handshake: hand the client its connection id for echo suppression.
    let _ = tx.try_send(Ok(SyncSseEvent::Connected(SyncConnectedData {
        connection_id: conn_id,
    })
    .into()));

    // Deadline = token exp (fallback far future if somehow absent).
    let secs_remaining = exp_unix
        .map(|exp| (exp - chrono::Utc::now().timestamp()).max(0) as u64)
        .unwrap_or(24 * 60 * 60);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs_remaining);

    let stream = async_stream::stream! {
        // Unregister on ANY stream termination — client disconnect, exp,
        // or deactivation. Drop runs even when the client vanishes
        // mid-await (axum drops the stream future on disconnect).
        let _guard = ConnGuard(conn_id);

        let mut recheck =
            tokio::time::interval_at(tokio::time::Instant::now() + recheck_interval(), recheck_interval());
        // After a stall, don't fire missed ticks back-to-back (each does DB work).
        recheck.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let sleep = tokio::time::sleep_until(deadline);
        tokio::pin!(sleep);

        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    match maybe {
                        Some(ev) => yield ev,
                        None => break,
                    }
                }
                _ = recheck.tick() => {
                    // Re-resolve is_active + permissions. Tear the stream
                    // down if the account was deactivated/removed OR lost the
                    // baseline subscribe permission; otherwise refresh the
                    // snapshot used to route Permission-audience events (so a
                    // user who loses an admin perm stops receiving its events).
                    match Repos.user.get_by_id(user_id).await {
                        Ok(Some(u)) if u.is_active => {
                            let g = if u.is_admin {
                                Vec::new()
                            } else {
                                Repos.user.get_user_groups(user_id).await.unwrap_or_default()
                            };
                            // Baseline gate: a user who no longer holds
                            // profile::read is no longer entitled to the
                            // stream (matches the subscribe-time gate).
                            if !u.is_admin && !check_permission_union(&u, &g, "profile::read") {
                                break;
                            }
                            registry().refresh(conn_id, SyncConnPrincipal { user: u, groups: g });
                        }
                        // Account removed or deactivated → tear the stream down.
                        Ok(_) => break,
                        // Transient DB error → keep the stream; retry next tick
                        // (don't drop an otherwise-valid connection on a blip).
                        Err(_) => {}
                    }
                }
                _ = &mut sleep => break,
            }
        }
    };

    Ok((
        StatusCode::OK,
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

/// Unregisters its connection from the registry on drop, covering every
/// way a stream can end (including a client that simply disappears).
struct ConnGuard(Uuid);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        registry().unregister(self.0);
    }
}

pub fn subscribe_sync_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("Sync.subscribe")
        .tag("Sync")
        .summary("Subscribe to realtime cross-device sync events via SSE")
        .description(
            "Server-Sent Events stream of `{entity, action, id}` change \
             notifications scoped to the authenticated user (and the \
             permission-/group-visibility rules per entity). The client \
             refetches the changed entity via its normal REST endpoint, so \
             no row data crosses this channel. The first frame (`connected`) \
             carries a connection id to echo back as `X-Sync-Connection-Id` \
             on mutations for self-echo suppression. The stream closes when \
             the access token expires or the account loses access.",
        )
        .response::<200, Json<SyncSseEvent>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<429, (), _>(|res| res.description("Too many open sync connections"))
}

pub fn sync_router() -> ApiRouter {
    ApiRouter::new().api_route("/sync/subscribe", get_with(subscribe_sync, subscribe_sync_docs))
}

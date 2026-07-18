//! SSE subscribe router for realtime cross-device sync.
//!
//! Chunk sdk-surfaces moved the `GET /api/sync/subscribe` SSE handler (the
//! `tokio::select!` over {channel recv, 60s re-check, JWT-`exp` deadline}) into
//! `ziee_framework::sync::sync_routes`, generic over the app's
//! [`IdentityResolver`](ziee_framework::permissions::IdentityResolver) + its
//! [`SyncSurface`](ziee_framework::sync::SyncSurface). ziee is a thin consumer:
//! it mounts that router with its concrete `ZieeIdentityResolver` (the auth
//! mechanism) + `SyncEntity` (the wire/registry surface, impl'd in `event.rs`),
//! so the endpoint's path / response schema / re-check / deadline behavior are
//! byte-identical to the former handler.

use aide::axum::ApiRouter;

use ziee_framework::sync::sync_routes;

use super::event::SyncEntity;
use crate::modules::permissions::extractors::ZieeIdentityResolver;

pub fn sync_router() -> ApiRouter {
    sync_routes::<ZieeIdentityResolver, SyncEntity>()
}

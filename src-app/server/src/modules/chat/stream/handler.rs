//! SSE subscribe endpoint + subscription control for the chat-token stream.

use std::sync::Arc;
use std::time::Duration;

use aide::axum::{ApiRouter, routing::get_with, routing::put_with};
use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Extension,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::stream::Stream;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::auth::jwt::JwtService;
use crate::modules::auth::jwt_extractor::verify_token_version;
use crate::modules::permissions::{checker::check_permission_union, extractors::RequirePermissions, with_permission};
use crate::modules::user::permissions::ProfileRead;

use super::event::{ChatStreamSseEvent, connected_event};
use super::registry::{CHAT_STREAM_CHANNEL_CAPACITY, ChatConn, registry};

/// Header the client echoes (from the `connected` handshake) so a subscription
/// PUT targets the right stream connection.
const CHAT_STREAM_CONNECTION_HEADER: &str = "X-Chat-Stream-Connection-Id";

/// Re-resolve `is_active` this often while a stream is open, tearing it down on
/// deactivation / loss of the baseline permission within the window.
const RECHECK_INTERVAL: Duration = Duration::from_secs(60);

/// GET /api/chat/stream — per-user live assistant-token stream.
#[debug_handler]
pub async fn subscribe_chat_stream(
    auth: RequirePermissions<(ProfileRead,)>,
    Extension(jwt): Extension<Arc<JwtService>>,
    headers: HeaderMap,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, axum::Error>>>> {
    let user_id = auth.user.id;

    // Bound the stream by the access token's expiry (client reconnects with a
    // fresh token, which re-runs the auth extractor).
    // `ver` is the token's access-token revocation epoch, kept so the periodic
    // re-check below can end an ALREADY-OPEN stream on logout — the subscribe
    // gate checks it once, but this stream then lives for the token's whole TTL
    // while delivering live assistant content.
    let claims = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| JwtService::extract_token_from_header(h).ok())
        .and_then(|t| jwt.validate_access_token(t).ok());
    let exp_unix = claims.as_ref().map(|c| c.exp);
    let token_ver = claims.as_ref().and_then(|c| c.ver);

    let sse = open_chat_stream(user_id, exp_unix, token_ver).map_err(|e| e.to_api_error())?;
    Ok((StatusCode::OK, sse))
}

/// Register a chat-token connection and build its SSE stream.
///
/// Extracted from `subscribe_chat_stream` so the slot-lifecycle contract is
/// TESTABLE: this fn needs no `RequirePermissions` extractor, no HTTP request and
/// no database, so a unit test can call it, drop the returned `Sse` **without
/// ever polling it**, and assert the connection's slot was released. That is the
/// termination the previous guard placement could not see.
///
/// It is emphatically NOT a hypothetical path. axum routes `HEAD` to the `GET`
/// handler (`method_routing.rs`, `call!(req, HEAD, get)`) and then REPLACES the
/// response body with `Body::empty()` inside `RouteFuture::poll`
/// (`routing/route.rs`) — synchronously, before the response ever reaches hyper.
/// The SSE body is therefore dropped WITHOUT EVER BEING POLLED, so any `HEAD
/// /api/chat/stream` (uptime monitor, reverse proxy, link previewer, scanner)
/// used to consume a per-user slot permanently.
///
/// `head_requests_do_not_leak_chat_stream_slots` covers that end-to-end, but
/// note it does not ISOLATE the guard placement: with the guard reverted and the
/// sweep kept, the (cap+1)th HEAD would reclaim the leaked entries and the test
/// would still pass. The test below is what isolates it — it asserts the slot
/// count after EVERY drop, staying below the cap so the sweep never runs.
fn open_chat_stream(
    user_id: Uuid,
    exp_unix: Option<i64>,
    token_ver: Option<i32>,
) -> Result<Sse<impl Stream<Item = Result<Event, axum::Error>>>, AppError> {
    let conn_id = Uuid::new_v4();
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(CHAT_STREAM_CHANNEL_CAPACITY);

    registry()
        .register(
            conn_id,
            ChatConn {
                user_id,
                active_conversation: None,
                sender: tx.clone(),
            },
        )?;

    // The slot is now OWNED by this guard — constructed eagerly the instant
    // registration succeeds (and only on success: the 429 path inserted
    // nothing, so nothing may claim ownership). It is moved into the stream
    // below.
    //
    // It CANNOT be declared inside the `stream!` body: that body is a generator
    // that does not run until the stream's FIRST poll, so a client that goes
    // away before the response body is ever polled would leave a registration
    // whose guard was never constructed — the slot would be held for the life
    // of the process, and every reconnect would burn another until the account
    // is permanently 429'd. Captured by the `async move` generator instead, the
    // guard lives in the future's state and is dropped when the future is
    // dropped, polled or not.
    let guard = ConnGuard(conn_id);

    // Handshake: hand the client its connection id to echo on the subscription PUT.
    let _ = tx.try_send(Ok(connected_event(conn_id)));

    let secs_remaining = exp_unix
        .map(|exp| (exp - chrono::Utc::now().timestamp()).max(0) as u64)
        .unwrap_or(24 * 60 * 60);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs_remaining);

    let stream = async_stream::stream! {
        // Unregister on ANY termination — disconnect, exp, or deactivation,
        // INCLUDING a stream dropped before it was ever polled (the guard was
        // constructed at registration and is merely MOVED in here).
        let _guard = guard;

        let mut recheck = tokio::time::interval_at(
            tokio::time::Instant::now() + RECHECK_INTERVAL,
            RECHECK_INTERVAL,
        );
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
                    // Tear the stream down if the account was deactivated/removed,
                    // LOGGED OUT, or lost the baseline subscribe permission.
                    match Repos.user.get_by_id_with_token_version(user_id).await {
                        Ok(Some((u, token_version))) if u.is_active => {
                            // A logout must end an already-open stream too: a
                            // holder of a revoked token doesn't run our client
                            // code, so the Session fan-out is not a boundary for
                            // them. Free: the query above already loads the row.
                            if verify_token_version(token_ver, token_version).is_err() {
                                break;
                            }
                            let g = if u.is_admin {
                                Vec::new()
                            } else {
                                Repos.user.get_user_groups(user_id).await.unwrap_or_default()
                            };
                            if !u.is_admin && !check_permission_union(&u, &g, "profile::read") {
                                break;
                            }
                        }
                        Ok(_) => break,
                        Err(_) => {}
                    }
                }
                _ = &mut sleep => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Unregisters its connection on drop, covering every way a stream can end.
struct ConnGuard(Uuid);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        registry().unregister(self.0);
    }
}

/// Body of `PUT /api/chat/stream/subscription`: the conversation whose live
/// tokens this connection wants (or `null` to receive nothing).
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSubscriptionRequest {
    pub conversation_id: Option<Uuid>,
}

/// PUT /api/chat/stream/subscription — scope a connection to one conversation.
#[debug_handler]
pub async fn set_chat_stream_subscription(
    auth: RequirePermissions<(ProfileRead,)>,
    headers: HeaderMap,
    Json(request): Json<SetSubscriptionRequest>,
) -> ApiResult<StatusCode> {
    let conn_id = headers
        .get(CHAT_STREAM_CONNECTION_HEADER)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| {
            AppError::bad_request(
                "MISSING_CONNECTION_ID",
                "X-Chat-Stream-Connection-Id header required",
            )
        })?;

    // Defense in depth: don't let a connection subscribe to a conversation it
    // doesn't own (delivery is already owner-keyed, but verify ownership too).
    if let Some(conversation_id) = request.conversation_id {
        Repos.chat
            .core
            .get_conversation(conversation_id, auth.user.id)
            .await?
            .ok_or_else(|| AppError::not_found("Conversation"))?;
    }

    registry().set_subscription(conn_id, request.conversation_id);

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn subscribe_chat_stream_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("ChatStream.subscribe")
        .tag("Chat")
        .summary("Subscribe to live assistant-token frames via SSE")
        .description(
            "Per-user Server-Sent Events stream of live chat generation frames \
             (`started`/`content`/`complete`/`error`), each tagged with its \
             `conversationId`. The first frame (`connected`) carries a \
             connection id to echo as `X-Chat-Stream-Connection-Id` on \
             `PUT /api/chat/stream/subscription`, which scopes delivery to the \
             one conversation the device is viewing (and replays the \
             reply-so-far if it is mid-generation).",
        )
        .response::<200, Json<ChatStreamSseEvent>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<429, (), _>(|res| res.description("Too many open chat-stream connections"))
}

pub fn set_chat_stream_subscription_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProfileRead,)>(op)
        .id("ChatStream.setSubscription")
        .tag("Chat")
        .summary("Scope a chat-stream connection to one conversation")
        .description(
            "Sets which conversation's live tokens the calling chat-stream \
             connection (identified by the `X-Chat-Stream-Connection-Id` \
             header) receives. `conversationId: null` unsubscribes. If the \
             target conversation is mid-generation its reply-so-far is replayed.",
        )
        .response_with::<204, (), _>(|res| res.description("Subscription updated"))
        .response_with::<400, (), _>(|res| res.description("Missing connection id"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

pub fn chat_stream_router() -> ApiRouter {
    ApiRouter::new()
        .api_route(
            "/chat/stream",
            get_with(subscribe_chat_stream, subscribe_chat_stream_docs),
        )
        .api_route(
            "/chat/stream/subscription",
            put_with(set_chat_stream_subscription, set_chat_stream_subscription_docs),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::registry::ChatStreamLimits;
    use uuid::Uuid;

    /// BEHAVIOURAL proof of the slot-lifecycle contract on THIS handler: a
    /// stream dropped **before it is ever polled** must still release its
    /// connection slot.
    ///
    /// This is the one termination path the old guard placement could not see.
    /// `ConnGuard` used to be a local of the `async_stream::stream!` generator
    /// body, and that body does not run until the stream's first poll — so a
    /// response abandoned before that poll left a registration whose guard was
    /// never constructed, holding its slot for the life of the process. Every
    /// reconnect burned another until the account was permanently 429'd.
    ///
    /// An abandoned GET does NOT reach this path (measured: 400 concurrent
    /// abandoned raw sockets leak 0 slots, because hyper polls the body while
    /// writing it) — but a `HEAD` does, because axum swaps the body for
    /// `Body::empty()` before it is ever polled.
    ///
    /// **This test is the one that ISOLATES the guard placement.** It stays
    /// BELOW the per-user cap between assertions, so `register`'s sweep never
    /// runs and cannot mask a reverted guard: the count must be 0 after every
    /// single drop. (The end-to-end `head_requests_do_not_leak_chat_stream_slots`
    /// is satisfied by the sweep alone, so it proves the user is not locked out
    /// — not where the release came from.) Driven through the
    /// `open_chat_stream` seam: no extractor, no HTTP, no DB — the only DB
    /// access is in the periodic re-check arm, which lives inside the generator
    /// and never runs here.
    #[tokio::test]
    async fn an_unpolled_stream_still_releases_its_slot() {
        let uid = Uuid::new_v4();
        let exp = chrono::Utc::now().timestamp() + 3600;
        let reg = registry();

        // More than the default per-user cap, so the leak's downstream effect
        // (registrations start being refused) would also show up. NOTE the
        // assertion that actually discriminates is the per-iteration slot count
        // below: `register` sweeps closed channels at the cap, and dropping an
        // unpolled stream closes its channel, so a guard-only regression is
        // caught by the count long before any 429.
        let n = ChatStreamLimits::default().per_user_max_connections + 8;
        for i in 0..n {
            let sse = open_chat_stream(uid, Some(exp), None)
                .unwrap_or_else(|e| panic!("subscribe #{i} must register: {e:?}"));
            // The client vanished: drop the stream WITHOUT ever polling it.
            drop(sse);
            assert_eq!(
                reg.connection_count_for_user(uid),
                0,
                "stream #{i} was dropped unpolled and MUST have released its slot"
            );
        }
        assert_eq!(reg.connection_count_for_user(uid), 0);
    }

    /// The complementary direction, so the contract is not satisfied by "always
    /// release": an OPEN stream holds exactly one slot for as long as it is
    /// held, and gives it back on drop. (The post-first-poll and exp-deadline
    /// exits are covered on the structurally identical sync handler by
    /// `ziee-framework`'s `every_stream_exit_path_releases_its_slot`; this pair
    /// covers the registration/​release edges on THIS handler.)
    #[tokio::test]
    async fn a_live_stream_keeps_its_slot_until_dropped() {
        let uid = Uuid::new_v4();
        let exp = chrono::Utc::now().timestamp() + 3600;
        let reg = registry();

        let sse = open_chat_stream(uid, Some(exp), None).expect("registers");
        assert_eq!(
            reg.connection_count_for_user(uid),
            1,
            "an open stream holds exactly one slot"
        );
        drop(sse);
        assert_eq!(
            reg.connection_count_for_user(uid),
            0,
            "dropping the stream releases it"
        );
    }
}

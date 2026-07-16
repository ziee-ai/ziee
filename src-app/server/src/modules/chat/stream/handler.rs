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
        )
        .map_err(|e| e.to_api_error())?;

    // Handshake: hand the client its connection id to echo on the subscription PUT.
    let _ = tx.try_send(Ok(connected_event(conn_id)));

    let secs_remaining = exp_unix
        .map(|exp| (exp - chrono::Utc::now().timestamp()).max(0) as u64)
        .unwrap_or(24 * 60 * 60);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(secs_remaining);

    let stream = async_stream::stream! {
        // Unregister on ANY termination — disconnect, exp, or deactivation.
        let _guard = ConnGuard(conn_id);

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

    Ok((
        StatusCode::OK,
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
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

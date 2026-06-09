// Send/stop handlers - fire-and-forget message send; the assistant reply
// streams over the per-user chat-token stream (`GET /api/chat/stream`), not
// this request's response.

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, extract::Path, http::StatusCode};
use schemars::JsonSchema;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            extension::{ExtensionRegistry, SendMessageRequest},
            permissions::*,
            services::StreamingService,
        },
        permissions::{extractors::RequirePermissions, with_permission},
        sync::SyncOrigin,
    },
    utils::cancellation::CANCELLATION_TRACKER,
};

/// Response to a fire-and-forget send: the persisted message ids. The assistant
/// reply arrives as live frames over `GET /api/chat/stream` (tokens), and the
/// finished turn lands via `sync:conversation` + refetch.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SendMessageResponse {
    /// None if an extension suppressed the user message (e.g. tool-approval resume).
    pub user_message_id: Option<Uuid>,
    pub assistant_message_id: Uuid,
}

/// POST /conversations/{id}/messages — send a user message and kick off the
/// assistant reply. Returns immediately with the message ids; tokens stream
/// over `GET /api/chat/stream`.
#[debug_handler]
pub async fn send_message(
    auth: RequirePermissions<(MessagesCreate,)>,
    origin: SyncOrigin,
    Extension(extension_registry): Extension<Arc<ExtensionRegistry>>,
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> ApiResult<Json<SendMessageResponse>> {
    // Verify conversation exists and user owns it
    let _conversation = Repos
        .chat
        .core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Validate model exists + is enabled
    let model = Repos
        .llm_model
        .get_by_id(request.model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;
    if !model.enabled {
        return Err(AppError::bad_request(
            "MODEL_DISABLED",
            "This model is currently disabled and cannot be used.",
        )
        .into());
    }

    // Verify the user can access this model's provider via their groups
    let has_access = Repos
        .user_group_llm_provider
        .user_has_access_to_provider(auth.user.id, model.provider_id)
        .await
        .map_err(AppError::from)?;
    if !has_access {
        return Err(AppError::forbidden(
            "ACCESS_DENIED",
            "You do not have access to this model. Contact your administrator to request access.",
        )
        .into());
    }

    // Validate branch belongs to this conversation
    let branch = Repos
        .chat
        .core
        .get_branch(request.branch_id)
        .await?
        .ok_or_else(|| AppError::not_found("Branch"))?;
    if branch.conversation_id != conversation_id {
        return Err(AppError::bad_request(
            "INVALID_BRANCH",
            "Branch does not belong to this conversation",
        )
        .into());
    }

    // Branch creation for the edit/regenerate flow
    let branch_id = if let Some(message_id) = request.create_branch_from_message_id {
        let new_branch = Repos
            .chat
            .core
            .create_branch_from_message(
                conversation_id,
                request.branch_id,
                message_id,
                &request.fork_level,
            )
            .await?;
        new_branch.id
    } else {
        request.branch_id
    };

    // Update conversation state with the active branch + model
    Repos
        .chat
        .core
        .update_conversation_state(conversation_id, auth.user.id, request.model_id, Some(branch_id))
        .await?;

    // Persist the user + assistant rows, return ids, drive generation detached.
    let streaming_service =
        StreamingService::new(Repos.pool().clone()).with_extensions(extension_registry);
    let (user_message_id, assistant_message_id) = streaming_service
        .start_generation(branch_id, conversation_id, auth.user.id, origin.0, request)
        .await?;

    Ok((
        StatusCode::OK,
        Json(SendMessageResponse {
            user_message_id,
            assistant_message_id,
        }),
    ))
}

pub fn send_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesCreate,)>(op)
        .id("Message.send")
        .tag("Chat")
        .summary("Send a message (fire-and-forget; reply streams over /api/chat/stream)")
        .description(
            "Sends a user message and starts the assistant reply, returning the \
             persisted `{userMessageId, assistantMessageId}` immediately. The \
             reply is delivered as live token frames over the per-user \
             `GET /api/chat/stream` (to which the device subscribes for this \
             conversation), and the finished turn is announced via a \
             `sync:conversation` notification so other surfaces refetch.",
        )
        .response::<200, Json<SendMessageResponse>>()
        .response::<404, ()>()
        .response::<401, ()>()
}

/// POST /conversations/{id}/messages/{assistant_message_id}/stop — cancel an
/// in-flight generation. The detached task emits a `complete` (cancelled) frame.
#[debug_handler]
pub async fn stop_generation(
    auth: RequirePermissions<(MessagesCreate,)>,
    Path((_conversation_id, assistant_message_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    // Verify the user owns the conversation containing this message.
    Repos
        .chat
        .core
        .verify_message_ownership(assistant_message_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;

    // `cancel_download` returns false when nothing is in flight for this message
    // (already finished, or never started). Surface that as 409 so a client can
    // tell "stopped" from "nothing to stop" instead of a misleading success.
    if !CANCELLATION_TRACKER.cancel_download(assistant_message_id).await {
        return Err(AppError::new(
            StatusCode::CONFLICT,
            "NO_ACTIVE_GENERATION",
            "No in-flight generation to stop for this message",
        )
        .into());
    }

    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn stop_generation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesCreate,)>(op)
        .id("Message.stopGeneration")
        .tag("Chat")
        .summary("Stop an in-flight assistant generation")
        .description(
            "Cancels the detached generation for the given assistant message. \
             A `complete` frame with `finishReason: cancelled` is emitted on \
             the chat-token stream.",
        )
        .response_with::<204, (), _>(|res| res.description("Cancellation requested"))
        .response_with::<404, (), _>(|res| res.description("Message not found"))
        .response_with::<409, (), _>(|res| {
            res.description("No in-flight generation to stop")
        })
}

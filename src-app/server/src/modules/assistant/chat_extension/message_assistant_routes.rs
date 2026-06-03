//! `GET /api/messages/{id}/assistant` — fetch the assistant that
//! was selected when the given message was sent.
//!
//! Replaces the inline `messages.assistant_id` column that used to
//! be on chat's `messages` table (migration 75 dropped it). The
//! frontend assistant extension's edit-restore subscriber hits this
//! to set the originally-active assistant back when the user clicks
//! Edit on a past message.

use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use axum::{debug_handler, extract::Path, http::StatusCode, Json};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::permissions::ConversationsRead,
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct MessageAssistantResponse {
    /// `None` when the message exists (and the caller owns it) but
    /// was sent WITHOUT an assistant. `Some(uuid)` when an assistant
    /// was attributed at send-time.
    pub assistant_id: Option<Uuid>,
}

/// Handler — returns the assistant attribution for a message the
/// caller owns. 404 if the message doesn't exist OR the caller
/// doesn't own the conversation (conflated to defeat probing for
/// message ids).
#[debug_handler]
pub async fn get_message_assistant(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(message_id): Path<Uuid>,
) -> ApiResult<Json<MessageAssistantResponse>> {
    let assistant_id = crate::core::Repos
        .chat
        .assistant
        .get_message_assistant_for_user(message_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;
    Ok((
        StatusCode::OK,
        Json(MessageAssistantResponse { assistant_id }),
    ))
}

pub fn get_message_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Message.getAssistant")
        .tag("Chat")
        .summary("Get the assistant that was selected when a message was sent")
        .description(
            "Returns the assistant id that was attributed to the \
             given user message at send-time. Used to restore the \
             original assistant selection when editing a past \
             message. `assistant_id` is null when the message was \
             sent without an assistant. Returns 404 if the message \
             doesn't exist or the caller doesn't own the \
             conversation it belongs to.",
        )
        .response::<200, Json<MessageAssistantResponse>>()
        .response_with::<404, (), _>(|res| res.description("Message not found"))
}

pub fn message_assistant_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/messages/{id}/assistant",
        get_with(get_message_assistant, get_message_assistant_docs),
    )
}

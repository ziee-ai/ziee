//! `GET /api/conversations/{id}/summarization-mode` and
//! `PUT /api/conversations/{id}/summarization-mode` — read/write the
//! per-conversation summarization toggle. Mirrors
//! `memory_mode_routes.rs` from the memory chat extension.

use aide::axum::{ApiRouter, routing::get_with};
use aide::transform::TransformOperation;
use axum::{Json, debug_handler, extract::Path, http::StatusCode};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::permissions::{ConversationsEdit, ConversationsRead},
        permissions::{extractors::RequirePermissions, with_permission},
        summarization::models::{
            ConversationSummarizationModeResponse, UpdateConversationSummarizationModeRequest,
            is_valid_summarization_mode,
        },
    },
};

use super::repository::DEFAULT_SUMMARIZATION_MODE;

#[debug_handler]
pub async fn get_conversation_summarization_mode(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<ConversationSummarizationModeResponse>> {
    let summarization_mode = crate::core::Repos
        .chat
        .summarization
        .get_for_user(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;
    Ok((
        StatusCode::OK,
        Json(ConversationSummarizationModeResponse { summarization_mode }),
    ))
}

pub fn get_conversation_summarization_mode_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.getSummarizationMode")
        .tag("Summarization")
        .summary("Get the per-conversation summarization mode")
        .description(
            "Returns the conversation's `summarization_mode` override \
             (`inherit` / `on` / `off`). Defaults to `inherit` when \
             the conversation has no explicit override. Returns 404 \
             if the conversation doesn't exist or the caller doesn't \
             own it.",
        )
        .response::<200, Json<ConversationSummarizationModeResponse>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

#[debug_handler]
pub async fn put_conversation_summarization_mode(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path(conversation_id): Path<Uuid>,
    Json(req): Json<UpdateConversationSummarizationModeRequest>,
) -> ApiResult<Json<ConversationSummarizationModeResponse>> {
    if !is_valid_summarization_mode(&req.summarization_mode) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!(
                "Invalid summarization_mode '{}'. Must be 'inherit', 'on', or 'off'.",
                req.summarization_mode
            ),
        )
        .into());
    }
    // Ownership check via get_for_user — 404 to defeat probing.
    let repo = &crate::core::Repos.chat.summarization;
    if repo
        .get_for_user(conversation_id, auth.user.id)
        .await?
        .is_none()
    {
        return Err(AppError::not_found("Conversation").into());
    }
    repo.set_conversation_summarization_mode(conversation_id, &req.summarization_mode)
        .await?;
    let summarization_mode = if req.summarization_mode == DEFAULT_SUMMARIZATION_MODE {
        DEFAULT_SUMMARIZATION_MODE.to_string()
    } else {
        req.summarization_mode
    };
    Ok((
        StatusCode::OK,
        Json(ConversationSummarizationModeResponse { summarization_mode }),
    ))
}

pub fn put_conversation_summarization_mode_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.setSummarizationMode")
        .tag("Summarization")
        .summary("Set the per-conversation summarization mode")
        .description(
            "Updates the conversation's `summarization_mode` override \
             (`inherit` / `on` / `off`). Setting `inherit` removes any \
             stored override. Returns 400 on invalid value, 404 if \
             the conversation doesn't exist or the caller doesn't own it.",
        )
        .response::<200, Json<ConversationSummarizationModeResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid summarization_mode"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

pub fn summarization_mode_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/conversations/{id}/summarization-mode",
        get_with(
            get_conversation_summarization_mode,
            get_conversation_summarization_mode_docs,
        )
        .put_with(
            put_conversation_summarization_mode,
            put_conversation_summarization_mode_docs,
        ),
    )
}

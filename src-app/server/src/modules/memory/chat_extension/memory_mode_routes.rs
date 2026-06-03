//! `GET /api/conversations/{id}/memory-mode` and
//! `PUT /api/conversations/{id}/memory-mode` — read/write the
//! per-conversation memory toggle that used to live as
//! `conversations.memory_mode` (migration 76 dropped the column).
//!
//! Replaces chat's prior `PUT /api/conversations/{id}` branch that
//! validated `memory_mode` inline — chat no longer knows the
//! vocabulary (`'inherit'`/`'on'`/`'off'`); the memory bridge owns it.

use aide::axum::{routing::get_with, ApiRouter};
use aide::transform::TransformOperation;
use axum::{debug_handler, extract::Path, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::permissions::{ConversationsEdit, ConversationsRead},
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

use super::repository::DEFAULT_MEMORY_MODE;

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct ConversationMemoryModeResponse {
    /// One of `"inherit"`, `"on"`, `"off"`. Defaults to `"inherit"`
    /// when the conversation has no explicit override row.
    pub memory_mode: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateConversationMemoryModeRequest {
    /// One of `"inherit"`, `"on"`, `"off"`. Setting `"inherit"`
    /// deletes any stored override (row absence == inherit).
    pub memory_mode: String,
}

fn is_valid_memory_mode(mode: &str) -> bool {
    matches!(mode, "inherit" | "on" | "off")
}

#[debug_handler]
pub async fn get_conversation_memory_mode(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<ConversationMemoryModeResponse>> {
    let memory_mode = crate::core::Repos
        .chat
        .memory
        .get_for_user(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;
    Ok((StatusCode::OK, Json(ConversationMemoryModeResponse { memory_mode })))
}

pub fn get_conversation_memory_mode_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Conversation.getMemoryMode")
        .tag("Memory")
        .summary("Get the per-conversation memory mode")
        .description(
            "Returns the conversation's `memory_mode` override \
             (`inherit` / `on` / `off`). Defaults to `inherit` when \
             the conversation has no explicit override. Returns 404 \
             if the conversation doesn't exist or the caller doesn't \
             own it.",
        )
        .response::<200, Json<ConversationMemoryModeResponse>>()
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

#[debug_handler]
pub async fn put_conversation_memory_mode(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path(conversation_id): Path<Uuid>,
    Json(req): Json<UpdateConversationMemoryModeRequest>,
) -> ApiResult<Json<ConversationMemoryModeResponse>> {
    if !is_valid_memory_mode(&req.memory_mode) {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!(
                "Invalid memory_mode '{}'. Must be 'inherit', 'on', or 'off'.",
                req.memory_mode
            ),
        )
        .into());
    }
    // Ownership check via get_for_user — returns None when the user
    // doesn't own the conversation; conflate to 404 to defeat probing.
    let repo = &crate::core::Repos.chat.memory;
    if repo.get_for_user(conversation_id, auth.user.id).await?.is_none() {
        return Err(AppError::not_found("Conversation").into());
    }
    repo.set_conversation_memory_mode(conversation_id, &req.memory_mode)
        .await?;
    // Echo back the persisted value (always the request value after a
    // successful upsert, but the round-trip lets the caller skip a
    // follow-up GET).
    let memory_mode = if req.memory_mode == DEFAULT_MEMORY_MODE {
        DEFAULT_MEMORY_MODE.to_string()
    } else {
        req.memory_mode
    };
    Ok((StatusCode::OK, Json(ConversationMemoryModeResponse { memory_mode })))
}

pub fn put_conversation_memory_mode_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("Conversation.setMemoryMode")
        .tag("Memory")
        .summary("Set the per-conversation memory mode")
        .description(
            "Updates the conversation's `memory_mode` override \
             (`inherit` / `on` / `off`). Setting `inherit` removes \
             any stored override. Returns 400 on invalid value, 404 \
             if the conversation doesn't exist or the caller doesn't \
             own it.",
        )
        .response::<200, Json<ConversationMemoryModeResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid memory_mode"))
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
}

pub fn memory_mode_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/conversations/{id}/memory-mode",
        get_with(get_conversation_memory_mode, get_conversation_memory_mode_docs)
            .put_with(put_conversation_memory_mode, put_conversation_memory_mode_docs),
    )
}

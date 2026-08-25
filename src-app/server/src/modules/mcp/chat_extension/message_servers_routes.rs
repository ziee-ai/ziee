//! `GET /api/messages/{id}/mcp-servers` — list the MCP servers that
//! were enabled when the given message was sent.
//!
//! Replaces the inline `messages.mcp_server_ids UUID[]` column that
//! used to be on chat's `messages` table (migration 74 dropped it).
//! Consumed by the frontend mcp extension's edit-restore subscriber
//! at `ui/src/modules/chat/extensions/mcp/extension.tsx` to set the
//! enabled-servers selection back to what was active when the
//! message was originally sent.

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
pub struct MessageMcpServersResponse {
    pub server_ids: Vec<Uuid>,
}

/// Handler — returns the server-id list for a message the caller owns.
/// 404 if the message doesn't exist OR the caller doesn't own the
/// conversation. Conflated to defeat probing for message ids.
#[debug_handler]
pub async fn get_message_mcp_servers(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(message_id): Path<Uuid>,
) -> ApiResult<Json<MessageMcpServersResponse>> {
    let server_ids = crate::core::Repos
        .chat
        .mcp
        .list_message_servers_for_user(message_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Message"))?;
    Ok((
        StatusCode::OK,
        Json(MessageMcpServersResponse { server_ids }),
    ))
}

pub fn get_message_mcp_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("Message.getMcpServers")
        .tag("Chat")
        .summary("List MCP servers that were enabled when a message was sent")
        .description(
            "Returns the snapshot of MCP server IDs that were enabled \
             at the time the given user message was sent. Used to \
             restore the original server selection when editing a \
             past message. Returns 404 if the message doesn't exist \
             or the caller doesn't own the conversation it belongs to.",
        )
        .response::<200, Json<MessageMcpServersResponse>>()
        .response_with::<404, (), _>(|res| res.description("Message not found"))
}

pub fn message_mcp_servers_router() -> ApiRouter {
    ApiRouter::new().api_route(
        "/messages/{id}/mcp-servers",
        get_with(get_message_mcp_servers, get_message_mcp_servers_docs),
    )
}

// Elicitation respond handler

use aide::transform::TransformOperation;
use axum::{
    debug_handler,
    extract::Path,
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        mcp::permissions::McpServersRead,
        permissions::{RequirePermissions, with_permission},
    },
};

use super::{models, registry};

/// Submit a user's response to an MCP elicitation request
#[debug_handler]
pub async fn respond_to_elicitation(
    auth: RequirePermissions<(McpServersRead,)>,
    Path(elicitation_id): Path<Uuid>,
    Json(request): Json<models::RespondToElicitationRequest>,
) -> ApiResult<Json<models::RespondToElicitationResponse>> {
    // Validate action field
    if !matches!(request.action.as_str(), "accept" | "decline" | "cancel") {
        return Err(AppError::bad_request(
            "INVALID_ACTION",
            "action must be one of: accept, decline, cancel",
        ).into());
    }

    // SECURITY: verify the responder owns this elicitation. The chat
    // extension layer binds the owning user_id via
    // registry::bind_owner when the elicitation/create notification
    // fires. Without this check, any holder of mcp_servers::read could
    // hijack any other user's elicitation by guessing/leaking the
    // random UUID. Fail-closed: if the binding hasn't happened yet
    // (race) we 403 rather than allow the response. Closes
    // 02-permissions F-04.
    match registry::owner_matches(elicitation_id, auth.user.id) {
        None => return Err(AppError::not_found("Elicitation request").into()),
        Some(false) => {
            return Err(AppError::forbidden(
                "FORBIDDEN",
                "Not authorized to respond to this elicitation",
            )
            .into());
        }
        Some(true) => {}
    }

    // The user's answer is a JSON OBJECT of field values. Our own frontend
    // always sends one (`resolveElicitation` is typed
    // `content?: Record<string, unknown>`), but this is a public REST route that
    // accepts any `Value`, and a JSON-ENCODED object here would reach the model
    // double-encoded through `ask_user_tool_result`'s `to_string`, and would be
    // POSTed back to an external MCP server as a non-conformant JSON-RPC result.
    // Same one rule as every model-facing argument: decode it, or refuse with a
    // message the caller can act on. Only `accept` carries content; `decline` /
    // `cancel` and an absent content are untouched.
    let content = match (request.action.as_str(), request.content.clone()) {
        ("accept", Some(v)) if !v.is_null() => Some(
            crate::common::tool_args::coerce_value(
                v,
                crate::common::tool_args::ArgShape::Object,
                "content",
                r#"{"name":"My project","confirm":true}"#,
            )
            .map_err(|e| AppError::bad_request("INVALID_CONTENT", e.into_message()))?,
        ),
        (_, other) => other,
    };

    let action = request.action.clone();

    let response = models::ElicitationResponse {
        action: request.action,
        content: content.clone(),
    };

    let (found, content_id_opt) = registry::respond(elicitation_id, response);
    if !found {
        return Err(AppError::not_found("Elicitation request").into());
    }

    // Persist the user's response to the DB row created when the elicitation started
    if let Some(content_id) = content_id_opt {
        let new_status = match action.as_str() {
            "accept" => "accepted",
            "decline" => "declined",
            _ => "cancelled",
        };
        let mut patch = serde_json::json!({ "status": new_status });
        if action == "accept"
            && let Some(values) = content {
                patch["response_content"] = values;
            }
        if let Err(err) = crate::core::Repos.chat.core
            .update_content_json(content_id, patch)
            .await
        {
            tracing::error!(
                error = %err,
                content_id = %content_id,
                "Failed to persist elicitation response to DB"
            );
        }
    }

    Ok((
        StatusCode::OK,
        Json(models::RespondToElicitationResponse { success: true }),
    ))
}

pub fn respond_to_elicitation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(McpServersRead,)>(op)
        .id("Mcp.respondToElicitation")
        .tag("Chat")
        .summary("Respond to an elicitation request")
        .description("Submit a user's response to an MCP server elicitation form. The elicitation_id is the per-elicitation UUID received via the mcpElicitationRequired SSE event.")
        .response::<200, Json<models::RespondToElicitationResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid action value"))
        .response_with::<404, (), _>(|res| res.description("Elicitation request not found or expired"))
}

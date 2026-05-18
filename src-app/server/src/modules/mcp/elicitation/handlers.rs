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
    _auth: RequirePermissions<(McpServersRead,)>,
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

    let action = request.action.clone();
    let content = request.content.clone();

    let response = models::ElicitationResponse {
        action: request.action,
        content: request.content,
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
        if action == "accept" {
            if let Some(values) = content {
                patch["response_content"] = values;
            }
        }
        let _ = crate::core::Repos.chat.core
            .update_content_json(content_id, patch)
            .await;
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

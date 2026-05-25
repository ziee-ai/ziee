// Streaming handlers - SSE streaming for AI chat responses

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::Path,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            extension::{ExtensionRegistry, SendMessageRequest},
            permissions::*,
            services::StreamingService,
            types::streaming::{SSEChatStreamEvent, SSEChatStreamErrorData, SSEChatStreamCompleteData},
        },
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

/// Send a message and stream the AI response using SSE
/// This is the main endpoint for interactive chat functionality
#[debug_handler]
pub async fn send_message(
    auth: RequirePermissions<(MessagesCreate,)>,

    Extension(extension_registry): Extension<Arc<ExtensionRegistry>>,
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<SendMessageRequest>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Verify conversation exists and user owns it
    let _conversation = Repos.chat.core
        .get_conversation(conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Validate model exists
    let model = Repos.llm_model
        .get_by_id(request.model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Verify model is enabled
    if !model.enabled {
        return Err(AppError::bad_request(
            "MODEL_DISABLED",
            "This model is currently disabled and cannot be used.",
        ).into());
    }

    // Verify user has access to this model's provider through their group assignments
    let has_access = Repos.llm_provider
        .user_has_access_to_provider(auth.user.id, model.provider_id)
        .await
        .map_err(AppError::from)?;

    if !has_access {
        return Err(AppError::forbidden(
            "ACCESS_DENIED",
            "You do not have access to this model. Contact your administrator to request access.",
        ).into());
    }

    // Validate branch exists and belongs to this conversation
    let branch = Repos.chat.core
        .get_branch(request.branch_id)
        .await?
        .ok_or_else(|| AppError::not_found("Branch"))?;

    if branch.conversation_id != conversation_id {
        return Err(AppError::bad_request("INVALID_BRANCH", "Branch does not belong to this conversation").into());
    }

    // Handle branch creation if requested (for edit/regenerate flow)
    let branch_id = if let Some(message_id) = request.create_branch_from_message_id {
        // Create new branch from the specified message
        let new_branch = Repos.chat.core
            .create_branch_from_message(conversation_id, request.branch_id, message_id, &request.fork_level)
            .await?;

        new_branch.id
    } else {
        // Use the specified branch directly
        request.branch_id
    };

    // Update conversation state with the active branch and model
    Repos.chat.core
        .update_conversation_state(conversation_id, auth.user.id, request.model_id, Some(branch_id))
        .await?;

    // Create streaming service with extensions
    // Provider is created inside send_message based on request.model_id
    let streaming_service =
        StreamingService::new(Repos.pool().clone()).with_extensions(extension_registry);

    // Send message and get both chunk stream and extension event receiver
    let (chunk_stream, ext_rx) = streaming_service
        .send_message(branch_id, conversation_id, auth.user.id, request)
        .await?;

    // Transform chunk stream to SSE Event stream
    let chunk_sse_stream = chunk_stream.map(move |result| {
        match result {
            Ok(chunk) => {
                // Check if this is a completion chunk (has finish_reason)
                let event = if let Some(finish_reason) = chunk.finish_reason {
                    // Convert to Complete event
                    SSEChatStreamEvent::Complete(SSEChatStreamCompleteData {
                        finish_reason,
                        usage: chunk.usage,
                    })
                } else {
                    // Wrap chunk in Content event
                    SSEChatStreamEvent::Content(chunk)
                };
                Ok(event.into())
            }
            Err(e) => {
                // Send error as SSE event
                let event = SSEChatStreamEvent::Error(SSEChatStreamErrorData {
                    message: e.to_string(),
                    code: Some("STREAM_ERROR".to_string()),
                });
                Ok(event.into())
            }
        }
    });

    // Convert extension event receiver to stream and merge with chunk stream
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio_stream::StreamExt;

    let ext_event_stream = UnboundedReceiverStream::new(ext_rx);

    // Merge both streams: chunk events and extension events
    let merged_stream = chunk_sse_stream.merge(ext_event_stream);

    // Create SSE response with keep-alive
    Ok((
        axum::http::StatusCode::OK,
        Sse::new(merged_stream).keep_alive(KeepAlive::default()),
    ))
}

pub fn send_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesCreate,)>(op)
        .id("Message.sendStream")
        .tag("Chat")
        .summary("Send message and stream AI response")
        .description(
            "Send a user message to a conversation and receive the AI response as a Server-Sent Events (SSE) stream. \
             Events include: 'content' (incremental content deltas), 'complete' (stream finished), 'error' (stream error), \
             and extension events like 'titleUpdated'.",
        )
        .response::<200, Json<SSEChatStreamEvent>>()
        .response::<404, ()>()
        .response::<401, ()>()
}

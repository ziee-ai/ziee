// Streaming handlers - SSE streaming for AI chat responses

use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::Path,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::{Stream, StreamExt};
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            extension::{ExtensionRegistry, SendMessageRequest},
            permissions::*,
            repository::conversations as conv_repo,
            services::StreamingService,
            types::{ChatStreamChunk, StreamError},
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
    let _conversation = conv_repo::get_conversation(Repos.pool(), conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Handle branch creation if requested (for edit/regenerate flow)
    let branch_id = if let Some(message_id) = request.create_branch_from_message_id {
        // Create new branch from the specified message
        let new_branch =
            crate::modules::chat::core::repository::messages::create_branch_from_message(
                Repos.pool(),
                conversation_id,
                request.branch_id,
                message_id,
            )
            .await?;

        new_branch.id
    } else {
        // Use the specified branch directly
        request.branch_id
    };

    // Update conversation state with the active branch and model
    conv_repo::update_conversation_state(
        Repos.pool(),
        conversation_id,
        auth.user.id,
        request.model_id,
        Some(branch_id),
    )
    .await?;

    // Create streaming service with extensions
    // Provider is created inside send_message based on request.model_id
    let streaming_service =
        StreamingService::new(Repos.pool().clone()).with_extensions(extension_registry);

    // Send message and get stream
    let chunk_stream = streaming_service
        .send_message(branch_id, conversation_id, auth.user.id, request)
        .await?;

    // Clone IDs for use in closure (they're Copy types)
    let conv_id = conversation_id;
    let br_id = branch_id;

    // Transform chunk stream to SSE Event stream
    let sse_stream = chunk_stream.map(move |result| {
        match result {
            Ok(chunk) => {
                // Serialize chunk to JSON
                match serde_json::to_string(&chunk) {
                    Ok(json) => Ok(Event::default().data(json)),
                    Err(e) => {
                        // If serialization fails, send error event
                        let error_chunk = ChatStreamChunk {
                            content: Vec::new(),
                            message_id: chunk.message_id,
                            conversation_id: chunk.conversation_id,
                            branch_id: chunk.branch_id,
                            finish_reason: None,
                            usage: None,
                            title: None,
                            error: Some(StreamError {
                                message: format!("Serialization error: {}", e),
                                code: Some("SERIALIZATION_ERROR".to_string()),
                            }),
                        };
                        match serde_json::to_string(&error_chunk) {
                            Ok(json) => Ok(Event::default().data(json)),
                            Err(_) => Ok(Event::default().data(r#"{"error":{"message":"Failed to serialize chunk","code":"SERIALIZATION_ERROR"}}"#)),
                        }
                    }
                }
            }
            Err(e) => {
                // Send error as SSE event
                let error_chunk = ChatStreamChunk {
                    content: Vec::new(),
                    message_id: None,
                    conversation_id: Some(conv_id),
                    branch_id: Some(br_id),
                    finish_reason: None,
                    usage: None,
                    title: None,
                    error: Some(StreamError {
                        message: e.to_string(),
                        code: Some("STREAM_ERROR".to_string()),
                    }),
                };
                match serde_json::to_string(&error_chunk) {
                    Ok(json) => Ok(Event::default().data(json)),
                    Err(_) => Ok(Event::default().data(r#"{"error":{"message":"Stream error","code":"STREAM_ERROR"}}"#)),
                }
            }
        }
    });

    // Create SSE response with keep-alive
    Ok((
        axum::http::StatusCode::OK,
        Sse::new(sse_stream).keep_alive(KeepAlive::default()),
    ))
}

pub fn send_message_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesCreate,)>(op)
        .id("Message.sendStream")
        .tag("Chat")
        .summary("Send message and stream AI response")
        .description(
            "Send a user message to a conversation and receive the AI response as a Server-Sent Events (SSE) stream. \
             Each event contains a ChatStreamChunk with incremental content deltas.",
        )
        .response_with::<200, (), _>(|res| {
            res.description(
                "SSE stream of ChatStreamChunk objects. Each event contains JSON with incremental content deltas. \
                 Example events:\n\
                 data: {\"content\":[{\"TextDelta\":{\"index\":0,\"delta\":\"Hello\"}}],\"message_id\":\"...\",\"finish_reason\":null}\n\
                 data: {\"content\":[{\"TextDelta\":{\"index\":0,\"delta\":\" world\"}}],\"message_id\":\"...\",\"finish_reason\":null}\n\
                 data: {\"content\":[],\"message_id\":\"...\",\"finish_reason\":\"stop\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}"
            )
        })
        .response_with::<404, (), _>(|res| res.description("Conversation not found"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

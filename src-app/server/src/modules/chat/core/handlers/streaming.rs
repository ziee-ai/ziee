// Streaming handlers - SSE streaming for AI chat responses

use aide::transform::TransformOperation;
use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    Extension, Json,
};
use futures_util::{Stream, StreamExt};
use sqlx::PgPool;
use std::convert::Infallible;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError},
    modules::{
        chat::core::{
            extension::{ExtensionRegistry, SendMessageRequest},
            models::{ChatStreamChunk, StreamError},
            permissions::*,
            repository::conversations as conv_repo,
            services::StreamingService,
        },
        llm_model::repository as model_repo,
        llm_provider::repository as provider_repo,
        permissions::{extractors::RequirePermissions, with_permission},
    },
};

use ai_providers::Provider;

/// Send a message and stream the AI response using SSE
/// This is the main endpoint for interactive chat functionality
pub async fn send_message(
    auth: RequirePermissions<(MessagesCreate,)>,
    State(pool): State<PgPool>,
    Extension(extension_registry): Extension<Arc<ExtensionRegistry>>,
    Path(conversation_id): Path<Uuid>,
    Json(mut request): Json<SendMessageRequest>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Verify conversation exists and user owns it
    let conversation = conv_repo::get_conversation(&pool, conversation_id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;

    // Get active branch
    let branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    // Create repository instances
    let model_repo = model_repo::LlmModelRepository::new(pool.clone());
    let provider_repo = provider_repo::LlmProviderRepository::new(pool.clone());

    // Get model information from database
    let model = model_repo
        .get_by_id(conversation.model_id)
        .await?
        .ok_or_else(|| AppError::not_found("Model"))?;

    // Get provider information from database
    let provider_info = provider_repo
        .get_by_id(model.provider_id)
        .await
        .map_err(AppError::database_error)?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    // Check if provider is enabled
    if !provider_info.enabled {
        return Err(AppError::bad_request(
            "PROVIDER_DISABLED",
            "The provider for this model is currently disabled",
        )
        .into());
    }

    // Map provider type to ai_providers format
    // anthropic and gemini map directly, everything else uses OpenAI-compatible API
    let provider_type = match provider_info.provider_type.as_str() {
        "anthropic" => "anthropic",
        "gemini" => "gemini",
        _ => "openai", // openai, groq, mistral, deepseek, custom, huggingface all use OpenAI-compatible API
    };

    // Get API key from provider
    let api_key = provider_info
        .api_key
        .as_deref()
        .unwrap_or("");

    // Get base URL (use provider's base_url or default based on type)
    let base_url = provider_info
        .base_url
        .as_deref()
        .unwrap_or_else(|| match provider_type {
            "anthropic" => "https://api.anthropic.com",
            "gemini" => "https://generativelanguage.googleapis.com",
            _ => "https://api.openai.com/v1",
        });

    // Create provider instance
    let provider = Arc::new(
        Provider::new(provider_type, api_key, base_url)
            .map_err(|e| AppError::internal_error(format!("Failed to create provider: {}", e)))?,
    );

    // Set model name in request for the streaming service
    request.model_name = Some(model.name.clone());

    // Create streaming service with extensions
    let streaming_service = StreamingService::new(pool.clone(), provider)
        .with_extensions(extension_registry);

    // Send message and get stream
    let chunk_stream = streaming_service
        .send_message(
            branch_id,
            conversation_id,
            auth.user.id,
            model.id,
            model.provider_id,
            request,
        )
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

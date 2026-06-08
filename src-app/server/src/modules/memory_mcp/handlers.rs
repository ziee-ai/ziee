//! JSON-RPC handler for the built-in memory MCP server.
//!
//! Memory tools are user-scoped (the MCP `remember/recall/forget`
//! operate on the caller's own `user_memories` rows). No
//! conversation_id required — user_id from the JWT is the only auth
//! input.
//!
//! Reuses the JSON-RPC types from `code_sandbox` so we don't duplicate
//! 100 lines of envelope/error scaffolding.

use axum::{
    Json, debug_handler,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use pgvector::HalfVector;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::memory::permissions::MemoryWrite;
use crate::modules::permissions::RequirePermissions;

// Shared between memory + memory_mcp handlers (see memory/models.rs).
use crate::modules::memory::models::MAX_MEMORY_CONTENT_LEN as MAX_CONTENT_LEN;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(MemoryWrite,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::parse_error(e.to_string()),
            );
        }
    };
    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return error_response(
                None,
                StatusCode::BAD_REQUEST,
                JsonRpcError::invalid_request(e.to_string()),
            );
        }
    };

    // Notifications carry no `id`, expect no response.
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let user_id = auth.user.id;
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "memory",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        ),
        "tools/list" => ok_response(id, super::tools::tool_list()),
        "ping" => ok_response(id, json!({})),
        "tools/call" => match dispatch_tool_call(user_id, conversation_id, &req.params).await {
            Ok(value) => ok_response(id, value),
            Err(e) => error_response(id, e.0, e.1),
        },
        _ => error_response(
            id,
            StatusCode::OK,
            JsonRpcError::method_not_found(&req.method),
        ),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> Response {
    (
        StatusCode::OK,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }),
    )
        .into_response()
}

fn error_response(id: Option<Value>, http: StatusCode, err: JsonRpcError) -> Response {
    (
        http,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

async fn dispatch_tool_call(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    params: &Value,
) -> Result<Value, (StatusCode, JsonRpcError)> {
    let call: ToolCallParams = serde_json::from_value(params.clone()).map_err(|e| {
        (
            StatusCode::OK,
            JsonRpcError::invalid_params(format!("tools/call params: {e}")),
        )
    })?;

    // Validate conversation ownership before using `conversation_id` for scope
    // derivation — a spoofed `x-conversation-id` must not let a user scope a
    // memory into a conversation/project they don't own. If not owned, drop it
    // (falls back to user scope) rather than erroring the whole call.
    let conversation_id = match conversation_id {
        Some(cid) => {
            let owner = Repos
                .code_sandbox
                .get_conversation_user_id(cid)
                .await
                .ok()
                .flatten();
            if owner == Some(user_id) { Some(cid) } else { None }
        }
        None => None,
    };

    let result = match call.name.as_str() {
        "remember" => remember(user_id, conversation_id, &call.arguments).await,
        "recall" => recall(user_id, conversation_id, &call.arguments).await,
        "forget" => forget(user_id, &call.arguments).await,
        other => {
            return Err((
                StatusCode::OK,
                JsonRpcError::method_not_found(&format!("memory tool: {other}")),
            ));
        }
    };

    match result {
        Ok(v) => Ok(json!({
            "content": [{ "type": "text", "text": v.to_string() }],
            "structuredContent": v,
        })),
        Err(e) => Err((StatusCode::OK, JsonRpcError::internal(e.to_string()))),
    }
}

#[derive(Debug, Deserialize)]
struct RememberArgs {
    content: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default = "default_importance")]
    importance: i16,
    /// 'user' | 'project' | 'conversation'. The model supplies only the scope
    /// NAME; the server derives the real ids (never trusts a raw project_id).
    #[serde(default)]
    scope: Option<String>,
}

/// Derive the (scope, project_id, conversation_id) to store from the LLM-chosen
/// scope name + the trusted conversation context. Fallbacks: omitted ⇒
/// conversation (least surprising); 'project' with no project ⇒ user.
async fn derive_scope(
    requested: Option<&str>,
    conversation_id: Option<Uuid>,
) -> (String, Option<Uuid>, Option<Uuid>) {
    match requested.unwrap_or("conversation") {
        "user" => ("user".to_string(), None, None),
        "project" => {
            if let Some(cid) = conversation_id {
                if let Ok(Some(pid)) = Repos.project.project_id_for_conversation(cid).await {
                    return ("project".to_string(), Some(pid), None);
                }
            }
            ("user".to_string(), None, None)
        }
        // "conversation" + any unknown value.
        _ => match conversation_id {
            Some(cid) => ("conversation".to_string(), None, Some(cid)),
            None => ("user".to_string(), None, None),
        },
    }
}
fn default_kind() -> String {
    "fact".to_string()
}
fn default_importance() -> i16 {
    50
}

async fn remember(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let args: RememberArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let content = args.content.trim();
    if content.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "content must not be empty",
        ));
    }
    if content.len() > MAX_CONTENT_LEN {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "content exceeds 4000 char limit",
        ));
    }

    let (scope, project_id, conv_id) = derive_scope(args.scope.as_deref(), conversation_id).await;

    let row = Repos
        .memory
        .insert(
            user_id,
            content,
            "mcp_tool",
            args.importance.clamp(0, 100),
            &args.kind,
            &json!({}),
            None,
            &scope,
            project_id,
            conv_id,
        )
        .await?;

    // Best-effort embed write-back so retrieval can find it.
    if let Ok(admin) = Repos.memory.get_admin_settings().await {
        if admin.enabled {
            if let Some(emb_model_id) = admin.embedding_model_id {
                if let Ok(vec) =
                    crate::modules::memory::engine::dispatch::embed(emb_model_id, content)
                        .await
                {
                    let model_name = Repos
                        .llm_model
                        .get_by_id(emb_model_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|m| m.name)
                        .unwrap_or_else(|| emb_model_id.to_string());
                    let v = HalfVector::from_f32_slice(&vec);
                    let pool = Repos.memory.pool_clone();
                    let _ = sqlx::query(
                        "UPDATE user_memories SET embedding = $1, embedding_model = $2 WHERE id = $3 AND user_id = $4",
                    )
                    .bind(&v)
                    .bind(&model_name)
                    .bind(row.id)
                    .bind(user_id)
                    .execute(&pool)
                    .await;
                }
            }
        }
    }

    Ok(json!({ "memory_id": row.id, "content": row.content, "scope": scope }))
}

#[derive(Debug, Deserialize)]
struct RecallArgs {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: i64,
}
fn default_top_k() -> i64 {
    8
}

async fn recall(
    user_id: Uuid,
    conversation_id: Option<Uuid>,
    args: &Value,
) -> Result<Value, AppError> {
    let args: RecallArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let admin = Repos.memory.get_admin_settings().await?;
    if !admin.enabled {
        return Err(AppError::bad_request(
            "MEMORY_DISABLED",
            "memory is disabled by the administrator",
        ));
    }
    let limit = args.top_k.clamp(1, 50);
    let q = args.query.trim();
    if q.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "query must not be empty",
        ));
    }

    // Scope union (user + this-project + this-conversation), same as the
    // automatic retriever.
    let project_id = match conversation_id {
        Some(cid) => Repos
            .project
            .project_id_for_conversation(cid)
            .await
            .ok()
            .flatten(),
        None => None,
    };

    use crate::modules::memory::chat_extension::retriever;
    // Hybrid (vector ⊕ FTS via RRF) when an embedding model is configured;
    // FTS-only fallback otherwise — so recall works embedding-free instead of
    // hard-erroring.
    let hits = match admin.embedding_model_id {
        Some(emb_id) => match crate::modules::memory::engine::dispatch::embed(emb_id, q).await {
            Ok(v) => {
                retriever::hybrid_search(
                    user_id,
                    project_id,
                    conversation_id,
                    HalfVector::from_f32_slice(&v),
                    admin.cosine_threshold,
                    q,
                    limit,
                )
                .await?
            }
            Err(_) => retriever::fts_search(user_id, project_id, conversation_id, q, limit).await?,
        },
        None => retriever::fts_search(user_id, project_id, conversation_id, q, limit).await?,
    };

    Ok(json!({
        "memories": hits.into_iter().map(|(id, content)| {
            json!({ "id": id, "content": content })
        }).collect::<Vec<_>>()
    }))
}

#[derive(Debug, Deserialize)]
struct ForgetArgs {
    memory_id: Uuid,
}

async fn forget(user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let args: ForgetArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let deleted = Repos
        .memory
        .soft_delete_owned(user_id, args.memory_id)
        .await?;
    if !deleted {
        return Err(AppError::not_found("Memory"));
    }
    Ok(json!({ "memory_id": args.memory_id, "deleted": true }))
}

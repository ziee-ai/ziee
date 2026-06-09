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
use crate::modules::code_sandbox::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use crate::modules::memory::permissions::MemoryWrite;
use crate::modules::permissions::RequirePermissions;
use crate::modules::sync::{
    Audience, SyncAction, SyncEntity, publish as sync_publish,
};

// Shared between memory + memory_mcp handlers (see memory/models.rs).
use crate::modules::memory::models::MAX_MEMORY_CONTENT_LEN as MAX_CONTENT_LEN;

#[debug_handler]
pub async fn jsonrpc_handler(
    auth: RequirePermissions<(MemoryWrite,)>,
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
        "tools/call" => match dispatch_tool_call(user_id, &req.params).await {
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
    params: &Value,
) -> Result<Value, (StatusCode, JsonRpcError)> {
    let call: ToolCallParams = serde_json::from_value(params.clone()).map_err(|e| {
        (
            StatusCode::OK,
            JsonRpcError::invalid_params(format!("tools/call params: {e}")),
        )
    })?;

    let result = match call.name.as_str() {
        "remember" => remember(user_id, &call.arguments).await,
        "recall" => recall(user_id, &call.arguments).await,
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
}
fn default_kind() -> String {
    "fact".to_string()
}
fn default_importance() -> i16 {
    50
}

async fn remember(user_id: Uuid, args: &Value) -> Result<Value, AppError> {
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

    // Notify the caller's other devices (origin None — MCP tool call, not a
    // tab-originated REST mutation).
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Create,
        row.id,
        Audience::owner(user_id),
        None,
    );
    Ok(json!({ "memory_id": row.id, "content": row.content }))
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

async fn recall(user_id: Uuid, args: &Value) -> Result<Value, AppError> {
    let args: RecallArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let admin = Repos.memory.get_admin_settings().await?;
    if !admin.enabled {
        return Err(AppError::bad_request(
            "MEMORY_DISABLED",
            "memory is disabled by the administrator",
        ));
    }
    let Some(emb_model_id) = admin.embedding_model_id else {
        return Err(AppError::bad_request(
            "MEMORY_DISABLED",
            "no embedding model configured",
        ));
    };

    let limit = args.top_k.clamp(1, 50);
    let q = args.query.trim();
    if q.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "query must not be empty",
        ));
    }

    let vec = crate::modules::memory::engine::dispatch::embed(emb_model_id, q).await?;
    let pool = Repos.memory.pool_clone();
    let rows: Vec<(Uuid, String, f32)> = sqlx::query_as(
        r#"
        SELECT id, content, (embedding <=> $2)::real AS distance
        FROM user_memories
        WHERE user_id = $1
          AND deleted_at IS NULL
          AND embedding IS NOT NULL
          AND (embedding <=> $2)::real < $4
        ORDER BY embedding <=> $2
        LIMIT $3
        "#,
    )
    .bind(user_id)
    .bind(&HalfVector::from_f32_slice(&vec))
    .bind(limit)
    .bind(admin.cosine_threshold)
    .fetch_all(&pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(json!({
        "memories": rows.into_iter().map(|(id, content, distance)| {
            json!({ "id": id, "content": content, "distance": distance })
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
    sync_publish(
        SyncEntity::Memory,
        SyncAction::Delete,
        args.memory_id,
        Audience::owner(user_id),
        None,
    );
    Ok(json!({ "memory_id": args.memory_id, "deleted": true }))
}

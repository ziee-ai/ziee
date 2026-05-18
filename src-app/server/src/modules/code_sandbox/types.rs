use std::path::PathBuf;

use aide::OperationIo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

// --- JSON-RPC protocol types ---

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// --- Per-request context passed to dispatch/tools ---

pub struct SandboxContext {
    pub pool: PgPool,
    pub data_dir: PathBuf,
    pub rootfs_path: PathBuf,
    pub conversation_id: Option<Uuid>,
    pub user_id: Uuid,
    pub base_url: String,
    pub jwt_secret: String,
}

// --- Axum State<> injected into the route handler ---

#[derive(Clone, Debug)]
pub struct CodeSandboxState {
    pub data_dir: PathBuf,
    pub rootfs_path: PathBuf,
    pub base_url: String,
    pub jwt_secret: String,
}

// --- Query params for the workspace file download endpoint ---

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct WorkspaceDownloadQuery {
    pub filename: String,
}

// --- Extractor for the optional x-conversation-id request header ---

#[derive(OperationIo)]
#[aide(input)]
pub struct ConversationIdHeader(pub Option<Uuid>);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ConversationIdHeader {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get("x-conversation-id")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| Uuid::parse_str(s).ok());
        Ok(ConversationIdHeader(id))
    }
}

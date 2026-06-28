//! Same-port reverse proxy handlers.
//!
//! These are the entry points for `/api/local-llm/v1/{chat/completions,embeddings,models}`.
//! They mediate every chat-completion call against a local engine
//! and are the SINGLE place "local-vs-remote" concerns surface.
//! Chat code never branches on `"local"`.

use aide::transform::TransformOperation;
use axum::body::Body;
use axum::extract::Extension;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Uuid;

use super::auto_start;
use super::proxy::{
    self, lookup_token, touch_last_used, InFlightGuard, InstanceFlag,
};
use crate::common::AppError;

// =====================================================================
// Error body shape (OpenAI-compat)
// =====================================================================

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProxyErrorBody {
    pub error: ProxyErrorInner,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProxyErrorInner {
    /// OpenAI-style error type: "authentication_error",
    /// "invalid_request_error", "not_found_error",
    /// "engine_start_timeout", "engine_start_failed",
    /// "engine_unavailable", "engine_failed", ...
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
    /// Optional per-error fields rendered as plain key/values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_failure_reason: Option<String>,
}

fn err_response(status: StatusCode, body: ProxyErrorBody) -> Response {
    (status, Json(body)).into_response()
}

fn err_auth() -> Response {
    err_response(
        StatusCode::UNAUTHORIZED,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "authentication_error".into(),
                message: "Missing or invalid Authorization bearer token".into(),
                ..default_error_inner()
            },
        },
    )
}

fn err_invalid_request(msg: impl Into<String>) -> Response {
    err_response(
        StatusCode::BAD_REQUEST,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "invalid_request_error".into(),
                message: msg.into(),
                ..default_error_inner()
            },
        },
    )
}

fn err_not_found(msg: impl Into<String>) -> Response {
    err_response(
        StatusCode::NOT_FOUND,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "not_found_error".into(),
                message: msg.into(),
                ..default_error_inner()
            },
        },
    )
}

fn err_engine_start_timeout(model: &str, elapsed_ms: u64) -> Response {
    err_response(
        StatusCode::GATEWAY_TIMEOUT,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "engine_start_timeout".into(),
                message: format!("Engine for model '{}' did not become healthy in time", model),
                model: Some(model.into()),
                elapsed_ms: Some(elapsed_ms),
                ..default_error_inner()
            },
        },
    )
}

fn err_engine_start_failed(reason: String) -> Response {
    err_response(
        StatusCode::BAD_GATEWAY,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "engine_start_failed".into(),
                message: reason,
                ..default_error_inner()
            },
        },
    )
}

fn err_engine_unavailable_draining() -> Response {
    err_response(
        StatusCode::SERVICE_UNAVAILABLE,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "engine_unavailable".into(),
                message: "Engine is being unloaded; retry shortly".into(),
                retry_after_ms: Some(2000),
                ..default_error_inner()
            },
        },
    )
}

fn err_engine_failed(reason: String) -> Response {
    err_response(
        StatusCode::SERVICE_UNAVAILABLE,
        ProxyErrorBody {
            error: ProxyErrorInner {
                kind: "engine_failed".into(),
                message: "Engine is in failed state; admin must clear".into(),
                last_failure_reason: Some(reason),
                ..default_error_inner()
            },
        },
    )
}

fn err_upstream(status: StatusCode, body: axum::body::Bytes) -> Response {
    // Pass through engine's error body as-is so debuggability is
    // preserved.
    Response::builder()
        .status(status)
        .body(Body::from(body))
        .unwrap()
}

fn default_error_inner() -> ProxyErrorInner {
    ProxyErrorInner {
        kind: String::new(),
        message: String::new(),
        model: None,
        elapsed_ms: None,
        retry_after_ms: None,
        last_failure_reason: None,
    }
}

// =====================================================================
// Auth helper
// =====================================================================

/// Extract `Authorization: Bearer <token>` and validate against the
/// cache. Returns the matching provider_id.
async fn auth_and_resolve_provider(headers: &HeaderMap) -> Result<Uuid, Response> {
    let auth = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("").trim();
    if token.is_empty() {
        return Err(err_auth());
    }
    lookup_token(token).await.ok_or_else(err_auth)
}

// =====================================================================
// Model resolution
// =====================================================================

/// Resolve a model NAME (the `model` field on the OpenAI body) to
/// `(model_id, file_path, status)` scoped to the given provider.
/// Returns 404 for cross-provider attempts so existence isn't leaked.
async fn resolve_model(
    pool: &PgPool,
    provider_id: Uuid,
    model_name: &str,
) -> Result<(Uuid, String), Response> {
    let row = sqlx::query!(
        "SELECT id, validation_status FROM llm_models
         WHERE provider_id = $1 AND name = $2 AND enabled = TRUE",
        provider_id,
        model_name,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("proxy: model lookup db error: {}", e);
        err_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ProxyErrorBody {
                error: ProxyErrorInner {
                    kind: "internal_error".into(),
                    message: "model lookup failed".into(),
                    ..default_error_inner()
                },
            },
        )
    })?;

    match row {
        Some(r) => Ok((r.id, r.validation_status.unwrap_or_default())),
        None => Err(err_not_found(format!("Unknown model: {}", model_name))),
    }
}

/// Read the live engine port + base_url for a started model.
async fn get_running_instance_base_url(
    pool: &PgPool,
    model_id: Uuid,
) -> Result<Option<String>, AppError> {
    let url: Option<String> = sqlx::query_scalar!(
        "SELECT base_url FROM llm_runtime_instances
         WHERE model_id = $1 AND status = 'running'",
        model_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("proxy: instance lookup: {e}")))?;
    Ok(url)
}

// =====================================================================
// Forward helper — shared by chat/completions + embeddings
// =====================================================================

async fn forward_post_with_body(
    pool: &PgPool,
    headers: &HeaderMap,
    body: axum::body::Bytes,
    suffix: &str,
) -> Response {
    // Auth.
    let provider_id = match auth_and_resolve_provider(headers).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    // Parse body as JSON; extract `model`.
    let parsed: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err_invalid_request(format!("body is not valid JSON: {e}")),
    };
    let model_name = match parsed.get("model").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return err_invalid_request("body field 'model' is required"),
    };

    // Resolve model.
    let (model_id, validation_status) =
        match resolve_model(pool, provider_id, &model_name).await {
            Ok(t) => t,
            Err(resp) => return resp,
        };
    if validation_status == "failed" || validation_status == "invalid" {
        return err_engine_failed(format!(
            "model {} validation_status = {}",
            model_name, validation_status
        ));
    }

    // Acquire the in-flight guard FIRST, then re-check the drain
    // flag. This closes the TOCTOU window (C2): the reaper sets
    // Draining then waits for inflight==0 before stopping. If it set
    // Draining before our guard, we observe it below and bail; if it
    // sets Draining after our guard, it sees inflight>0 and waits for
    // us. Either way the engine can't be stopped out from under an
    // in-flight request. The guard is held across auto-start too, so
    // a model mid-start is never reaped.
    let _guard = InFlightGuard::acquire(model_id).await;
    if proxy::get_instance_flag(model_id).await == InstanceFlag::Draining {
        return err_engine_unavailable_draining();
    }

    // Auto-start if not running.
    let started_at = std::time::Instant::now();
    if let Err(e) = auto_start::ensure_running(pool, model_id).await {
        let msg = format!("{}", e);
        if msg.contains("did not become Healthy") {
            return err_engine_start_timeout(
                &model_name,
                started_at.elapsed().as_millis() as u64,
            );
        }
        return err_engine_start_failed(msg);
    }

    // Resolve the live engine base_url.
    let engine_base = match get_running_instance_base_url(pool, model_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return err_engine_start_failed(
                "engine reported started but no running instance row".into(),
            );
        }
        Err(e) => return err_engine_start_failed(format!("{e}")),
    };

    // Look up per-instance bearer.
    let engine_bearer = match crate::modules::llm_local_runtime::deployment::local::get_instance_api_key(
        model_id,
    ) {
        Some(t) => t,
        None => {
            return err_engine_start_failed("missing per-instance bearer token".into());
        }
    };

    // Build the upstream URL: engine_base already includes scheme +
    // host + port; we append the OpenAI-compat path suffix.
    let upstream_url = format!("{}{}", engine_base.trim_end_matches('/'), suffix);

    // Forward via a shared reqwest client.
    let upstream = match shared_client()
        .post(&upstream_url)
        .bearer_auth(&engine_bearer)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return err_engine_start_failed(format!("upstream POST failed: {e}"));
        }
    };

    touch_last_used(model_id).await;
    stream_back(upstream).await
}

/// Forward a GET (no body). Used by `/v1/models`.
async fn forward_get(
    pool: &PgPool,
    headers: &HeaderMap,
    _suffix: &str,
) -> Response {
    // Auth.
    let provider_id = match auth_and_resolve_provider(headers).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    // We don't actually need to touch the engine — return the
    // provider's configured models from our own DB in OpenAI shape.
    list_provider_models(pool, provider_id).await
}

async fn list_provider_models(pool: &PgPool, provider_id: Uuid) -> Response {
    // Bound the otherwise-unbounded models list. This is an OpenAI-compatible
    // `/v1/models` proxy endpoint (consumed by OpenAI SDK clients that don't
    // send limit/offset), so the cap is applied server-side via the shared
    // DEFAULT_PAGE_SIZE rather than exposed as query params. A provider's
    // configured model count is far below this in practice.
    let rows = match sqlx::query!(
        "SELECT name, created_at FROM llm_models
         WHERE provider_id = $1 AND enabled = TRUE
         ORDER BY created_at DESC
         LIMIT $2",
        provider_id,
        crate::common::DEFAULT_PAGE_SIZE as i64,
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return err_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ProxyErrorBody {
                    error: ProxyErrorInner {
                        kind: "internal_error".into(),
                        message: format!("models query failed: {e}"),
                        ..default_error_inner()
                    },
                },
            );
        }
    };

    let data: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            // created_at is time::OffsetDateTime (sqlx default here);
            // unix_timestamp() is the time-crate equivalent of chrono's
            // timestamp().
            serde_json::json!({
                "id": r.name,
                "object": "model",
                "created": r.created_at.unix_timestamp(),
                "owned_by": "local",
            })
        })
        .collect();

    Json(serde_json::json!({
        "object": "list",
        "data": data,
    }))
    .into_response()
}

/// Forward the upstream response body to the client, preserving
/// SSE streaming. The body bytes are passed through verbatim.
async fn stream_back(upstream: reqwest::Response) -> Response {
    let status = upstream.status();
    let mut headers_out = HeaderMap::new();
    for (k, v) in upstream.headers().iter() {
        if k == reqwest::header::CONTENT_LENGTH || k == reqwest::header::TRANSFER_ENCODING {
            continue;
        }
        if let Ok(hv) = HeaderValue::from_bytes(v.as_bytes()) {
            if let Ok(name) = axum::http::HeaderName::from_bytes(k.as_str().as_bytes()) {
                headers_out.insert(name, hv);
            }
        }
    }

    if !status.is_success() {
        // For non-2xx, materialize the body so we can attach our
        // structured envelope.
        let body = upstream.bytes().await.unwrap_or_default();
        return err_upstream(
            axum::http::StatusCode::from_u16(status.as_u16())
                .unwrap_or(StatusCode::BAD_GATEWAY),
            body,
        );
    }

    let stream = upstream.bytes_stream();
    let body = Body::from_stream(stream);

    let mut resp = Response::new(body);
    *resp.status_mut() = axum::http::StatusCode::from_u16(status.as_u16())
        .unwrap_or(StatusCode::OK);
    *resp.headers_mut() = headers_out;
    resp
}

// =====================================================================
// Shared reqwest client (keep-alive pool)
// =====================================================================

static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn shared_client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            // No request timeout — the engine may stream a multi-minute
            // completion. We must NOT call `.timeout(Duration::ZERO)`:
            // reqwest treats a zero duration as a 0ms deadline (every
            // request fails instantly), not "unlimited". Omitting the
            // call entirely is reqwest's "no timeout" default. SSE
            // clients (the chat module's outbound `Provider::new` wrap)
            // handle their own cancellation.
            .no_proxy()
            .build()
            .expect("shared reqwest client init")
    })
}

// =====================================================================
// Handlers
// =====================================================================

pub async fn proxy_chat_completions(
    Extension(pool): Extension<sqlx::PgPool>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    forward_post_with_body(&pool, &headers, body, "/v1/chat/completions").await
}

pub fn proxy_chat_completions_docs(op: TransformOperation) -> TransformOperation {
    op.id("LocalLlmProxy.chatCompletions")
        .tag("Local LLM Proxy")
        .summary("OpenAI-compatible /v1/chat/completions proxy.")
        .description(concat!(
            "Auth via Authorization: Bearer <PROXY_TOKEN> (the api_key ",
            "of a local llm_provider). Model name from the body's `model` ",
            "field. Auto-starts the engine if needed."
        ))
}

pub async fn proxy_embeddings(
    Extension(pool): Extension<sqlx::PgPool>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    forward_post_with_body(&pool, &headers, body, "/v1/embeddings").await
}

pub fn proxy_embeddings_docs(op: TransformOperation) -> TransformOperation {
    op.id("LocalLlmProxy.embeddings")
        .tag("Local LLM Proxy")
        .summary("OpenAI-compatible /v1/embeddings proxy.")
}

pub async fn proxy_models(
    Extension(pool): Extension<sqlx::PgPool>,
    headers: HeaderMap,
) -> Response {
    forward_get(&pool, &headers, "/v1/models").await
}

pub fn proxy_models_docs(op: TransformOperation) -> TransformOperation {
    op.id("LocalLlmProxy.listModels")
        .tag("Local LLM Proxy")
        .summary("OpenAI-compatible /v1/models — list models in this provider.")
}

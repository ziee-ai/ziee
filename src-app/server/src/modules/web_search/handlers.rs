//! HTTP handlers: the JSON-RPC MCP endpoint + the admin settings REST surface.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::code_sandbox::types::{
    ConversationIdHeader, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
};
use crate::modules::permissions::{RequirePermissions, with_permission};
use crate::modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish};

use super::models::{
    ProviderCatalogEntry, ProviderCatalogResponse, SaveUserProviderKeyRequest,
    UpdateProviderRequest, UpdateWebSearchSettingsRequest, UserProviderKeyCatalogEntry,
    UserProviderKeyCatalogResponse, WebSearchSettings,
};
use super::permissions::{WebSearchAdminManage, WebSearchAdminRead, WebSearchUse};
use super::{fetch, providers};

// ─────────────────────────── JSON-RPC MCP endpoint ───────────────────────────

#[debug_handler]
pub async fn jsonrpc_handler(
    // Gated on `web_search::use`; the JWT is validated by the extractor. Both
    // tools share this single permission. Conversation id is accepted but
    // unused (the tools are stateless / user-scoped).
    auth: RequirePermissions<(WebSearchUse,)>,
    ConversationIdHeader(_conversation_id): ConversationIdHeader,
    body: axum::body::Bytes,
) -> Response {
    let user_id = auth.user.id;
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
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => ok_response(
            id,
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "web_search", "version": env!("CARGO_PKG_VERSION") },
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
        "web_search" => do_search(user_id, &call.arguments).await,
        "fetch_url" => do_fetch(&call.arguments).await,
        other => {
            return Err((
                StatusCode::OK,
                JsonRpcError::method_not_found(&format!("web_search tool: {other}")),
            ));
        }
    };

    match result {
        // Each tool returns a (readable text rendering, structured value) pair.
        // The text is what the LLM reads (text-as-text — NOT stringified JSON);
        // structuredContent is the typed payload the UI renders + the model can
        // recall via get_tool_result. Both are now persisted (structured_content
        // on the tool_result block).
        Ok((text, structured)) => Ok(json!({
            "content": [{ "type": "text", "text": text }],
            "structuredContent": structured,
        })),
        Err(e) => Err((StatusCode::OK, JsonRpcError::from_app_error(&e))),
    }
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    query: String,
    #[serde(default)]
    max_results: Option<i64>,
}

/// Returns `(readable text digest, structured payload)`. The digest is what the
/// LLM reads (one line per hit, text-as-text); the structured payload `{ provider,
/// results }` is the typed copy for the UI / get_tool_result recall.
async fn do_search(user_id: Uuid, args: &Value) -> Result<(String, Value), AppError> {
    let args: SearchArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let q = args.query.trim();
    if q.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "query must not be empty",
        ));
    }
    let settings = Repos.web_search.get_settings().await?;
    if !settings.enabled {
        return Err(AppError::bad_request(
            "WEB_SEARCH_DISABLED",
            "web search is disabled by the administrator",
        ));
    }
    let count = args
        .max_results
        .map(|n| n.clamp(1, 20) as usize)
        .unwrap_or_else(|| settings.max_results.clamp(1, 20) as usize);

    let outcome = providers::search_via_chain(q, count, &settings, user_id).await?;

    // Readable digest for the model — one entry per hit, NOT stringified JSON.
    let mut text = format!(
        "{} result(s) for \"{}\" (via {}).\n",
        outcome.results.len(),
        q,
        outcome.provider
    );
    if outcome.results.is_empty() {
        text.push_str("No results.\n");
    } else {
        for (i, hit) in outcome.results.iter().enumerate() {
            text.push_str(&format!(
                "{}. {} — {}\n   {}\n",
                i + 1,
                hit.title,
                hit.url,
                hit.snippet
            ));
        }
    }

    let structured = json!({ "provider": outcome.provider, "results": outcome.results });
    Ok((text, structured))
}

#[derive(Debug, Deserialize)]
struct FetchArgs {
    url: String,
}

/// Returns `(readable markdown, structured FetchedPage)`. A fetched page's content
/// IS text, so it goes in the text channel as readable markdown (title/url header +
/// body + truncation note) — NOT wrapped in a JSON string. structuredContent is the
/// typed `FetchedPage` for the UI / get_tool_result recall.
async fn do_fetch(args: &Value) -> Result<(String, Value), AppError> {
    let args: FetchArgs = serde_json::from_value(args.clone())
        .map_err(|e| AppError::bad_request("INVALID_ARGS", e.to_string()))?;
    let url = args.url.trim();
    if url.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "url must not be empty",
        ));
    }
    let settings = Repos.web_search.get_settings().await?;
    if !settings.enabled {
        return Err(AppError::bad_request(
            "WEB_SEARCH_DISABLED",
            "web search is disabled by the administrator",
        ));
    }
    let page = fetch::fetch_url(
        url,
        settings.fetch_max_bytes.max(0) as u64,
        settings.fetch_max_chars.max(0) as usize,
        settings.request_timeout_secs.max(1) as u64,
    )
    .await?;

    let mut text = String::new();
    if !page.title.is_empty() {
        text.push_str(&format!("# {}\n", page.title));
    }
    text.push_str(&format!("<{}>\n\n", page.final_url));
    text.push_str(&page.content);
    if page.truncated {
        text.push_str("\n\n[content truncated at the configured character cap]");
    }

    let structured =
        serde_json::to_value(&page).map_err(|e| AppError::internal_error(e.to_string()))?;
    Ok((text, structured))
}

// ─────────────────────────── Admin REST: settings ───────────────────────────

#[debug_handler]
pub async fn get_settings(
    _auth: RequirePermissions<(WebSearchAdminRead,)>,
) -> ApiResult<Json<WebSearchSettings>> {
    let row = Repos.web_search.get_settings().await?;
    Ok((StatusCode::OK, Json(row)))
}

pub fn get_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchAdminRead,)>(op)
        .id("WebSearch.getSettings")
        .tag("WebSearch")
        .summary("Read web search settings")
        .response::<200, Json<WebSearchSettings>>()
}

#[debug_handler]
pub async fn update_settings(
    _auth: RequirePermissions<(WebSearchAdminManage,)>,
    origin: SyncOrigin,
    Json(body): Json<UpdateWebSearchSettingsRequest>,
) -> ApiResult<Json<WebSearchSettings>> {
    if let Some(ref chain) = body.provider_chain {
        if chain.is_empty() {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                "provider_chain must not be empty",
            )
            .into());
        }
        providers::validate_chain(chain)?;
    }
    if let Some(n) = body.max_results
        && !(1..=20).contains(&n)
    {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "max_results out of range (1..=20)").into(),
        );
    }
    if let Some(n) = body.fetch_max_bytes
        && !(65_536..=104_857_600).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "fetch_max_bytes out of range (65536..=104857600)",
        )
        .into());
    }
    if let Some(n) = body.fetch_max_chars
        && !(1_000..=500_000).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "fetch_max_chars out of range (1000..=500000)",
        )
        .into());
    }
    if let Some(n) = body.request_timeout_secs
        && !(1..=120).contains(&n)
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "request_timeout_secs out of range (1..=120)",
        )
        .into());
    }

    let row = Repos
        .web_search
        .update_settings(
            body.enabled,
            body.provider_chain,
            body.max_results,
            body.fetch_max_bytes,
            body.fetch_max_chars,
            body.request_timeout_secs,
        )
        .await?;

    sync_publish(
        SyncEntity::WebSearchSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<WebSearchAdminRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(row)))
}

pub fn update_settings_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchAdminManage,)>(op)
        .id("WebSearch.updateSettings")
        .tag("WebSearch")
        .summary("Update web search settings (enable, provider chain, caps)")
        .response::<200, Json<WebSearchSettings>>()
}

// ─────────────────────────── Admin REST: providers ──────────────────────────

/// Build the provider catalog = code descriptors joined with stored state.
async fn build_catalog() -> Result<ProviderCatalogResponse, AppError> {
    let rows = Repos.web_search.list_providers().await?;
    let entries = providers::catalog()
        .into_iter()
        .map(|d| {
            let row = rows.iter().find(|r| r.provider == d.key);
            let api_key = row.and_then(|r| r.api_key.as_deref());
            let config = row
                .map(|r| r.config.clone())
                .unwrap_or_else(|| serde_json::json!({}));
            let configured = providers::is_configured(&d, api_key, &config);
            ProviderCatalogEntry {
                key: d.key.to_string(),
                display_name: d.display_name.to_string(),
                needs_api_key: d.needs_api_key,
                config_fields: d.config_fields.clone(),
                configured,
                api_key_set: api_key.map(|k| !k.trim().is_empty()).unwrap_or(false),
                config,
            }
        })
        .collect();
    Ok(ProviderCatalogResponse { providers: entries })
}

#[debug_handler]
pub async fn get_providers(
    _auth: RequirePermissions<(WebSearchAdminRead,)>,
) -> ApiResult<Json<ProviderCatalogResponse>> {
    Ok((StatusCode::OK, Json(build_catalog().await?)))
}

pub fn get_providers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchAdminRead,)>(op)
        .id("WebSearch.getProviders")
        .tag("WebSearch")
        .summary("List search-provider catalog (descriptors + configured state)")
        .response::<200, Json<ProviderCatalogResponse>>()
}

#[debug_handler]
pub async fn update_provider(
    _auth: RequirePermissions<(WebSearchAdminManage,)>,
    origin: SyncOrigin,
    Path(provider): Path<String>,
    Json(body): Json<UpdateProviderRequest>,
) -> ApiResult<Json<ProviderCatalogResponse>> {
    // Reject unknown providers (registry is the source of truth).
    if providers::descriptor(&provider).is_none() {
        return Err(AppError::bad_request(
            "WEB_SEARCH_UNKNOWN_PROVIDER",
            format!("unknown search provider: {provider}"),
        )
        .into());
    }

    // Normalize an explicit JSON `null` to "absent" so it follows the
    // documented contract (absent = leave existing config) instead of
    // overwriting the stored config with JSONB 'null' (which would silently
    // unconfigure the provider).
    let config = body.config.filter(|v| !v.is_null());

    // Validate provider config at write time so a malformed value (e.g. a
    // non-URL searxng base_url) can't be stored and mis-reported as configured.
    if let Some(ref cfg) = config {
        providers::validate_config(&provider, cfg)?;
    }

    // api_key tri-state: absent = leave; "" = clear; non-empty = set.
    let api_key_action = body.api_key.map(|k| {
        let k = k.trim().to_string();
        if k.is_empty() { None } else { Some(k) }
    });

    Repos
        .web_search
        .upsert_provider(&provider, api_key_action, config)
        .await?;

    sync_publish(
        SyncEntity::WebSearchSettings,
        SyncAction::Update,
        Uuid::nil(),
        Audience::perm::<WebSearchAdminRead>(),
        origin.0,
    );
    Ok((StatusCode::OK, Json(build_catalog().await?)))
}

pub fn update_provider_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchAdminManage,)>(op)
        .id("WebSearch.updateProvider")
        .tag("WebSearch")
        .summary("Upsert one provider's API key / config")
        .response::<200, Json<ProviderCatalogResponse>>()
}

// ──────────────────────── User REST: per-user provider keys ──────────────────
// Gated on `web_search::use` (held by the Users group): a user who can use web
// search may manage their OWN key, resolved before the deployment key. The raw
// key is never echoed — only masked state + a `system_key_set` boolean.

/// Validate a user-supplied API key: non-empty, ≤500 chars, no control chars.
/// Mirrors `llm_provider::handlers::user::save_user_api_key`.
fn validate_user_key(raw: &str) -> Result<String, AppError> {
    let key = raw.trim().to_string();
    if key.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "API key cannot be empty",
        ));
    }
    if key.len() > 500 {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "API key too long",
        ));
    }
    if key.bytes().any(|b| b < 0x20 && b != b'\t') {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "API key contains invalid characters",
        ));
    }
    Ok(key)
}

/// Build the user-facing key catalog: the key-accepting providers joined with
/// the deployment key-set boolean + the user's own masked key.
async fn build_user_key_catalog(user_id: Uuid) -> Result<UserProviderKeyCatalogResponse, AppError> {
    let rows = Repos.web_search.list_providers().await?;
    let user_keys = Repos.web_search.list_user_keys_masked(user_id).await?;
    let providers = providers::catalog()
        .into_iter()
        .filter(|d| d.needs_api_key)
        .map(|d| {
            let system_key_set = rows
                .iter()
                .find(|r| r.provider == d.key)
                .and_then(|r| r.api_key.as_deref())
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false);
            let user_key = user_keys
                .iter()
                .find(|e| e.provider == d.key)
                .map(|e| e.masked_key.clone());
            UserProviderKeyCatalogEntry {
                provider: d.key.to_string(),
                display_name: d.display_name.to_string(),
                needs_api_key: d.needs_api_key,
                system_key_set,
                user_key,
            }
        })
        .collect();
    Ok(UserProviderKeyCatalogResponse { providers })
}

/// List the calling user's web-search provider keys (masked) + catalog state.
#[debug_handler]
pub async fn list_user_keys(
    auth: RequirePermissions<(WebSearchUse,)>,
) -> ApiResult<Json<UserProviderKeyCatalogResponse>> {
    Ok((
        StatusCode::OK,
        Json(build_user_key_catalog(auth.user.id).await?),
    ))
}

pub fn list_user_keys_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchUse,)>(op)
        .id("WebSearch.listUserKeys")
        .tag("WebSearch")
        .summary("List the caller's own web-search provider keys (masked) + catalog")
        .description(
            "Returns each key-accepting provider with the caller's own key in masked \
             form (or null) and whether a shared deployment key exists. The raw key \
             value is never returned.",
        )
        .response::<200, Json<UserProviderKeyCatalogResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Save or update the calling user's own key for a provider.
#[debug_handler]
pub async fn save_user_key(
    auth: RequirePermissions<(WebSearchUse,)>,
    origin: SyncOrigin,
    Path(provider): Path<String>,
    Json(body): Json<SaveUserProviderKeyRequest>,
) -> ApiResult<Json<UserProviderKeyCatalogResponse>> {
    // Only known, key-accepting providers may hold a user key.
    match providers::descriptor(&provider) {
        Some(d) if d.needs_api_key => {}
        Some(_) => {
            return Err(AppError::bad_request(
                "WEB_SEARCH_PROVIDER_NO_KEY",
                format!("provider '{provider}' does not use an API key"),
            )
            .into());
        }
        None => {
            return Err(AppError::bad_request(
                "WEB_SEARCH_UNKNOWN_PROVIDER",
                format!("unknown search provider: {provider}"),
            )
            .into());
        }
    }

    let key = validate_user_key(&body.api_key)?;
    Repos
        .web_search
        .upsert_user_key(auth.user.id, &provider, &key)
        .await?;

    sync_publish(
        SyncEntity::WebSearchUserKey,
        SyncAction::Update,
        Uuid::nil(),
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((
        StatusCode::OK,
        Json(build_user_key_catalog(auth.user.id).await?),
    ))
}

pub fn save_user_key_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchUse,)>(op)
        .id("WebSearch.saveUserKey")
        .tag("WebSearch")
        .summary("Save the caller's own API key for a web-search provider")
        .description("Stores the caller's own key (encrypted). The key is never echoed back.")
        .response::<200, Json<UserProviderKeyCatalogResponse>>()
        .response_with::<400, (), _>(|res| res.description("Validation error / unknown provider"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Delete the calling user's own key for a provider (falls back to the shared key).
#[debug_handler]
pub async fn delete_user_key(
    auth: RequirePermissions<(WebSearchUse,)>,
    origin: SyncOrigin,
    Path(provider): Path<String>,
) -> ApiResult<()> {
    Repos
        .web_search
        .delete_user_key(auth.user.id, &provider)
        .await?;

    sync_publish(
        SyncEntity::WebSearchUserKey,
        SyncAction::Delete,
        Uuid::nil(),
        Audience::owner(auth.user.id),
        origin.0,
    );
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_user_key_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WebSearchUse,)>(op)
        .id("WebSearch.deleteUserKey")
        .tag("WebSearch")
        .summary("Delete the caller's own API key for a web-search provider")
        .response::<204, ()>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

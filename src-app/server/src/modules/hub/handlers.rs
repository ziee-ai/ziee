use crate::core::Repos;
use aide::transform::TransformOperation;
use axum::{Extension, Json, debug_handler, extract::Query, http::StatusCode};

use crate::{
    common::{ApiResult, AppError},
    core::events::EventBus,
    modules::{
        assistant::{events::AssistantEvent, permissions::AssistantsTemplateCreate},
        llm_model::{ModelParameters, permissions::LlmModelsCreate},
        mcp::McpServersAdminCreate,
        permissions::{RequirePermissions, with_permission},
    },
};
use std::sync::Arc;

use super::{
    events::HubEvent,
    hub_manager::{Catalog, HubManager, HubManifest},
    models::{HubCategory, HubEntityType},
    permissions::*,
    types::*,
};
use axum::extract::Path as AxumPath;
// HubReleaseInfo is re-exported through `types::*` below; the response
// wrappers (HubReleasesResponse, ActivateHubVersionRequest) live in types.rs.

// =====================================================
// Route Handlers
// =====================================================

/// Get hub models with locale support and created_ids (system-wide)
#[debug_handler]
pub async fn get_hub_models(
    _auth: RequirePermissions<(HubModelsRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubModelsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created model IDs (system-wide, no user filter)
    let created_map = Repos.hub.get_created_model_ids().await?;

    // Compute, per source repository, whether a credential is configured — so the
    // UI can block + guide BEFORE the user clicks download. Mirrors the precise,
    // decrypting `has_credential()` gate EXACTLY (repos are few, so the per-row
    // decrypt is cheap). `database_error` redacts the raw sqlx text.
    let cred_by_url: std::collections::HashMap<String, bool> = Repos
        .llm_repository
        .list_credential_presence()
        .await
        .map_err(AppError::database_error)?
        .into_iter()
        .collect();

    // Merge created_ids + source_auth_configured into models
    let mut models = hub_data.models;
    for model in &mut models {
        model.created_ids = created_map.get(&model.id).cloned().unwrap_or_default();
        model.source_auth_configured = cred_by_url
            .get(&model.repository_url)
            .copied()
            .unwrap_or(false);
    }

    Ok((StatusCode::OK, Json(models)))
}

/// Get hub assistants with locale support and created_ids for current user
#[debug_handler]
pub async fn get_hub_assistants(
    auth: RequirePermissions<(HubAssistantsRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubAssistantsResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created assistant IDs for current user
    let created_map = Repos.hub.get_created_assistant_ids(auth.user.id).await?;
    // Get system-wide template install IDs (used to disable the
    // "Use as Template" button when a template already exists, so
    // admins don't accidentally create duplicates).
    let template_map = Repos.hub.get_template_install_ids().await?;

    // Merge created_ids + created_template_ids into assistants
    let mut assistants = hub_data.assistants;
    for assistant in &mut assistants {
        assistant.created_ids = created_map.get(&assistant.id).cloned().unwrap_or_default();
        assistant.created_template_ids = template_map
            .get(&assistant.id)
            .cloned()
            .unwrap_or_default();
    }

    Ok((StatusCode::OK, Json(assistants)))
}

/// Get hub MCP servers with locale support and created_ids for current user
#[debug_handler]
pub async fn get_hub_mcp_servers(
    auth: RequirePermissions<(HubMCPServersRead,)>,

    Query(query): Query<HubQuery>,
) -> ApiResult<Json<HubMCPServersResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale(&query.lang).await?;

    // Get created MCP server IDs for current user
    let created_map = Repos.hub.get_created_mcp_server_ids(auth.user.id).await?;
    // Get system-wide install IDs (used to disable the "Install as
    // System" button when a system install already exists, so admins
    // don't accidentally create duplicates).
    let system_map = Repos.hub.get_system_mcp_install_ids().await?;

    // Merge created_ids + created_system_ids into servers
    let mut mcp_servers = hub_data.mcp_servers;
    for server in &mut mcp_servers {
        server.created_ids = created_map.get(&server.id).cloned().unwrap_or_default();
        server.created_system_ids = system_map
            .get(&server.id)
            .cloned()
            .unwrap_or_default();
    }

    Ok((StatusCode::OK, Json(mcp_servers)))
}

/// Get hub models version
#[debug_handler]
pub async fn get_hub_models_version(
    _auth: RequirePermissions<(HubModelsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.current_version().await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: hub_manager.last_refreshed().map(|t| t.to_rfc3339()),
        }),
    ))
}

/// Get hub assistants version
#[debug_handler]
pub async fn get_hub_assistants_version(
    _auth: RequirePermissions<(HubAssistantsVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.current_version().await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: hub_manager.last_refreshed().map(|t| t.to_rfc3339()),
        }),
    ))
}

/// Get hub MCP servers version
#[debug_handler]
pub async fn get_hub_mcp_servers_version(
    _auth: RequirePermissions<(HubMCPServersVersionRead,)>,
) -> ApiResult<Json<HubVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let version = hub_manager.current_version().await?;

    Ok((
        StatusCode::OK,
        Json(HubVersionResponse {
            version,
            last_updated: hub_manager.last_refreshed().map(|t| t.to_rfc3339()),
        }),
    ))
}

/// Refresh hub models from GitHub
#[debug_handler]
pub async fn refresh_hub_models(
    _auth: RequirePermissions<(HubModelsRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.current_version().await?;
    // Honor the admin pin (same as POST /hub/refresh) — the legacy
    // per-category endpoints still drive a full unified refresh.
    let pinned = Repos.hub.get_pinned_version().await?;
    hub_manager.refresh(pinned).await?;
    let new_version = hub_manager.current_version().await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::models_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

/// Refresh hub assistants from GitHub
#[debug_handler]
pub async fn refresh_hub_assistants(
    _auth: RequirePermissions<(HubAssistantsRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.current_version().await?;
    let pinned = Repos.hub.get_pinned_version().await?;
    hub_manager.refresh(pinned).await?;
    let new_version = hub_manager.current_version().await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::assistants_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

/// Refresh hub MCP servers from GitHub
#[debug_handler]
pub async fn refresh_hub_mcp_servers(
    _auth: RequirePermissions<(HubMCPServersRefresh,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    let old_version = hub_manager.current_version().await?;
    let pinned = Repos.hub.get_pinned_version().await?;
    hub_manager.refresh(pinned).await?;
    let new_version = hub_manager.current_version().await?;

    // Emit event if version changed
    if old_version != new_version {
        event_bus.emit_async(
            HubEvent::mcp_servers_refreshed(old_version.clone(), new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubRefreshResponse {
            updated: old_version != new_version,
            version: new_version,
        }),
    ))
}

// =====================================================
// ASSISTANT FROM HUB
// =====================================================

/// Output of `build_assistant_create_from_hub` — bundles the typed
/// create-request the caller passes to `Repos.assistant.create` with
/// the catalog version that the same lookup resolved against. The
/// version is captured ONCE here (not re-read after the insert) so
/// concurrent catalog activation can't drift the
/// `hub_entities.hub_version` stamp away from the data we actually
/// installed.
struct HubAssistantCreatePlan {
    create_request: crate::modules::assistant::types::CreateAssistantRequest,
    hub_version: Option<String>,
}

/// Shared lookup + validation for both hub-assistant install paths
/// (user / template). `is_template` discriminates the result; the
/// permission gate is at the extractor, not here.
async fn build_assistant_create_from_hub(
    request: &CreateAssistantFromHubRequest,
    is_template: bool,
) -> Result<HubAssistantCreatePlan, AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;
    // Capture the catalog version up front so the same version stamps
    // the `hub_entities` row downstream — guards against a concurrent
    // /hub/activate swap between this lookup and the tracking insert.
    let hub_version = hub_manager.current_version().await.ok();

    let hub_assistant = hub_data
        .assistants
        .into_iter()
        .find(|a| a.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub assistant '{}'", request.hub_id)))?;

    // Defense-in-depth: reject incompatible items (min_ziee_version >
    // server). The UI hides these in the catalog; this is the
    // backstop for a direct API call.
    hub_manager
        .ensure_installable(HubCategory::Assistant, &request.hub_id)
        .await?;

    let create_request = crate::modules::assistant::types::CreateAssistantRequest {
        name: request.name.clone().unwrap_or(hub_assistant.name.clone()),
        description: request
            .description
            .clone()
            .or(hub_assistant.description.clone()),
        instructions: request
            .instructions
            .clone()
            .or(hub_assistant.instructions.clone()),
        parameters: request
            .parameters
            .clone()
            .and_then(|p| serde_json::from_value::<ModelParameters>(p).ok())
            .or_else(|| {
                serde_json::from_value::<ModelParameters>(hub_assistant.parameters.clone()).ok()
            }),
        is_template: Some(is_template),
        is_default: Some(request.is_default),
        enabled: Some(request.enabled),
    };

    // Mirror the native handlers' validation gates
    // (`assistant::handlers::create_user_assistant` +
    // `create_template_assistant`). Without these a caller-supplied
    // empty name or a multi-MB description / instructions would land
    // straight in the assistants table — and as a TEMPLATE would fan
    // out to every new user via the clone-on-signup hook, amplifying
    // token cost on every chat turn.
    if create_request.name.trim().is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Assistant name cannot be empty",
        ));
    }
    crate::modules::assistant::handlers::validate_assistant_text_lengths(
        create_request.description.as_deref(),
        create_request.instructions.as_deref(),
    )?;

    Ok(HubAssistantCreatePlan {
        create_request,
        hub_version,
    })
}

/// Create a user-scoped assistant from the hub catalog. The resulting
/// row has `is_template=false` and `created_by=<user.id>` — owned by
/// the caller, only visible to them in the assistant list.
#[debug_handler]
pub async fn create_assistant_from_hub(
    auth: RequirePermissions<(HubAssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateAssistantFromHubRequest>,
) -> ApiResult<Json<AssistantFromHubResponse>> {
    // `replace_existing` is template-only — reject explicitly so
    // clients don't silently pass it expecting an idempotent re-install
    // on the user-scoped path (per-user installs aren't dedup'd).
    if request.replace_existing {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "replace_existing is only valid on the template install endpoint",
        )
        .into());
    }
    let plan = build_assistant_create_from_hub(&request, false).await?;

    let assistant = Repos
        .assistant
        .create(Some(auth.user.id), plan.create_request)
        .await?;

    // Track in hub_entities, stamping the catalog version captured
    // by the lookup so /hub/updates can detect when this row falls
    // behind a future catalog activation.
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::Assistant,
            assistant.id,
            &request.hub_id,
            HubCategory::Assistant,
            Some(auth.user.id),
            plan.hub_version.as_deref(),
        )
        .await?;

    event_bus.emit_async(
        HubEvent::assistant_created_from_hub(
            assistant.id,
            request.hub_id.clone(),
            false,
        )
        .into(),
    );

    Ok((
        StatusCode::CREATED,
        Json(AssistantFromHubResponse {
            assistant,
            hub_tracking,
        }),
    ))
}

/// Create a SYSTEM-WIDE template assistant from the hub catalog.
/// `is_template=true` + `created_by=NULL` per the assistants table
/// CHECK constraint (migration 6: `template_must_have_no_owner`).
/// The clone-default-templates-on-signup hook in
/// `assistant::event_handlers` will then propagate this template to
/// every new user's assistant list.
///
/// Permission gate is the intersection of "can install from the
/// hub" + "can author templates" — admins typically have both.
#[debug_handler]
pub async fn create_assistant_template_from_hub(
    _auth: RequirePermissions<(HubAssistantsCreate, AssistantsTemplateCreate)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateAssistantFromHubRequest>,
) -> ApiResult<Json<AssistantFromHubResponse>> {
    // Idempotency guard: look up the existing template install (if
    // any) but DON'T delete yet. The delete is deferred until AFTER
    // the catalog lookup + validation succeeds so a failing re-install
    // (e.g. the upstream maintainer raised min_ziee_version since the
    // original install, or expanded instructions past the 64 KiB cap)
    // does NOT leave the admin with the prior template wiped + no
    // system-wide fallback for new signups.
    let existing_id = Repos.hub.find_template_install(&request.hub_id).await?;
    if existing_id.is_some() && !request.replace_existing {
        return Err(AppError::conflict("Hub assistant template").into());
    }

    // Carry forward the prior template's `is_default` / `enabled` on
    // the `replace_existing` re-install path so a previously-promoted
    // template doesn't silently get demoted off auto-clone duty by the
    // refresh. (`CloneTemplateAssistantsHandler` only fans out
    // `is_default && enabled` templates to new users — silently
    // flipping either OFF would stop new signups receiving the
    // template until an admin re-promotes manually.)
    let mut plan = build_assistant_create_from_hub(&request, true).await?;
    if let Some(existing_id) = existing_id {
        if let Some(prior) = Repos.assistant.get(existing_id).await? {
            plan.create_request.is_default = Some(prior.is_default);
            plan.create_request.enabled = Some(prior.enabled);
        }
    }

    // Validation passed — now delete the prior template (if any) and
    // emit `AssistantEvent::Deleted` so the hub module's
    // `CleanupHubEntitiesHandler` removes the orphan `hub_entities`
    // row. There is NO FK cascade between `hub_entities.entity_id`
    // and `assistants.id`; cleanup is event-driven.
    if let Some(existing_id) = existing_id {
        // Tolerate "already deleted" — racy with the admin templates
        // page deleting the same row in another tab. Any other DB
        // error still surfaces.
        match Repos.assistant.delete(existing_id).await {
            Ok(()) => {
                event_bus
                    .emit(AssistantEvent::deleted(existing_id, None))
                    .await;
            }
            Err(e) if e.status_code() == 404 => (),
            Err(e) => return Err(e.into()),
        }
    }

    // Templates have no owner — pass None for the user-id arg.
    let assistant = Repos.assistant.create(None, plan.create_request).await?;

    // Track with `created_by: None` so /hub/updates surfaces this as
    // a system-wide install (the `is_template_install` flag on the
    // outdated row then routes the Re-install UI through the
    // template endpoint).
    //
    // Partial unique index `uniq_hub_template_install` (migration 79)
    // is the last-line backstop against the TOCTOU race where two
    // admins both passed the `find_template_install` check above
    // concurrently. If the insert hits that index, we delete the
    // orphan assistants row we just created (rolling back the partial
    // state) and return 409 — matches the fast-path error code so
    // clients see a consistent contract regardless of which
    // serialization layer caught the dup.
    let hub_tracking = match Repos
        .hub
        .track_hub_entity(
            HubEntityType::Assistant,
            assistant.id,
            &request.hub_id,
            HubCategory::Assistant,
            None,
            plan.hub_version.as_deref(),
        )
        .await
    {
        Ok(t) => t,
        Err(e) if e.status_code() == 409 => {
            let _ = Repos.assistant.delete(assistant.id).await;
            event_bus
                .emit(AssistantEvent::deleted(assistant.id, None))
                .await;
            return Err(AppError::conflict("Hub assistant template").into());
        }
        Err(e) => return Err(e.into()),
    };

    event_bus.emit_async(
        HubEvent::assistant_created_from_hub(
            assistant.id,
            request.hub_id.clone(),
            true,
        )
        .into(),
    );

    Ok((
        StatusCode::CREATED,
        Json(AssistantFromHubResponse {
            assistant,
            hub_tracking,
        }),
    ))
}

// =====================================================
// MCP SERVER FROM HUB
// =====================================================

/// Output of `build_mcp_server_create_from_hub` — bundles the typed
/// create-request the caller passes to `Repos.mcp.create_*_server`
/// with the catalog version that the same lookup resolved against.
/// Same TOCTOU rationale as `HubAssistantCreatePlan`: capture once,
/// pass through to `track_hub_entity` so a concurrent
/// `/hub/activate` swap between lookup and insert can't drift the
/// `hub_entities.hub_version` stamp.
struct HubMcpServerCreatePlan {
    create_request: crate::modules::mcp::CreateMcpServerRequest,
    hub_version: Option<String>,
}

/// Shared lookup + validation for both hub MCP server install paths
/// (user / system). Mirrors `build_assistant_create_from_hub` — but
/// MCP doesn't need an `is_system` discriminator on the result
/// because the scope is decided by which `Repos.mcp.create_*_server`
/// method the caller invokes (the assistants table uses a column
/// flag; mcp_servers uses two distinct insert paths).
async fn build_mcp_server_create_from_hub(
    request: &CreateMcpServerFromHubRequest,
) -> Result<HubMcpServerCreatePlan, AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;
    // Capture the catalog version up front so the same version stamps
    // the `hub_entities` row downstream — guards against a concurrent
    // /hub/activate swap between this lookup and the tracking insert.
    let hub_version = hub_manager.current_version().await.ok();

    let hub_server = hub_data
        .mcp_servers
        .into_iter()
        .find(|s| s.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub MCP server '{}'", request.hub_id)))?;

    // Defense-in-depth: reject incompatible items (min_ziee_version >
    // server). The UI hides these in the catalog; this is the
    // backstop for a direct API call.
    hub_manager
        .ensure_installable(HubCategory::McpServer, &request.hub_id)
        .await?;

    let transport_type = hub_server
        .transport_type
        .as_ref()
        .and_then(|t| match t.as_str() {
            "stdio" => Some(crate::modules::mcp::TransportType::Stdio),
            "sse" => Some(crate::modules::mcp::TransportType::Sse),
            "http" => Some(crate::modules::mcp::TransportType::Http),
            _ => None,
        })
        .unwrap_or(crate::modules::mcp::TransportType::Stdio);

    // Seed env + header maps from the catalog values, then merge
    // `required_*` placeholders for any key the catalog left empty.
    // This is the whole point of the schema addition: without the
    // merge, an empty `GITHUB_TOKEN: ""` in the manifest would land
    // verbatim in the user's MCP row and they'd have no signal that
    // configuration is needed. With the merge, the user sees
    // `GITHUB_TOKEN: ghp_xxxxxxxx...` (the placeholder) and knows
    // exactly what to replace.
    //
    // `.entry().or_insert_with(...)` skips keys the catalog already
    // pre-filled with a concrete example (e.g. postgres-mcp ships a
    // sample connection string) and keys the user already supplied
    // via the request override path (future extension).
    let mut env_map: std::collections::HashMap<String, String> = hub_server
        .environment_variables
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    for req in &hub_server.required_env {
        let placeholder = req.placeholder.clone().unwrap_or_default();
        env_map
            .entry(req.name.clone())
            .and_modify(|existing| {
                // Treat empty-string entries (the legacy "this is
                // required" convention) as also needing the seed —
                // otherwise the new schema's placeholder wouldn't
                // surface until the manifest also drops the empty
                // string. Non-empty existing values are respected
                // (e.g. postgres-mcp's example connection string).
                if existing.is_empty() {
                    *existing = placeholder.clone();
                }
            })
            .or_insert_with(|| placeholder);
    }

    let mut headers_map: std::collections::HashMap<String, String> = hub_server
        .headers
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    for req in &hub_server.required_headers {
        let placeholder = req.placeholder.clone().unwrap_or_default();
        headers_map
            .entry(req.name.clone())
            .and_modify(|existing| {
                if existing.is_empty() {
                    *existing = placeholder.clone();
                }
            })
            .or_insert_with(|| placeholder);
    }

    let create_request = crate::modules::mcp::CreateMcpServerRequest {
        name: request.name.clone().unwrap_or(hub_server.name.clone()),
        display_name: request
            .display_name
            .clone()
            .unwrap_or(hub_server.display_name.clone()),
        description: hub_server.description.clone(),
        enabled: Some(request.enabled),
        transport_type,
        command: hub_server.command.clone(),
        args: hub_server.args.clone(),
        environment_variables: Some(env_map),
        url: hub_server.url.clone(),
        headers: Some(headers_map),
        timeout_seconds: Some(if hub_server.supports_sampling == Some(true) { 300 } else { 30 }),
        supports_sampling: hub_server.supports_sampling,
        usage_mode: None,
        max_concurrent_sessions: None,
        // Hub installs don't surface the sandbox option in the UI;
        // the option only honors admin/system servers when set
        // explicitly via the native admin form.
        run_in_sandbox: None,
    };

    // Validation MUST run before any DB write so the `replace_existing`
    // re-install path doesn't delete the prior system MCP server and
    // then fail on transport validation, leaving the admin with no
    // system server for this hub_id. The native create path runs this
    // inside `create_*_server`; calling explicitly here hoists it
    // above the delete.
    crate::modules::mcp::validate_transport_config(
        &create_request.transport_type,
        &create_request,
    )?;

    Ok(HubMcpServerCreatePlan {
        create_request,
        hub_version,
    })
}

/// Create a USER-scoped MCP server from the hub catalog.
#[debug_handler]
pub async fn create_mcp_server_from_hub(
    auth: RequirePermissions<(HubMcpServersCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateMcpServerFromHubRequest>,
) -> ApiResult<Json<McpServerFromHubResponse>> {
    // `replace_existing` is system-only — reject explicitly so
    // clients don't silently pass it expecting an idempotent
    // re-install on the user-scoped path (per-user installs aren't
    // dedup'd). Mirrors the user-assistant handler.
    if request.replace_existing {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "replace_existing is only valid on the system MCP install endpoint",
        )
        .into());
    }

    let plan = build_mcp_server_create_from_hub(&request).await?;

    let server = Repos
        .mcp
        .create_user_server(auth.user.id, plan.create_request)
        .await?;

    // Track in hub_entities, stamping the catalog version captured by
    // the lookup so /hub/updates can detect when this row falls
    // behind a future catalog activation.
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::McpServer,
            server.id,
            &request.hub_id,
            HubCategory::McpServer,
            Some(auth.user.id),
            plan.hub_version.as_deref(),
        )
        .await?;

    event_bus.emit_async(
        HubEvent::mcp_server_created_from_hub(
            server.id,
            request.hub_id.clone(),
            false,
        )
        .into(),
    );

    Ok((
        StatusCode::CREATED,
        Json(McpServerFromHubResponse {
            server,
            hub_tracking,
        }),
    ))
}

/// Create a SYSTEM-WIDE MCP server from the hub catalog.
/// `is_system=true` + `user_id=NULL` per the mcp_servers table CHECK
/// constraint (migration 7: `system_server_must_have_no_owner`).
/// Permission gate is the intersection of "can install from the hub"
/// + "can author system MCP servers" — admins typically have both.
#[debug_handler]
pub async fn create_system_mcp_server_from_hub(
    _auth: RequirePermissions<(HubMcpServersCreate, McpServersAdminCreate)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateMcpServerFromHubRequest>,
) -> ApiResult<Json<McpServerFromHubResponse>> {
    // Idempotency guard: look up the existing system install (if any)
    // but DON'T delete yet. The delete is deferred until AFTER the
    // catalog lookup + validation succeeds so a failing re-install
    // (e.g. catalog raised min_ziee_version, transport config is now
    // invalid) doesn't leave the admin with the prior system server
    // wiped and no replacement.
    let existing_id = Repos.hub.find_system_mcp_install(&request.hub_id).await?;
    if existing_id.is_some() && !request.replace_existing {
        return Err(AppError::conflict("Hub MCP system server").into());
    }

    // Carry forward the prior system server's admin-tunable runtime
    // fields on the `replace_existing` re-install path so a Re-install
    // doesn't silently undo the admin's prior promotions:
    //   - `enabled` — previously-disabled servers stay disabled
    //   - `run_in_sandbox` — bwrap hardening posture survives
    //   - `usage_mode` — `always`/`never` overrides survive
    //   - `max_concurrent_sessions` — session caps survive
    //   - `timeout_seconds` — admin-tuned timeouts survive
    //   - `environment_variables` — real tokens / connection strings
    //     the admin pasted (replacing the install-time placeholders)
    //     survive instead of being stomped back to placeholders
    //   - `headers` — same as env vars but for HTTP header values
    //     taking direct user input (`required_headers` schema entries)
    // Without these, an admin who'd hardened a hub-installed system
    // server (e.g. flipped `run_in_sandbox=true`) or pasted a real
    // token into the env map would see their change silently reverted
    // to the catalog default / placeholder on Re-install.
    //
    // Caveat — `supports_sampling` is NOT carried forward (it's a
    // catalog-declared capability, not admin-tunable). If a catalog
    // upgrade flips `supports_sampling` from false→true for the same
    // hub_id, the carried-forward `timeout_seconds=30` (the
    // non-sampling default) will pair with the new sampling-capable
    // server and may cut off long-running sampling calls. Admin
    // should re-tune the timeout after such an upgrade — flagged in
    // the release notes for the catalog version that flips the flag.
    let mut plan = build_mcp_server_create_from_hub(&request).await?;
    if let Some(existing_id) = existing_id {
        if let Some(prior) = Repos.mcp.get_any_server(existing_id).await? {
            plan.create_request.enabled = Some(prior.enabled);
            plan.create_request.run_in_sandbox = Some(prior.run_in_sandbox);
            plan.create_request.usage_mode = Some(prior.usage_mode);
            plan.create_request.max_concurrent_sessions =
                prior.max_concurrent_sessions;
            plan.create_request.timeout_seconds = Some(prior.timeout_seconds);
            // The model stores env / headers as `serde_json::Value`
            // (the on-disk JSONB column); the create request takes
            // typed `Option<HashMap<String, String>>`. Round-trip
            // through serde_json::from_value; failure (malformed
            // historical row) falls back to the catalog defaults
            // from `build_mcp_server_create_from_hub` rather than
            // crashing the re-install.
            //
            // MERGE (not replace) — start with the helper-seeded map
            // (catalog defaults + placeholders for any NEW
            // `required_*` keys the catalog added between installs)
            // and overlay the prior row's values. Prior values win
            // for keys the admin set; catalog wins for newly-added
            // keys the admin hasn't seen. Without this merge, a
            // catalog upgrade that adds `WORKSPACE_ID` to a server
            // that previously only needed `API_KEY` would silently
            // drop the new placeholder on Re-install — the admin
            // would have no UI signal that new configuration is
            // needed.
            if let Ok(prior_env) = serde_json::from_value::<
                std::collections::HashMap<String, String>,
            >(prior.environment_variables.clone())
            {
                let mut merged = plan
                    .create_request
                    .environment_variables
                    .take()
                    .unwrap_or_default();
                for (k, v) in prior_env {
                    merged.insert(k, v);
                }
                plan.create_request.environment_variables = Some(merged);
            }
            if let Ok(prior_hdrs) = serde_json::from_value::<
                std::collections::HashMap<String, String>,
            >(prior.headers.clone())
            {
                let mut merged = plan
                    .create_request
                    .headers
                    .take()
                    .unwrap_or_default();
                for (k, v) in prior_hdrs {
                    merged.insert(k, v);
                }
                plan.create_request.headers = Some(merged);
            }
        }
    }

    // Validation passed — now delete the prior system server (if any)
    // and emit `McpServerEvent::SystemServerDeleted` so the hub
    // module's `CleanupHubEntitiesHandler` removes the orphan
    // `hub_entities` row. There is NO FK cascade between
    // `hub_entities.entity_id` and `mcp_servers.id`; cleanup is
    // event-driven.
    if let Some(existing_id) = existing_id {
        // Tolerate "already deleted" — racy with the admin MCP page
        // deleting the same row in another tab. Any other DB error
        // still surfaces.
        match Repos.mcp.delete_system_server(existing_id).await {
            Ok(()) => {
                event_bus
                    .emit(crate::modules::mcp::events::McpServerEvent::system_server_deleted(
                        existing_id,
                    ))
                    .await;
            }
            Err(e) if e.status_code() == 404 => (),
            Err(e) => return Err(e.into()),
        }
    }

    let server = Repos.mcp.create_system_server(plan.create_request).await?;

    // Track with `created_by: None` so /hub/updates surfaces this as
    // a system-wide install (the `is_system_mcp_install` flag on the
    // outdated row then routes the Re-install UI through this
    // endpoint).
    //
    // Partial unique index `uniq_hub_system_mcp_install` (migration
    // 80) is the last-line backstop against the TOCTOU race where
    // two admins both passed the `find_system_mcp_install` check
    // above concurrently. If the insert hits that index, we delete
    // the orphan mcp_servers row we just created (rolling back the
    // partial state) and return 409 — matches the fast-path error
    // code so clients see a consistent contract regardless of which
    // serialization layer caught the dup.
    let hub_tracking = match Repos
        .hub
        .track_hub_entity(
            HubEntityType::McpServer,
            server.id,
            &request.hub_id,
            HubCategory::McpServer,
            None,
            plan.hub_version.as_deref(),
        )
        .await
    {
        Ok(t) => t,
        Err(e) if e.status_code() == 409 => {
            let _ = Repos.mcp.delete_system_server(server.id).await;
            event_bus
                .emit(crate::modules::mcp::events::McpServerEvent::system_server_deleted(
                    server.id,
                ))
                .await;
            return Err(AppError::conflict("Hub MCP system server").into());
        }
        Err(e) => return Err(e.into()),
    };

    event_bus.emit_async(
        HubEvent::mcp_server_created_from_hub(
            server.id,
            request.hub_id.clone(),
            true,
        )
        .into(),
    );

    Ok((
        StatusCode::CREATED,
        Json(McpServerFromHubResponse {
            server,
            hub_tracking,
        }),
    ))
}

// =====================================================
// MODEL FROM HUB
// =====================================================

/// Create model download from hub catalog.
///
/// SECURITY: requires BOTH hub::models::create AND llm_models::create.
/// The handler routes to `initiate_repository_download_internal` which
/// bypasses the llm_models::create permission check that the
/// equivalent /llm-models/download endpoint applies — so without the
/// added LlmModelsCreate requirement here, anyone with just
/// hub::models::create could create models via this back-door. Closes
/// 11-hub F-05 (Medium).
#[debug_handler]
pub async fn create_model_from_hub(
    _auth: RequirePermissions<(HubModelsCreate, LlmModelsCreate)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateModelFromHubRequest>,
) -> ApiResult<Json<ModelFromHubResponse>> {
    use crate::modules::llm_model::models::FileFormat as LlmFileFormat;

    // 1. Load hub model
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let hub_data = hub_manager.load_hub_data_with_locale("en").await?;

    let hub_model = hub_data
        .models
        .into_iter()
        .find(|m| m.id == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub model '{}'", request.hub_id)))?;

    // 1b. Reject incompatible items (min_ziee_version > server).
    hub_manager
        .ensure_installable(HubCategory::Model, &request.hub_id)
        .await?;

    // 2. Find repository by URL
    let repository = Repos
        .llm_repository
        .find_by_url(&hub_model.repository_url)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("Database error: {}", e)),
            )
        })?
        .ok_or_else(|| {
            AppError::not_found(&format!(
                "Repository with URL '{}' not found",
                hub_model.repository_url
            ))
        })?;

    // 2b. Block early with clear guidance when this model needs auth but the
    // source repository has no credential configured. Without this the download
    // is spawned and only fails later in the background with an opaque git auth
    // error. Enforced server-side; the UI mirrors it via `source_auth_configured`.
    if hub_model.auth_required && !repository.has_credential() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            AppError::unprocessable_entity(
                "HUB_REPOSITORY_AUTH_NOT_CONFIGURED",
                format!(
                    "Downloading \"{}\" requires authentication for the \"{}\" repository, \
                     but no credential is configured. Add it in Settings → LLM Repositories.",
                    hub_model.display_name, repository.name
                ),
            )
            .with_details(serde_json::json!({
                "repository_id": repository.id,
                "repository_name": repository.name,
                "settings_path": "/settings/llm-repositories",
            })),
        ));
    }

    // 3. Select quantization option if specified
    let main_filename = if let Some(ref quant_name) = request.quantization_name {
        hub_model
            .quantization_options
            .as_ref()
            .and_then(|opts| {
                opts.iter()
                    .find(|o| &o.name == quant_name)
                    .map(|opt| opt.main_filename.clone())
            })
            .unwrap_or_else(|| hub_model.main_filename.clone())
    } else {
        hub_model.main_filename.clone()
    };

    // 4. Convert FileFormat from hub to llm_model
    let file_format = match hub_model.file_format {
        super::models::FileFormat::GGUF => LlmFileFormat::Gguf,
        super::models::FileFormat::SafeTensors => LlmFileFormat::Safetensors,
        super::models::FileFormat::PyTorch => LlmFileFormat::Pytorch,
    };

    // 5. Convert capabilities from hub to llm_model format
    let capabilities = hub_model.capabilities.map(|hub_caps| {
        crate::modules::llm_model::models::ModelCapabilities {
            vision: Some(hub_caps.vision),
            audio: Some(hub_caps.audio),
            tools: Some(hub_caps.tools),
            code_interpreter: Some(hub_caps.code_interpreter),
            chat: Some(hub_caps.chat),
            text_embedding: Some(hub_caps.text_embedding),
            image_generator: Some(hub_caps.image_generator),
        }
    });

    // 6. Build download request for initiate_repository_download
    let download_request = crate::modules::llm_model::handlers::uploads::DownloadFromRepositoryRequest {
        provider_id: request.provider_id,
        repository_id: repository.id,
        repository_path: hub_model.repository_path.clone(),
        repository_branch: None,
        name: hub_model.name.clone(),
        display_name: request
            .display_name
            .unwrap_or_else(|| hub_model.display_name.clone()),
        description: hub_model.description.clone(),
        file_format,
        main_filename,
        capabilities,
        parameters: hub_model
            .recommended_parameters
            .and_then(|p| serde_json::from_value(p).ok()),
        engine_type: hub_model
            .recommended_engine
            .and_then(|e| crate::modules::llm_model::models::EngineType::from_str(&e)),
        engine_settings: hub_model
            .recommended_engine_settings
            .and_then(|s| serde_json::from_value(s).ok()),
    };

    // 7. Initiate the actual download (this creates the download instance AND spawns the background task)
    let download = crate::modules::llm_model::handlers::uploads::initiate_repository_download_internal(
        download_request,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Failed to initiate download: {}", e)),
        )
    })?;

    // 8. Track in hub_entities (stamp the installed catalog version).
    let hub_version = hub_manager.current_version().await.ok();
    let hub_tracking = Repos
        .hub
        .track_hub_entity(
            HubEntityType::LlmModel,
            download.id,
            &request.hub_id,
            HubCategory::Model,
            None, // Models are system-wide, not user-specific
            hub_version.as_deref(),
        )
        .await?;

    // 9. Emit event
    event_bus.emit_async(
        HubEvent::model_download_started_from_hub(download.id, request.hub_id.clone()).into(),
    );

    // 10. Return response
    Ok((
        StatusCode::CREATED,
        Json(ModelFromHubResponse {
            download,
            hub_tracking,
        }),
    ))
}

// =====================================================
// OpenAPI Documentation
// =====================================================

pub fn get_hub_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getModels")
        .tag("Hub")
        .summary("Get hub models with locale support")
        .response::<200, Json<HubModelsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsRead,)>(op)
        .id("Hub.getAssistants")
        .tag("Hub")
        .summary("Get hub assistants with locale support")
        .response::<200, Json<HubAssistantsResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_mcp_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersRead,)>(op)
        .id("Hub.getMCPServers")
        .tag("Hub")
        .summary("Get hub MCP servers with locale support")
        .response::<200, Json<HubMCPServersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_models_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsVersionRead,)>(op)
        .id("Hub.getModelsVersion")
        .tag("Hub")
        .summary("Get hub models version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_assistants_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsVersionRead,)>(op)
        .id("Hub.getAssistantsVersion")
        .tag("Hub")
        .summary("Get hub assistants version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn get_hub_mcp_servers_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersVersionRead,)>(op)
        .id("Hub.getMCPServersVersion")
        .tag("Hub")
        .summary("Get hub MCP servers version information")
        .response::<200, Json<HubVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

pub fn refresh_hub_models_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRefresh,)>(op)
        .id("Hub.refreshModels")
        .tag("Hub")
        .summary("Refresh hub models from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn refresh_hub_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsRefresh,)>(op)
        .id("Hub.refreshAssistants")
        .tag("Hub")
        .summary("Refresh hub assistants from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn refresh_hub_mcp_servers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMCPServersRefresh,)>(op)
        .id("Hub.refreshMCPServers")
        .tag("Hub")
        .summary("Refresh hub MCP servers from GitHub")
        .response::<200, Json<HubRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("Failed to refresh hub data"))
}

pub fn create_assistant_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsCreate,)>(op)
        .id("Hub.createAssistantFromHub")
        .tag("Hub")
        .summary("Create assistant from hub catalog")
        .response::<201, Json<AssistantFromHubResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description(
                "Validation error (empty name, oversized description / instructions, \
                 or `replace_existing` passed on the user-scoped endpoint)",
            )
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub assistant not found"))
        .response_with::<422, (), _>(|res| {
            res.description("Hub item incompatible with this server version")
        })
}

pub fn create_assistant_template_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubAssistantsCreate, AssistantsTemplateCreate)>(op)
        .id("Hub.createAssistantTemplateFromHub")
        .tag("Hub")
        .tag("Assistant Templates")
        .summary("Create assistant TEMPLATE from hub catalog")
        .description(
            "Installs a hub assistant entry as a system-wide template \
             (`is_template=true, created_by=NULL`) rather than a personal \
             user assistant. Requires both `hub::assistants::create` and \
             `assistant_templates::create` permissions. Returns 409 when \
             a template install for this `hub_id` already exists, unless \
             `replace_existing: true` is passed to overwrite it.",
        )
        .response::<201, Json<AssistantFromHubResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description(
                "Validation error (empty name, oversized description / instructions)",
            )
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub assistant not found"))
        .response_with::<409, (), _>(|res| {
            res.description("Template install already exists for this hub_id")
        })
        .response_with::<422, (), _>(|res| {
            res.description("Hub item incompatible with this server version")
        })
}

pub fn create_mcp_server_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMcpServersCreate,)>(op)
        .id("Hub.createMcpServerFromHub")
        .tag("Hub")
        .summary("Create MCP server from hub catalog")
        .response::<201, Json<McpServerFromHubResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description(
                "Validation error (invalid transport config, or \
                 `replace_existing` passed on the user-scoped endpoint)",
            )
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub MCP server not found"))
        .response_with::<422, (), _>(|res| {
            res.description("Hub item incompatible with this server version")
        })
}

pub fn create_system_mcp_server_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubMcpServersCreate, McpServersAdminCreate)>(op)
        .id("Hub.createSystemMcpServerFromHub")
        .tag("Hub")
        .tag("MCP Servers - System")
        .summary("Create SYSTEM-WIDE MCP server from hub catalog")
        .description(
            "Installs a hub MCP server entry as a system-wide server \
             (`is_system=true, user_id=NULL`) rather than a personal \
             user MCP server. Requires both `hub::mcp_servers::create` \
             and `mcp_servers_admin::create` permissions. Returns 409 \
             when a system install for this `hub_id` already exists, \
             unless `replace_existing: true` is passed to overwrite \
             it. On `replace_existing` the prior server's `enabled` \
             flag is carried forward.",
        )
        .response::<201, Json<McpServerFromHubResponse>>()
        .response_with::<400, (), _>(|res| {
            res.description("Validation error (invalid transport config)")
        })
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub MCP server not found"))
        .response_with::<409, (), _>(|res| {
            res.description("System MCP install already exists for this hub_id")
        })
        .response_with::<422, (), _>(|res| {
            res.description("Hub item incompatible with this server version")
        })
}

pub fn create_model_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsCreate, LlmModelsCreate)>(op)
        .id("Hub.createModelFromHub")
        .tag("Hub")
        .summary("Download model from hub catalog")
        .response::<201, Json<ModelFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub model not found"))
        .response_with::<422, (), _>(|res| {
            res.description("Hub item incompatible with this server version")
        })
}

// =====================================================
// LOCAL PROVIDERS FOR HUB DOWNLOADS
// =====================================================

/// List enabled local providers available as download targets for hub models
#[debug_handler]
pub async fn get_hub_local_providers(
    _auth: RequirePermissions<(HubModelsCreate,)>,
) -> ApiResult<Json<HubLocalProvidersResponse>> {
    let providers = Repos.llm_provider.list_local_providers().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AppError::internal_error(format!("Database error: {}", e)),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(HubLocalProvidersResponse {
            providers: providers
                .into_iter()
                .map(|p| HubLocalProvider { id: p.id, name: p.name })
                .collect(),
        }),
    ))
}

pub fn get_hub_local_providers_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsCreate,)>(op)
        .id("Hub.getLocalProviders")
        .tag("Hub")
        .summary("List local providers available for hub model downloads")
        .response::<200, Json<HubLocalProvidersResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

// =====================================================
// UNIFIED CATALOG ENDPOINTS (new in Phase 1)
// =====================================================

/// GET /api/hub/index — return the full parsed catalog (flat across
/// all categories). Cheap: reads ~6 KB of JSON. The Phase-2 frontend
/// will load this once per session and client-side-filter into the
/// existing three tabs.
#[debug_handler]
pub async fn get_hub_catalog(
    _auth: RequirePermissions<(HubModelsRead,)>,
) -> ApiResult<Json<Catalog>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    Ok((StatusCode::OK, Json(catalog)))
}

pub fn get_hub_catalog_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getCatalog")
        .tag("Hub")
        .summary("Get the unified hub catalog (index.json)")
        .response::<200, Json<Catalog>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/hub/version — catalog hub_version, the running server's
/// own semver (for client-side compat filtering), per-category counts.
#[debug_handler]
pub async fn get_hub_catalog_version(
    _auth: RequirePermissions<(HubModelsRead,)>,
) -> ApiResult<Json<HubCatalogVersionResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    let mut counts = HubCatalogCounts {
        models: 0,
        assistants: 0,
        mcp_servers: 0,
    };
    for item in &catalog.items {
        match item.category {
            HubCategory::Model => counts.models += 1,
            HubCategory::Assistant => counts.assistants += 1,
            HubCategory::McpServer => counts.mcp_servers += 1,
        }
    }
    Ok((
        StatusCode::OK,
        Json(HubCatalogVersionResponse {
            hub_version: catalog.hub_version,
            server_version: super::hub_manager::server_version().to_string(),
            counts,
            source: hub_manager.provenance(),
            last_refreshed: hub_manager.last_refreshed().map(|t| t.to_rfc3339()),
        }),
    ))
}

pub fn get_hub_catalog_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getCatalogVersion")
        .tag("Hub")
        .summary("Current hub catalog version + server version + counts")
        .response::<200, Json<HubCatalogVersionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// POST /api/hub/refresh — admin-only force fetch from GitHub.
/// Respects the admin-pinned version (hub_settings.pinned_version):
/// pinned → re-fetch that exact version; unpinned → fetch latest.
/// Cosign + sha256 failure leaves the previous catalog in place.
#[debug_handler]
pub async fn refresh_hub_catalog(
    _auth: RequirePermissions<(HubCatalogManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
) -> ApiResult<Json<HubCatalogRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let pinned = Repos.hub.get_pinned_version().await?;
    let outcome = hub_manager.refresh(pinned).await?;

    if outcome.updated {
        // Reuse the existing per-category events so any listener wired
        // to one of them still picks up the change. The new catalog is
        // unified — three identical events emit at once.
        let prev = outcome.previous_version.clone().unwrap_or_default();
        event_bus.emit_async(
            HubEvent::models_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::assistants_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::mcp_servers_refreshed(prev, outcome.new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubCatalogRefreshResponse {
            updated: outcome.updated,
            previous_version: outcome.previous_version,
            new_version: outcome.new_version,
            cosign_verified: outcome.cosign_verified,
        }),
    ))
}

pub fn refresh_hub_catalog_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubCatalogManage,)>(op)
        .id("Hub.refreshCatalog")
        .tag("Hub")
        .summary("Force-refresh the hub catalog from GitHub Releases (admin only)")
        .response::<200, Json<HubCatalogRefreshResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| {
            res.description("Fetch / sha256 / cosign verify failure — previous catalog left in place")
        })
}

/// GET /api/hub/updates — admin-only. Installed entities whose
/// `hub_version` lags the catalog. NULL `hub_version` (legacy rows)
/// counts as behind.
#[debug_handler]
pub async fn get_hub_updates(
    _auth: RequirePermissions<(HubCatalogRead,)>,
) -> ApiResult<Json<HubUpdatesResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;
    let rows = Repos
        .hub
        .list_outdated_entities(&catalog.hub_version)
        .await?;
    Ok((
        StatusCode::OK,
        Json(HubUpdatesResponse {
            catalog_version: catalog.hub_version.clone(),
            updates: rows
                .into_iter()
                .map(|r| HubUpdateRow {
                    hub_id: r.hub_id,
                    hub_category: r.hub_category,
                    entity_type: r.entity_type,
                    entity_id: r.entity_id,
                    installed_version: r.installed_version,
                    current_version: catalog.hub_version.clone(),
                    is_template_install: r.is_template_install,
                    is_system_mcp_install: r.is_system_mcp_install,
                })
                .collect(),
        }),
    ))
}

pub fn get_hub_updates_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubCatalogRead,)>(op)
        .id("Hub.getUpdates")
        .tag("Hub")
        .summary("Installed hub entities behind the current catalog version (admin only)")
        .response::<200, Json<HubUpdatesResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// GET /api/hub/manifest/:id?category=... — full YAML manifest for one
/// item. Backs the detail-drawer view in the Phase-2 frontend so the
/// list view can stay small (just the index entries).
#[debug_handler]
pub async fn get_hub_manifest(
    _auth: RequirePermissions<(HubModelsRead,)>,
    AxumPath(id): AxumPath<String>,
    Query(q): Query<HubManifestQuery>,
) -> ApiResult<Json<HubManifest>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let manifest = hub_manager.manifest(q.category, &id).await?;
    Ok((StatusCode::OK, Json(manifest)))
}

pub fn get_hub_manifest_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubModelsRead,)>(op)
        .id("Hub.getManifest")
        .tag("Hub")
        .summary("Full manifest for one hub item (model / assistant / mcp-server)")
        .response::<200, Json<HubManifest>>()
        .response_with::<400, (), _>(|res| res.description("Invalid id"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Manifest not found in catalog"))
}

/// GET /api/hub/releases — admin-only. Lists catalog versions published
/// on GitHub Releases (newest first), marking the active (currently
/// installed) one + the admin's pin.
#[debug_handler]
pub async fn get_hub_releases(
    _auth: RequirePermissions<(HubCatalogRead,)>,
) -> ApiResult<Json<HubReleasesResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let releases = hub_manager.list_releases().await?;
    let active_version = hub_manager.catalog().await.ok().map(|c| c.hub_version);
    let pinned_version = Repos.hub.get_pinned_version().await?;
    Ok((
        StatusCode::OK,
        Json(HubReleasesResponse {
            active_version,
            pinned_version,
            releases,
        }),
    ))
}

pub fn get_hub_releases_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubCatalogRead,)>(op)
        .id("Hub.getReleases")
        .tag("Hub")
        .summary("List catalog versions published on GitHub (admin only)")
        .response::<200, Json<HubReleasesResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| res.description("GitHub unreachable"))
}

/// POST /api/hub/activate — admin-only. Pin a specific catalog version
/// (or clear the pin to track latest, by sending `version: null`),
/// then fetch + verify + rotate `current/` to it. Server-wide: every
/// user sees the activated version. Cosign / sha256 failure leaves the
/// previous catalog in place AND does not persist the pin.
#[debug_handler]
pub async fn activate_hub_version(
    _auth: RequirePermissions<(HubCatalogManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<ActivateHubVersionRequest>,
) -> ApiResult<Json<HubCatalogRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;

    // Fetch + verify + rotate FIRST. Only persist the pin if it
    // succeeds — otherwise an admin could pin a bad/yanked version and
    // brick every subsequent refresh.
    let outcome = hub_manager.refresh(request.version.clone()).await?;
    Repos
        .hub
        .set_pinned_version(request.version.as_deref())
        .await?;

    if outcome.updated {
        let prev = outcome.previous_version.clone().unwrap_or_default();
        event_bus.emit_async(
            HubEvent::models_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::assistants_refreshed(prev.clone(), outcome.new_version.clone()).into(),
        );
        event_bus.emit_async(
            HubEvent::mcp_servers_refreshed(prev, outcome.new_version.clone()).into(),
        );
    }

    Ok((
        StatusCode::OK,
        Json(HubCatalogRefreshResponse {
            updated: outcome.updated,
            previous_version: outcome.previous_version,
            new_version: outcome.new_version,
            cosign_verified: outcome.cosign_verified,
        }),
    ))
}

pub fn activate_hub_version_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(HubCatalogManage,)>(op)
        .id("Hub.activateVersion")
        .tag("Hub")
        .summary("Pin + activate a catalog version server-wide (admin only)")
        .response::<200, Json<HubCatalogRefreshResponse>>()
        .response_with::<400, (), _>(|res| res.description("Invalid version string"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<500, (), _>(|res| {
            res.description("Fetch / verify failure — pin not persisted, previous catalog kept")
        })
}

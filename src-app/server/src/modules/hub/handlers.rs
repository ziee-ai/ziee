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
        skill::{
            self,
            models::CreateSkill,
            permissions::{SkillsInstall, SkillsManageSystem},
        },
        sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
        workflow::{
            self,
            models::CreateWorkflow,
            permissions::{WorkflowsInstall, WorkflowsManageSystem},
        },
    },
};
use std::sync::Arc;
use uuid::Uuid;

use super::{
    events::HubEvent,
    hub_manager::{Catalog, HubManager, HubManifest},
    models::{HubCategory, HubEntity, HubEntityType},
    permissions::*,
    types::*,
};
use crate::modules::skill::types::{
    CreateSkillFromHubRequest, CreateSystemSkillFromHubRequest, SkillFromHubResponse,
};
use crate::modules::workflow::types::{
    CreateSystemWorkflowFromHubRequest, CreateWorkflowFromHubRequest, WorkflowFromHubResponse,
};
use axum::extract::Path as AxumPath;

/// H1: the per-owner on-disk path segment for a skill/workflow bundle.
/// User installs land under their UUID; system installs under "system".
/// This keeps user A's and user B's copies of the same hub item in
/// distinct dirs so one install's `remove_dir_all` can never clobber the
/// other's live bundle. The segment is a UUID or the literal "system" —
/// both path-safe (no separators / `..`).
fn owner_dir_segment(owner_user_id: Option<Uuid>) -> String {
    match owner_user_id {
        Some(uid) => uid.to_string(),
        None => "system".to_string(),
    }
}

/// Resolve the per-entry semver for one catalog item, used to stamp
/// `hub_entities.hub_version` on install. Returns `None` if the entry
/// is missing or has no `version` field set (legacy seed entries).
/// Falls back to the catalog build-marker `hub_version` so older
/// entries that haven't been re-published with the per-entry envelope
/// still surface SOMETHING in the Installed view rather than blank.
async fn resolve_entry_version(
    hub_manager: &HubManager,
    category: HubCategory,
    name: &str,
) -> Option<String> {
    let catalog = hub_manager.catalog().await.ok()?;
    let item = catalog
        .items
        .iter()
        .find(|it| it.category == category && it.name == name)?;
    item.version.clone().or_else(|| Some(catalog.hub_version.clone()))
}

/// Look up the human display title for a hub item via the catalog's
/// IndexItem. Used as the fallback when an install request doesn't
/// provide a `display_name`. Falls back to the leaf of the reverse-DNS
/// `name` if the catalog has no title set.
async fn resolve_entry_title(
    hub_manager: &HubManager,
    category: HubCategory,
    name: &str,
) -> String {
    if let Ok(catalog) = hub_manager.catalog().await
        && let Some(item) = catalog
            .items
            .iter()
            .find(|it| it.category == category && it.name == name)
        && let Some(t) = item.title.as_deref()
    {
        return t.to_string();
    }
    // Fallback: leaf of the reverse-DNS string.
    name.rsplit('/').next().unwrap_or(name).to_string()
}

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

    // Merge created_ids + source_auth_configured into models. v2:
    // catalog identity is the reverse-DNS `name`, which matches the
    // value stored in `hub_entities.hub_id`.
    //
    // `source_auth_configured` is computed PER MODEL but driven off
    // PER SOURCE env vars. For each source with at least one
    // `is_required + is_secret` env var, derive the matching
    // `llm_repositories.url` from the source's `registry_type`
    // (`huggingface` → `https://huggingface.co`, `s3` →
    // `https://s3.amazonaws.com`, `url` → the source identifier itself)
    // and check whether that repo has a credential present. The flag
    // is `true` if AT LEAST ONE source's required-secret credential is
    // configured — multi-source models work as soon as a single source
    // is usable.
    //
    // Sources without any required-secret env var (no auth needed)
    // count as auth-satisfied: a public HuggingFace mirror doesn't
    // need a token, so a model whose only source is that mirror has
    // `source_auth_configured = true`.
    let mut models = hub_data.models;
    for model in &mut models {
        model.created_ids = created_map.get(&model.name).cloned().unwrap_or_default();
        let configured = if model.sources.is_empty() {
            false
        } else {
            model.sources.iter().any(|source| {
                let needs_auth = source
                    .environment_variables
                    .iter()
                    .any(|e| e.is_required.unwrap_or(false) && e.is_secret);
                if !needs_auth {
                    return true;
                }
                let repo_url = match derive_registry_url(source) {
                    Some(u) => u,
                    None => return false,
                };
                cred_by_url.get(&repo_url).copied().unwrap_or(false)
            })
        };
        model.source_auth_configured = configured;
    }

    Ok((StatusCode::OK, Json(models)))
}

/// Derive the `llm_repositories.url` for one `ModelSource` based on its
/// `registry_type`. Mirrors the same mapping used by `create_model_from_hub`
/// to look up the repo row at install time — the two MUST agree, else a
/// model would surface `source_auth_configured = true` but fail at the
/// download gate (or vice versa).
fn derive_registry_url(source: &super::models::ModelSource) -> Option<String> {
    match source.registry_type.as_str() {
        "huggingface" => Some("https://huggingface.co".to_string()),
        "s3" => Some("https://s3.amazonaws.com".to_string()),
        "url" => Some(source.identifier.clone()),
        _ => None,
    }
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

    // Merge created_ids + created_template_ids into assistants. v2:
    // hub_entities.hub_id stores the reverse-DNS `name`.
    let mut assistants = hub_data.assistants;
    for assistant in &mut assistants {
        assistant.created_ids = created_map
            .get(&assistant.name)
            .cloned()
            .unwrap_or_default();
        assistant.created_template_ids = template_map
            .get(&assistant.name)
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

    // Merge created_ids + created_system_ids into servers. v2:
    // hub_entities.hub_id stores the reverse-DNS `name`.
    let mut mcp_servers = hub_data.mcp_servers;
    for server in &mut mcp_servers {
        server.created_ids = created_map.get(&server.name).cloned().unwrap_or_default();
        server.created_system_ids = system_map
            .get(&server.name)
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
    // v2: refresh is index-only + per-entry versioning, no admin pin.
    hub_manager.refresh().await?;
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
    hub_manager.refresh().await?;
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
    hub_manager.refresh().await?;
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
    // v2: stamp the *per-entry* version into `hub_entities.hub_version`
    // (not the catalog-wide build marker). Resolved once here so a
    // concurrent refresh between this lookup and the tracking insert
    // can't drift the stamp.
    let hub_version =
        resolve_entry_version(&hub_manager, HubCategory::Assistant, &request.hub_id).await;

    let hub_assistant = hub_data
        .assistants
        .into_iter()
        .find(|a| a.name == request.hub_id)
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
    // `replace_existing` is the Updates-tab "upgrade my install"
    // path. Same shape as `create_mcp_server_from_hub` — when set,
    // delete the user's most-recent live install of the same hub_id
    // and emit `assistant.deleted` so the hub_entities orphan-cleanup
    // listener fires before the new row's `track_hub_entity` insert.
    // Without this, clicking Re-install on a user assistant row in
    // the Updates tab would create a NEW assistant + leave the OLD
    // one orphaned at the stale hub_version, so the row never drops
    // from `list_outdated_entities`.
    // Find ALL prior user installs (not just the most-recent) so a
    // user who accumulated duplicates from before the
    // replace_existing path existed gets brought back to a clean
    // single-install state on Re-install.
    let existing_ids: Vec<Uuid> = if request.replace_existing {
        Repos
            .hub
            .find_user_assistant_installs(&request.hub_id, auth.user.id)
            .await?
    } else {
        Vec::new()
    };

    let plan = build_assistant_create_from_hub(&request, false).await?;

    for existing_id in &existing_ids {
        match Repos.assistant.delete(*existing_id).await {
            Ok(()) => {
                event_bus
                    .emit(crate::modules::assistant::events::AssistantEvent::deleted(
                        *existing_id,
                        Some(auth.user.id),
                    ))
                    .await;
            }
            Err(e) if e.status_code() == 404 => (),
            Err(e) => return Err(e.into()),
        }
    }

    let assistant = Repos
        .assistant
        .create(Some(auth.user.id), plan.create_request)
        .await?;

    // Track in hub_entities, stamping the catalog version captured
    // by the lookup so /hub/installed can detect when this row falls
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

    // Track with `created_by: None` so /hub/installed surfaces this as
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
            // Roll back the just-created assistant. Best-effort, but log a
            // failure so an orphaned row isn't left behind silently.
            if let Err(del_err) = Repos.assistant.delete(assistant.id).await {
                tracing::warn!(
                    "hub: failed to roll back assistant {} after 409 conflict: {del_err}",
                    assistant.id
                );
            }
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
    // Per-entry version stamping (see resolve_entry_version above).
    let hub_version =
        resolve_entry_version(&hub_manager, HubCategory::McpServer, &request.hub_id).await;

    let hub_server = hub_data
        .mcp_servers
        .into_iter()
        .find(|s| s.name == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub MCP server '{}'", request.hub_id)))?;

    // Defense-in-depth: reject incompatible items (min_ziee_version >
    // server). The UI hides these in the catalog; this is the
    // backstop for a direct API call.
    hub_manager
        .ensure_installable(HubCategory::McpServer, &request.hub_id)
        .await?;

    // server.json mapping. Strict — drive everything off `remotes[]`
    // / `packages[]`, no flat-field fallback. Precedence:
    //   1. remotes[0] (streamable-http / sse) → Http / Sse + url + headers
    //   2. packages[0] (npm/pypi stdio)       → Stdio + npx/uvx argv
    //
    // The publisher filters packages to npm/pypi + npx/uvx at build, so
    // the consumer only sees launchable ones. Manifests with neither
    // populated are a publisher error — surface as 422.
    let transport_type;
    let derived_command: Option<String>;
    let derived_args: Option<Vec<String>>;
    let derived_url: Option<String>;
    let mut env_entries: Vec<crate::modules::mcp::EnvVarEntry> = Vec::new();
    let mut header_entries: Vec<crate::modules::mcp::HeaderEntry> = Vec::new();

    if let Some(remote) = hub_server.remotes.as_ref().and_then(|r| r.first()) {
        // Official spelling is `"streamable-http"` (kebab-case) | `"sse"`.
        // Map anything else as Http (forward compat for new variants).
        transport_type = match remote.transport_kind.as_str() {
            "sse" => crate::modules::mcp::TransportType::Sse,
            _ => crate::modules::mcp::TransportType::Http,
        };
        derived_url = Some(remote.url.clone());
        derived_command = None;
        derived_args = None;
        for h in &remote.headers {
            header_entries.push(crate::modules::mcp::HeaderEntry {
                key: h.name.clone(),
                value: h.value.clone(),
                is_secret: h.is_secret,
            });
        }
    } else if let Some(pkg) = hub_server.packages.as_ref().and_then(|p| p.first()) {
        // Build argv: runtimeArguments ++ [identifier@version] ++
        // packageArguments. The runtime command is `runtimeHint`; the
        // npx + uvx commands are already in `HOST_ALLOWED_COMMANDS`.
        transport_type = crate::modules::mcp::TransportType::Stdio;
        derived_command = pkg.runtime_hint.clone();
        let mut argv: Vec<String> = Vec::new();
        for a in &pkg.runtime_arguments {
            if let Some(v) = &a.value {
                argv.push(v.clone());
            }
        }
        // Package spec — npm uses `<name>@<version>`; pypi via uvx
        // accepts the same form (or bare `<name>`). Prefer the npm
        // shape since it covers the common npx case; uvx tolerates
        // a leading positional with no version.
        let spec = if pkg.version.is_empty() {
            pkg.identifier.clone()
        } else {
            format!("{}@{}", pkg.identifier, pkg.version)
        };
        argv.push(spec);
        for a in &pkg.package_arguments {
            if let Some(v) = &a.value {
                argv.push(v.clone());
            }
        }
        derived_args = Some(argv);
        derived_url = None;
        for ev in &pkg.environment_variables {
            env_entries.push(crate::modules::mcp::EnvVarEntry {
                key: ev.name.clone(),
                value: ev.value.clone().or_else(|| ev.default.clone()),
                is_secret: ev.is_secret,
            });
        }
    } else {
        return Err(AppError::unprocessable_entity(
            "HUB_MCP_NO_TRANSPORT",
            format!(
                "Hub MCP server '{}' has neither packages[] nor remotes[]",
                hub_server.name
            ),
        ));
    }

    // Resolve the user-facing slug (the `mcp_servers.name` row value:
    // `^[a-z0-9-]+$`) from the reverse-DNS `hub_server.name` (leaf
    // after the first `/`, normalized).
    let derived_slug = super::hub_manager::derive_mcp_slug(&hub_server.name);
    // Resolve the human display fallback from the catalog's IndexItem
    // title (set by the publisher's `_hub_curation.title`). Falls back
    // to the slug if the catalog has no title.
    let display_fallback = resolve_entry_title(
        &hub_manager,
        HubCategory::McpServer,
        &hub_server.name,
    )
    .await;

    let create_request = crate::modules::mcp::CreateMcpServerRequest {
        // Server slug — must match `^[a-z0-9-]+$`; derived from the
        // reverse-DNS leaf. Request override (manual rename at install
        // time) wins if provided.
        name: request.name.clone().unwrap_or(derived_slug),
        display_name: request
            .display_name
            .clone()
            .unwrap_or(display_fallback),
        description: hub_server.description.clone(),
        // Hub installs ALWAYS land disabled — most hub servers ship
        // with placeholder secrets the user has to configure before
        // they can connect, so auto-probing on install would just
        // toast a failure. The user opens the drawer, fills in their
        // tokens, and toggles the title Enabled Switch — that flow
        // runs the probe + auto-enables on success (see
        // `connection_health::enforce_on_update_transition`).
        // `request.enabled` is ignored here for the same reason.
        enabled: Some(false),
        transport_type,
        command: derived_command,
        args: derived_args,
        environment_variables_entries: Some(env_entries),
        url: derived_url,
        headers_entries: Some(header_entries),
        // The manifest does not carry a `supports_sampling` flag.
        // Default to 30s timeout; admins can raise this in the
        // settings drawer.
        timeout_seconds: Some(30),
        supports_sampling: None,
        usage_mode: None,
        max_concurrent_sessions: None,
        // Hub installs don't surface the sandbox option in the UI;
        // the option only honors admin/system servers when set
        // explicitly via the native admin form.
        run_in_sandbox: None,
        // Sandbox flavor unset on hub installs. The DB column
        // defaults to 'full' (migration 83) for any system row the
        // admin later flips `run_in_sandbox=true` on. User-scope
        // hub installs route through `create_user_server` which
        // force-applies the active policy flavor regardless.
        sandbox_flavor: None,
        // Hub-scope tracking: store the reverse-DNS `name` in
        // `hub_entities.hub_id` so the Updates view + cleanup can
        // resolve back to the catalog entry.
        hub_id: Some(hub_server.name.clone()),
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
    // `replace_existing` is the Updates-tab "upgrade my user install"
    // path. When set, deletes the user's most-recent live install of
    // the same hub_id (filtered by created_by = this user) and emits
    // the `mcp_server.deleted` event so `CleanupHubEntitiesHandler`
    // removes the matching `hub_entities` row. Without this, clicking
    // Re-install on a user MCP row in the Updates tab would create a
    // NEW row at the current catalog version, leaving the OLD row in
    // place — `list_outdated_entities` would keep surfacing the old
    // row indefinitely, making the Updates tab appear broken.
    //
    // When false (default — the "Install" hub-card action), creates
    // an additional copy; a user CAN have multiple installs of the
    // same hub_id this way (each subsequent click adds another row).
    // The Updates-tab Re-install flow ALWAYS sets true to avoid that
    // staircase.
    // Find ALL prior user installs (not just the most-recent) so a
    // user who accumulated duplicates from before the
    // replace_existing path existed gets brought back to a clean
    // single-install state on Re-install. Without this, the older
    // duplicates stay at their stale `hub_version` and keep showing
    // up in `list_outdated_entities` even after a successful
    // Re-install of the newest copy.
    let existing_ids: Vec<Uuid> = if request.replace_existing {
        Repos
            .hub
            .find_user_mcp_installs(&request.hub_id, auth.user.id)
            .await?
    } else {
        Vec::new()
    };

    // Plan first so a failing lookup / validation doesn't wipe the
    // prior installs with no replacement.
    let mut plan = build_mcp_server_create_from_hub(&request).await?;

    // Policy gate: same rules as the regular `POST /api/mcp/servers`
    // handler. Done on the planned CreateMcpServerRequest BEFORE the
    // delete loop so a rejection by policy doesn't wipe the prior
    // install.
    let policy = crate::modules::mcp::user_policy::load(Repos.pool()).await?;
    crate::modules::mcp::user_policy::enforce_on_user_create(
        &mut plan.create_request,
        &policy,
    )?;

    // Same tiered command + flavor validation the native create path runs
    // (hub installs are user-owned → host tier). Done before the transaction
    // so a validation failure never touches the DB.
    crate::modules::mcp::handlers::validate_sandbox_fields_create(
        false,
        &plan.create_request,
    )?;

    // Wrap the whole replace flow (delete prior installs + create + track) in
    // ONE transaction: a mid-failure can no longer leave installs deleted with
    // no replacement, or a created server untracked. Events are deferred until
    // after commit so peers never react to rolled-back state.
    let pool = Repos.pool();
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    let mut deleted_ids: Vec<Uuid> = Vec::new();
    for existing_id in &existing_ids {
        // Tolerate "already deleted" — racy with a concurrent delete
        // (admin page in another tab). Any other DB error surfaces (rolls back).
        match Repos.mcp.delete_user_server_in_tx(&mut tx, *existing_id, auth.user.id).await {
            Ok(()) => deleted_ids.push(*existing_id),
            Err(e) if e.status_code() == 404 => (),
            Err(e) => return Err(e.into()),
        }
    }

    let server = Repos
        .mcp
        .create_user_server_in_tx(&mut tx, auth.user.id, plan.create_request)
        .await?;

    // Track in hub_entities, stamping the catalog version captured by
    // the lookup so /hub/installed can detect when this row falls behind.
    let hub_tracking = crate::modules::hub::repository::track_hub_entity_in_tx(
        &mut tx,
        HubEntityType::McpServer,
        server.id,
        &request.hub_id,
        HubCategory::McpServer,
        Some(auth.user.id),
        plan.hub_version.as_deref(),
    )
    .await?;

    tx.commit().await.map_err(AppError::database_error)?;

    // Emit events only AFTER the commit succeeds.
    for id in deleted_ids {
        event_bus
            .emit(crate::modules::mcp::events::McpServerEvent::user_server_deleted(
                id,
                auth.user.id,
            ))
            .await;
    }
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
            plan.create_request.sandbox_flavor = Some(prior.sandbox_flavor.clone());
            plan.create_request.usage_mode = Some(prior.usage_mode);
            plan.create_request.max_concurrent_sessions =
                prior.max_concurrent_sessions;
            plan.create_request.timeout_seconds = Some(prior.timeout_seconds);
            // MERGE (not replace) — start with the helper-seeded
            // entries (catalog defaults + placeholders for any NEW
            // `required_*` keys the catalog added between installs)
            // and overlay the prior row's entries. Prior entries win
            // for keys the admin set; catalog wins for newly-added
            // keys the admin hasn't seen. Without this merge, a
            // catalog upgrade that adds `WORKSPACE_ID` to a server
            // that previously only needed `API_KEY` would silently
            // drop the new placeholder on Re-install — the admin
            // would have no UI signal that new configuration is
            // needed.
            //
            // Per-entry `is_secret` is preserved from the prior row's
            // `*_entries` (the source of truth on the new storage
            // shape). For secret entries, `value: None` keeps the
            // stored encrypted value untouched on insert; non-secret
            // entries carry the prior plain value forward.
            {
                let prior_env_secret: std::collections::HashMap<&str, bool> = prior
                    .environment_variables_entries
                    .iter()
                    .map(|e| (e.key.as_str(), e.is_secret))
                    .collect();
                let mut merged: Vec<crate::modules::mcp::EnvVarEntry> = plan
                    .create_request
                    .environment_variables_entries
                    .take()
                    .unwrap_or_default();
                let mut seen: std::collections::HashSet<String> =
                    merged.iter().map(|e| e.key.clone()).collect();
                // Overlay prior values: replace seeded entry for keys
                // the admin set, or append new entries the catalog
                // doesn't declare.
                let prior_env_map: std::collections::HashMap<String, String> = prior
                    .environment_variables
                    .as_object()
                    .map(|o| {
                        o.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                for (k, v) in prior_env_map {
                    let is_secret = prior_env_secret.get(k.as_str()).copied().unwrap_or(false);
                    // Carry the plain value forward unconditionally —
                    // for SECRET entries `v` is the DECRYPTED plaintext
                    // from `prior.environment_variables` (the runtime
                    // flat map assembled with decrypts by
                    // `assemble_mcp_server`). The downstream
                    // `split_entries_for_storage` re-encrypts when
                    // `is_secret=true`, so the secret round-trips
                    // through decrypt-then-re-encrypt with a fresh
                    // pgp_sym_encrypt call. (Originally tried passing
                    // `value: None` here to preserve the on-disk
                    // ciphertext byte-for-byte, but the prior row is
                    // about to be deleted — `split_entries_for_storage`
                    // never sees a `prior_encrypted` value on the
                    // create path, so a None drops the secret entirely
                    // from the new row.)
                    let value = Some(v);
                    if seen.insert(k.clone()) {
                        merged.push(crate::modules::mcp::EnvVarEntry {
                            key: k,
                            value,
                            is_secret,
                        });
                    } else if let Some(existing) = merged.iter_mut().find(|e| e.key == k) {
                        existing.value = value;
                        existing.is_secret = is_secret;
                    }
                }
                plan.create_request.environment_variables_entries = Some(merged);
            }
            {
                let prior_hdr_secret: std::collections::HashMap<&str, bool> = prior
                    .headers_entries
                    .iter()
                    .map(|e| (e.key.as_str(), e.is_secret))
                    .collect();
                let mut merged: Vec<crate::modules::mcp::HeaderEntry> = plan
                    .create_request
                    .headers_entries
                    .take()
                    .unwrap_or_default();
                let mut seen: std::collections::HashSet<String> =
                    merged.iter().map(|e| e.key.clone()).collect();
                let prior_hdr_map: std::collections::HashMap<String, String> = prior
                    .headers
                    .as_object()
                    .map(|o| {
                        o.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                for (k, v) in prior_hdr_map {
                    let is_secret = prior_hdr_secret.get(k.as_str()).copied().unwrap_or(false);
                    // Same carry-forward semantic as env vars — `v` is
                    // the decrypted plaintext for secret headers.
                    let value = Some(v);
                    if seen.insert(k.clone()) {
                        merged.push(crate::modules::mcp::HeaderEntry {
                            key: k,
                            value,
                            is_secret,
                        });
                    } else if let Some(existing) = merged.iter_mut().find(|e| e.key == k) {
                        existing.value = value;
                        existing.is_secret = is_secret;
                    }
                }
                plan.create_request.headers_entries = Some(merged);
            }
        }
    }

    // Mirror the regular `POST /api/mcp/system-servers` handler's
    // tiered command + flavor validation BEFORE the prior-row delete
    // so a flavor-invalid re-install doesn't wipe the existing system
    // server with no replacement. Same gate runs again after the
    // delete (line ~1124) — defensive duplication so the gate fires
    // in BOTH orderings.
    crate::modules::mcp::handlers::validate_sandbox_fields_create(
        true,
        &plan.create_request,
    )?;

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

    // Same tiered command + flavor validation the native create path runs.
    // Runs AFTER the re-install run_in_sandbox carry-over above, so a
    // re-installed sandboxed server is correctly treated as sandbox-tier.
    crate::modules::mcp::handlers::validate_sandbox_fields_create(
        true,
        &plan.create_request,
    )?;

    let server = Repos.mcp.create_system_server(plan.create_request).await?;

    // Track with `created_by: None` so /hub/installed surfaces this as
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
        .find(|m| m.name == request.hub_id)
        .ok_or_else(|| AppError::not_found(&format!("Hub model '{}'", request.hub_id)))?;

    // 1b. Reject incompatible items (min_ziee_version > server).
    hub_manager
        .ensure_installable(HubCategory::Model, &request.hub_id)
        .await?;

    // 2. Resolve the source + quantization the user picked
    // (`repository_url` / `repository_path` / `main_filename` /
    // `file_format` are per-source, not flat on the manifest).
    if hub_model.sources.is_empty() {
        return Err(AppError::unprocessable_entity(
            "HUB_MODEL_NO_SOURCES",
            format!(
                "Hub model '{}' has no sources[] — publisher error.",
                hub_model.name
            ),
        )
        .into());
    }
    let source_index = request.source_index.unwrap_or(0);
    let source = hub_model.sources.get(source_index).ok_or_else(|| {
        AppError::bad_request(
            "HUB_MODEL_SOURCE_OUT_OF_RANGE",
            format!(
                "source_index {} out of range for hub model '{}' ({} sources)",
                source_index,
                hub_model.name,
                hub_model.sources.len()
            ),
        )
    })?;
    if source.quantizations.is_empty() {
        return Err(AppError::unprocessable_entity(
            "HUB_MODEL_NO_QUANTIZATIONS",
            format!(
                "Hub model '{}' source {} has no quantizations[] — publisher error.",
                hub_model.name, source_index
            ),
        )
        .into());
    }
    let quantization = if let Some(ref name) = request.quantization_name {
        source
            .quantizations
            .iter()
            .find(|q| &q.name == name)
            .ok_or_else(|| {
                AppError::bad_request(
                    "HUB_MODEL_QUANTIZATION_NOT_FOUND",
                    format!(
                        "Quantization '{}' not found in source {} of '{}'",
                        name, source_index, hub_model.name
                    ),
                )
            })?
    } else {
        source
            .quantizations
            .iter()
            .find(|q| q.is_default)
            .unwrap_or(&source.quantizations[0])
    };

    // 3. Derive (registry_url, repository_path) from the source's
    // `registry_type`. Same mapping as `derive_registry_url()` above
    // in `get_hub_models` — keeping them in lockstep avoids the
    // surprise of `source_auth_configured` saying true while the
    // install path 404s on the lookup, or vice versa.
    let (registry_url, repository_path) = match source.registry_type.as_str() {
        "huggingface" => (
            "https://huggingface.co".to_string(),
            source.identifier.clone(),
        ),
        "s3" => (
            "https://s3.amazonaws.com".to_string(),
            source.identifier.clone(),
        ),
        "url" => (source.identifier.clone(), source.identifier.clone()),
        other => {
            return Err(AppError::unprocessable_entity(
                "HUB_MODEL_REGISTRY_UNSUPPORTED",
                format!(
                    "Unsupported registry_type '{}' on hub model '{}'",
                    other, hub_model.name
                ),
            )
            .into());
        }
    };

    // 4. Find the matching `llm_repositories` row by URL.
    let repository = Repos
        .llm_repository
        .find_by_url(&registry_url)
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
                registry_url
            ))
        })?;

    // 4a. Block when the source repository is disabled. Mirrors the
    // auth gate just below — without this, a download against a
    // disabled repo would either fail later in the background (when
    // the git/HF client tries to clone) or, worse, silently succeed
    // because the repo's `enabled` field is purely UI state today.
    // The UI also gates on this before clicking Download (see
    // `ModelHubCard.tsx::handleDownload`); this is the defense-in-
    // depth backstop for direct API calls and stale-UI snapshots.
    if !repository.enabled {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            AppError::unprocessable_entity(
                "HUB_REPOSITORY_DISABLED",
                format!(
                    "Downloading \"{}\" requires the \"{}\" repository to be enabled, but \
                     it's currently disabled. Open the repository's settings and enable it.",
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

    // 4b. Block early with clear guidance when this source needs auth
    // (an env var marked `is_required + is_secret`) but the matching
    // repository has no credential configured. Without this the download
    // is spawned and only fails later in the background with an opaque
    // git auth error. Enforced server-side; the UI mirrors it via
    // `source_auth_configured`.
    let needs_auth = source
        .environment_variables
        .iter()
        .any(|ev| ev.is_required.unwrap_or(false) && ev.is_secret);
    if needs_auth && !repository.has_credential() {
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

    // 5. Convert FileFormat from hub to llm_model
    let file_format = match source.file_format {
        super::models::FileFormat::GGUF => LlmFileFormat::Gguf,
        super::models::FileFormat::SafeTensors => LlmFileFormat::Safetensors,
        super::models::FileFormat::PyTorch => LlmFileFormat::Pytorch,
    };

    // 6. Convert capabilities from hub to llm_model format
    let capabilities = hub_model.capabilities.map(|hub_caps| {
        crate::modules::llm_model::models::ModelCapabilities {
            vision: Some(hub_caps.vision),
            audio: Some(hub_caps.audio),
            tools: Some(hub_caps.tools),
            code_interpreter: Some(hub_caps.code_interpreter),
            chat: Some(hub_caps.chat),
            text_embedding: Some(hub_caps.text_embedding),
            image_generator: Some(hub_caps.image_generator),
            context_length: source.context_length.and_then(|n| u32::try_from(n).ok()),
        }
    });

    // 7. Build download request for initiate_repository_download.
    //    `main_filename` comes from the selected quantization; the
    //    `repository_branch` is the source's version pin (branch /
    //    commit / tag). Engine fields are dropped from the manifest
    //    — the install path no longer carries `recommended_engine` /
    //    `recommended_engine_settings`.
    let download_request = crate::modules::llm_model::handlers::uploads::DownloadFromRepositoryRequest {
        provider_id: request.provider_id,
        repository_id: repository.id,
        repository_path,
        repository_branch: if source.version.is_empty() {
            None
        } else {
            Some(source.version.clone())
        },
        name: hub_model.name.clone(),
        display_name: request
            .display_name
            .unwrap_or_else(|| hub_model.display_name.clone()),
        description: hub_model.description.clone(),
        file_format,
        main_filename: quantization.main_file.clone(),
        capabilities,
        parameters: hub_model
            .recommended_parameters
            .and_then(|p| serde_json::from_value(p).ok()),
        // No model-wide engine hints. `runtime_hint` lives on the
        // source but is purely informational today — the engine is
        // picked downstream from the file format.
        engine_type: None,
        engine_settings: None,
    };

    // 8. Initiate the actual download (this creates the download instance AND spawns the background task)
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

    // 9. Track in hub_entities (stamp the entry's per-entry version
    //    — see resolve_entry_version above).
    let hub_version =
        resolve_entry_version(&hub_manager, HubCategory::Model, &request.hub_id).await;
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

    // 10. Emit event
    event_bus.emit_async(
        HubEvent::model_download_started_from_hub(download.id, request.hub_id.clone()).into(),
    );

    // 11. Return response
    Ok((
        StatusCode::CREATED,
        Json(ModelFromHubResponse {
            download,
            hub_tracking,
        }),
    ))
}

// =====================================================
// SKILL FROM HUB
// =====================================================

/// Shared lookup + bundle-extract + frontmatter parse for both skill
/// install paths (user / system). Mirrors `build_assistant_create_from_hub`
/// but for the directory-bundle shape.
struct HubSkillCreatePlan {
    create_request: CreateSkill,
    hub_version: Option<String>,
    extracted_path: std::path::PathBuf,
}

async fn build_skill_create_from_hub(
    hub_id: &str,
    scope: &str,
    owner_user_id: Option<Uuid>,
    created_by: Option<Uuid>,
) -> Result<HubSkillCreatePlan, AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir.clone())?;

    hub_manager
        .ensure_installable(HubCategory::Skill, hub_id)
        .await?;

    let hub_version =
        resolve_entry_version(&hub_manager, HubCategory::Skill, hub_id).await;

    let manifest = hub_manager.manifest(HubCategory::Skill, hub_id).await?;
    let hub_skill = manifest
        .skill
        .ok_or_else(|| {
            AppError::internal_error(format!(
                "hub: manifest for '{hub_id}' is not a skill"
            ))
        })?;

    // SEC-2: validate the manifest-supplied entry_point BEFORE it's
    // joined to the extracted dir or persisted. A malicious hub author
    // could otherwise set `entry_point: "../../../etc/passwd"`.
    super::bundle::validate_entry_point(&hub_skill.bundle.entry_point)?;

    // H1: owner-scope the on-disk layout so two users installing the same
    // hub skill don't share a dir (User B's install would `remove_dir_all`
    // User A's live dir). Layout:
    //   <app_data>/skills/<owner-or-"system">/<reverse-dns>/<version>/
    // The reverse-DNS path safety is enforced by `is_safe_name` inside
    // hub_manager::manifest; defense-in-depth in the extractor against
    // `..`/`/`.
    let version_seg = hub_skill
        .version
        .clone()
        .unwrap_or_else(|| "unversioned".to_string());
    let target_dir = app_data_dir
        .join("skills")
        .join(owner_dir_segment(owner_user_id))
        .join(&hub_skill.name)
        .join(&version_seg);

    let extraction = super::bundle::fetch_and_extract(
        &hub_manager,
        &hub_skill.bundle,
        &target_dir,
        super::bundle::BundleKind::Skill,
    )
    .await?;

    // Parse SKILL.md frontmatter from the extracted bundle. Entry point
    // defaults to "SKILL.md" — honor whatever the manifest specifies in
    // case the publisher evolves the convention.
    let skill_md_path = extraction.extracted_path.join(&hub_skill.bundle.entry_point);
    let content = tokio::fs::read_to_string(&skill_md_path).await.map_err(|e| {
        AppError::internal_error(format!(
            "hub: read SKILL.md at {}: {}",
            skill_md_path.display(),
            e
        ))
    })?;
    let (frontmatter_json, _body) =
        skill::frontmatter::parse_skill_md_frontmatter(&content)?;

    let display_name = frontmatter_json
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let description = frontmatter_json
        .get("description")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| hub_skill.description.clone());
    let when_to_use = frontmatter_json
        .get("when_to_use")
        .or_else(|| frontmatter_json.get("when-to-use"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let create_request = CreateSkill {
        name: hub_skill.name.clone(),
        version: hub_skill.version.clone(),
        display_name,
        description,
        when_to_use,
        extracted_path: extraction.extracted_path.display().to_string(),
        bundle_sha256: extraction.sha256_hex.clone(),
        bundle_size_bytes: extraction.total_bytes as i64,
        file_count: extraction.file_count as i32,
        entry_point: hub_skill.bundle.entry_point.clone(),
        frontmatter_json,
        tags: serde_json::Value::Array(
            hub_skill
                .tags
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
        scope: scope.to_string(),
        owner_user_id,
        created_by,
        enabled: true,
        is_dev: false,
    };

    Ok(HubSkillCreatePlan {
        create_request,
        hub_version,
        extracted_path: extraction.extracted_path,
    })
}

/// User-scoped skill install. Permission: `skills::install` (any
/// authenticated user by default).
#[debug_handler]
pub async fn create_skill_from_hub(
    auth: RequirePermissions<(SkillsInstall,)>,
    Extension(_event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateSkillFromHubRequest>,
) -> ApiResult<Json<SkillFromHubResponse>> {
    let plan = build_skill_create_from_hub(
        &request.hub_id,
        "user",
        Some(auth.user.id),
        Some(auth.user.id),
    )
    .await?;

    // H1: re-install must OVERWRITE cleanly. The per-owner unique index
    // means a same (name, version) re-install by THIS user would 23505 on
    // insert. Delete the caller's own prior row first (the on-disk dir was
    // already replaced by fetch_and_extract). Scoped to THIS owner so we
    // never touch another user's row or the system copy (also closes H6
    // for the hub path).
    let create_name = plan.create_request.name.clone();
    let create_version = plan.create_request.version.clone();
    if let Some(prior) = Repos
        .skill
        .find_by_name_version_owner(
            &create_name,
            create_version.as_deref(),
            Some(auth.user.id),
        )
        .await?
    {
        Repos.skill.delete(prior.id).await?;
        // Drop the prior install's hub_entities row (no FK cascade) so
        // the overwrite doesn't leave it orphaned at the stale version.
        Repos
            .hub
            .delete_hub_tracking(HubEntityType::Skill, prior.id)
            .await?;
    }

    let skill = match Repos.skill.insert(plan.create_request).await {
        Ok(s) => s,
        Err(e) => {
            // Best-effort cleanup of the extracted bundle on insert
            // failure so we don't leak disk.
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    let hub_tracking = match Repos
        .hub
        .track_hub_entity(
            HubEntityType::Skill,
            skill.id,
            &request.hub_id,
            HubCategory::Skill,
            Some(auth.user.id),
            plan.hub_version.as_deref(),
        )
        .await
    {
        Ok(t) => t,
        Err(e) => {
            // Roll back the partial state: drop the skill row + extracted
            // dir so a hub-tracking failure doesn't leak a half-install.
            let _ = Repos.skill.delete(skill.id).await;
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    skill::events::emit_user_skill(SyncAction::Create, skill.id, auth.user.id, origin.0);

    Ok((StatusCode::CREATED, Json(SkillFromHubResponse { skill, hub_tracking })))
}

/// System-scope skill install. Permission: `skills::manage_system`.
/// Optional `groups: [...]` body field assigns the new system-scope
/// skill to specific groups in the same TX as the install.
///
/// M6: the three post-extract DB writes (skills insert + group_skills
/// rows + hub_entities track) run in ONE transaction. A mid-failure
/// rolls back ALL of them so we never leak a half-install (orphan row,
/// partial group set, untracked entity). The extracted dir is cleaned
/// up on any post-extract error. H1: a same (name, version) system
/// re-install deletes the prior system row first (within the same TX).
#[debug_handler]
pub async fn create_system_skill_from_hub(
    auth: RequirePermissions<(SkillsManageSystem,)>,
    Extension(_event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateSystemSkillFromHubRequest>,
) -> ApiResult<Json<SkillFromHubResponse>> {
    let plan = build_skill_create_from_hub(
        &request.hub_id,
        "system",
        None,
        Some(auth.user.id),
    )
    .await?;

    let skill = match install_system_skill_tx(
        Repos.pool(),
        &plan.create_request,
        &request.groups,
        &request.hub_id,
        plan.hub_version.as_deref(),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            // Roll-back already happened (TX dropped uncommitted). Clean
            // up the extracted dir so a failed install leaks nothing.
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    let hub_tracking = HubEntity {
        id: skill.hub_entity_id,
        entity_type: HubEntityType::Skill.as_str().to_string(),
        entity_id: skill.skill.id,
        hub_id: request.hub_id.clone(),
        hub_category: HubCategory::Skill.as_str().to_string(),
        created_at: skill.skill.created_at,
        created_by: None,
    };

    skill::events::emit_system_skill(SyncAction::Create, skill.skill.id, origin.0);

    Ok((
        StatusCode::CREATED,
        Json(SkillFromHubResponse {
            skill: skill.skill,
            hub_tracking,
        }),
    ))
}

/// Bundles the inserted skill row with the hub_entities row id created in
/// the same transaction.
struct SystemSkillInstallResult {
    skill: skill::models::Skill,
    hub_entity_id: Uuid,
}

/// M6: transactional system-skill install (insert + group rows +
/// hub_entities track). Returns Err with the TX rolled back on any step.
async fn install_system_skill_tx(
    pool: &sqlx::PgPool,
    create: &CreateSkill,
    groups: &[Uuid],
    hub_id: &str,
    hub_version: Option<&str>,
) -> Result<SystemSkillInstallResult, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // H1: overwrite a prior system row for the same (name, version).
    // Drop its hub_entities tracking row too (no FK cascade) so the
    // overwrite doesn't leave an orphan pointing at the deleted skill.
    let prior_ids: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT id FROM skills WHERE name = $1 AND scope = 'system'
           AND (($2::text IS NULL AND version IS NULL) OR version = $2)"#,
        create.name,
        create.version,
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(AppError::database_error)?;
    for pid in &prior_ids {
        sqlx::query!(
            r#"DELETE FROM hub_entities WHERE entity_type = 'skill' AND entity_id = $1"#,
            pid,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        sqlx::query!(r#"DELETE FROM skills WHERE id = $1"#, pid)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
    }

    let skill = sqlx::query_as!(
        skill::models::Skill,
        r#"
        INSERT INTO skills (
            name, version, display_name, description, when_to_use,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point, frontmatter_json, tags,
            scope, owner_user_id, created_by, enabled, is_dev
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
        RETURNING
            id, name, version, display_name, description, when_to_use,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point,
            frontmatter_json as "frontmatter_json: _",
            tags as "tags: _", scope, owner_user_id, created_by,
            enabled, is_dev,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        create.name,
        create.version,
        create.display_name,
        create.description,
        create.when_to_use,
        create.extracted_path,
        create.bundle_sha256,
        create.bundle_size_bytes,
        create.file_count,
        create.entry_point,
        create.frontmatter_json,
        create.tags,
        create.scope,
        create.owner_user_id,
        create.created_by,
        create.enabled,
        create.is_dev,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    for group_id in groups {
        sqlx::query!(
            r#"INSERT INTO group_skills (group_id, skill_id)
               VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
            group_id,
            skill.id,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    let hub_entity_id = sqlx::query_scalar!(
        r#"
        INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by, hub_version)
        VALUES ($1, $2, $3, $4, NULL, $5)
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET hub_id = EXCLUDED.hub_id, hub_category = EXCLUDED.hub_category, hub_version = EXCLUDED.hub_version
        RETURNING id
        "#,
        HubEntityType::Skill.as_str(),
        skill.id,
        hub_id,
        HubCategory::Skill.as_str(),
        hub_version,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(SystemSkillInstallResult { skill, hub_entity_id })
}

// =====================================================
// WORKFLOW FROM HUB
// =====================================================

struct HubWorkflowCreatePlan {
    create_request: CreateWorkflow,
    hub_version: Option<String>,
    extracted_path: std::path::PathBuf,
}

async fn build_workflow_create_from_hub(
    hub_id: &str,
    scope: &str,
    owner_user_id: Option<Uuid>,
    created_by: Option<Uuid>,
) -> Result<HubWorkflowCreatePlan, AppError> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir.clone())?;

    hub_manager
        .ensure_installable(HubCategory::Workflow, hub_id)
        .await?;

    let hub_version =
        resolve_entry_version(&hub_manager, HubCategory::Workflow, hub_id).await;

    let manifest = hub_manager.manifest(HubCategory::Workflow, hub_id).await?;
    let hub_workflow = manifest
        .workflow
        .ok_or_else(|| {
            AppError::internal_error(format!(
                "hub: manifest for '{hub_id}' is not a workflow"
            ))
        })?;

    // SEC-2: validate the manifest-supplied entry_point before any join.
    super::bundle::validate_entry_point(&hub_workflow.bundle.entry_point)?;

    // H1: owner-scope the on-disk layout (see build_skill_create_from_hub).
    let version_seg = hub_workflow
        .version
        .clone()
        .unwrap_or_else(|| "unversioned".to_string());
    let target_dir = app_data_dir
        .join("workflows")
        .join(owner_dir_segment(owner_user_id))
        .join(&hub_workflow.name)
        .join(&version_seg);

    let extraction = super::bundle::fetch_and_extract(
        &hub_manager,
        &hub_workflow.bundle,
        &target_dir,
        super::bundle::BundleKind::Workflow,
    )
    .await?;

    // Parse + Layer 1+2+3 validate workflow.yaml. Rejects malformed
    // bundles before they touch the DB. Published workflows are NOT
    // is_dev → mock: in step defs is rejected here.
    let workflow_yaml_path = extraction.extracted_path.join(&hub_workflow.bundle.entry_point);
    let content = tokio::fs::read_to_string(&workflow_yaml_path).await.map_err(|e| {
        AppError::internal_error(format!(
            "hub: read workflow.yaml at {}: {}",
            workflow_yaml_path.display(),
            e
        ))
    })?;
    let workflow_def = workflow::validate::parse_workflow_yaml(&content)?;
    workflow::validate::validate_for_install(
        &workflow_def,
        &extraction.extracted_path,
        false,
    )?;

    // Reject install when the computed MCP tool slug would overflow the
    // 128-char composed-name cap (slug body > 87 chars) — otherwise the
    // workflow installs but can never surface as a workflow_mcp tool
    // (list_tools would silently drop it). Audit gap 4 / plan §4.
    if let Err(e) =
        crate::modules::workflow_mcp::tools::check_install_slug_len(&hub_workflow.name)
    {
        let _ = tokio::fs::remove_dir_all(&extraction.extracted_path).await;
        return Err(e);
    }

    let display_name = hub_workflow.name.rsplit('/').next().map(str::to_string);

    let create_request = CreateWorkflow {
        name: hub_workflow.name.clone(),
        version: hub_workflow.version.clone(),
        display_name,
        description: hub_workflow.description.clone(),
        extracted_path: extraction.extracted_path.display().to_string(),
        bundle_sha256: extraction.sha256_hex.clone(),
        bundle_size_bytes: extraction.total_bytes as i64,
        file_count: extraction.file_count as i32,
        entry_point: hub_workflow.bundle.entry_point.clone(),
        tags: serde_json::Value::Array(
            hub_workflow
                .tags
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
        scope: scope.to_string(),
        owner_user_id,
        created_by,
        enabled: true,
        is_dev: false,
        // Pattern (d): compile the validated def into the typed IR and
        // persist it so the column is non-NULL + available to the runner.
        compiled_ir_json: workflow::compiled::compile_to_json(&workflow_def),
    };

    Ok(HubWorkflowCreatePlan {
        create_request,
        hub_version,
        extracted_path: extraction.extracted_path,
    })
}

#[debug_handler]
pub async fn create_workflow_from_hub(
    auth: RequirePermissions<(WorkflowsInstall,)>,
    Extension(_event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateWorkflowFromHubRequest>,
) -> ApiResult<Json<WorkflowFromHubResponse>> {
    let plan = build_workflow_create_from_hub(
        &request.hub_id,
        "user",
        Some(auth.user.id),
        Some(auth.user.id),
    )
    .await?;

    // H1: re-install overwrite — delete THIS user's prior row for the
    // same (name, version) (the dir was already replaced by extract).
    let create_name = plan.create_request.name.clone();
    let create_version = plan.create_request.version.clone();
    if let Some(prior) = Repos
        .workflow
        .find_by_name_version_owner(
            &create_name,
            create_version.as_deref(),
            Some(auth.user.id),
        )
        .await?
    {
        Repos.workflow.delete(prior.id).await?;
        Repos
            .hub
            .delete_hub_tracking(HubEntityType::Workflow, prior.id)
            .await?;
    }

    let workflow = match Repos.workflow.insert(plan.create_request).await {
        Ok(w) => w,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    let hub_tracking = match Repos
        .hub
        .track_hub_entity(
            HubEntityType::Workflow,
            workflow.id,
            &request.hub_id,
            HubCategory::Workflow,
            Some(auth.user.id),
            plan.hub_version.as_deref(),
        )
        .await
    {
        Ok(t) => t,
        Err(e) => {
            let _ = Repos.workflow.delete(workflow.id).await;
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    workflow::events::emit_user_workflow(
        SyncAction::Create,
        workflow.id,
        auth.user.id,
        origin.0,
    );

    Ok((
        StatusCode::CREATED,
        Json(WorkflowFromHubResponse {
            workflow,
            hub_tracking,
        }),
    ))
}

/// M6 + H1: transactional system-workflow install (mirrors
/// `create_system_skill_from_hub`).
#[debug_handler]
pub async fn create_system_workflow_from_hub(
    auth: RequirePermissions<(WorkflowsManageSystem,)>,
    Extension(_event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateSystemWorkflowFromHubRequest>,
) -> ApiResult<Json<WorkflowFromHubResponse>> {
    let plan = build_workflow_create_from_hub(
        &request.hub_id,
        "system",
        None,
        Some(auth.user.id),
    )
    .await?;

    let result = match install_system_workflow_tx(
        Repos.pool(),
        &plan.create_request,
        &request.groups,
        &request.hub_id,
        plan.hub_version.as_deref(),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&plan.extracted_path);
            return Err(e.into());
        }
    };

    let hub_tracking = HubEntity {
        id: result.hub_entity_id,
        entity_type: HubEntityType::Workflow.as_str().to_string(),
        entity_id: result.workflow.id,
        hub_id: request.hub_id.clone(),
        hub_category: HubCategory::Workflow.as_str().to_string(),
        created_at: result.workflow.created_at,
        created_by: None,
    };

    workflow::events::emit_system_workflow(SyncAction::Create, result.workflow.id, origin.0);

    Ok((
        StatusCode::CREATED,
        Json(WorkflowFromHubResponse {
            workflow: result.workflow,
            hub_tracking,
        }),
    ))
}

struct SystemWorkflowInstallResult {
    workflow: workflow::models::Workflow,
    hub_entity_id: Uuid,
}

/// M6: transactional system-workflow install (insert + group rows +
/// hub_entities track). TX rolls back on any step failure.
async fn install_system_workflow_tx(
    pool: &sqlx::PgPool,
    create: &CreateWorkflow,
    groups: &[Uuid],
    hub_id: &str,
    hub_version: Option<&str>,
) -> Result<SystemWorkflowInstallResult, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // H1: overwrite a prior system row for the same (name, version) +
    // drop its hub_entities tracking row (no FK cascade).
    let prior_ids: Vec<Uuid> = sqlx::query_scalar!(
        r#"SELECT id FROM workflows WHERE name = $1 AND scope = 'system'
           AND (($2::text IS NULL AND version IS NULL) OR version = $2)"#,
        create.name,
        create.version,
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(AppError::database_error)?;
    for pid in &prior_ids {
        sqlx::query!(
            r#"DELETE FROM hub_entities WHERE entity_type = 'workflow' AND entity_id = $1"#,
            pid,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
        sqlx::query!(r#"DELETE FROM workflows WHERE id = $1"#, pid)
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
    }

    let workflow = sqlx::query_as!(
        workflow::models::Workflow,
        r#"
        INSERT INTO workflows (
            name, version, display_name, description,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point, tags,
            scope, owner_user_id, created_by, enabled, is_dev,
            compiled_ir_json
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
        RETURNING
            id, name, version, display_name, description,
            extracted_path, bundle_sha256, bundle_size_bytes, file_count,
            entry_point,
            tags as "tags: _",
            scope, owner_user_id, created_by, enabled, is_dev,
            compiled_ir_json as "compiled_ir_json: _",
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        create.name,
        create.version,
        create.display_name,
        create.description,
        create.extracted_path,
        create.bundle_sha256,
        create.bundle_size_bytes,
        create.file_count,
        create.entry_point,
        create.tags,
        create.scope,
        create.owner_user_id,
        create.created_by,
        create.enabled,
        create.is_dev,
        create.compiled_ir_json,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    for group_id in groups {
        sqlx::query!(
            r#"INSERT INTO group_workflows (group_id, workflow_id)
               VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
            group_id,
            workflow.id,
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    let hub_entity_id = sqlx::query_scalar!(
        r#"
        INSERT INTO hub_entities (entity_type, entity_id, hub_id, hub_category, created_by, hub_version)
        VALUES ($1, $2, $3, $4, NULL, $5)
        ON CONFLICT (entity_type, entity_id)
        DO UPDATE SET hub_id = EXCLUDED.hub_id, hub_category = EXCLUDED.hub_category, hub_version = EXCLUDED.hub_version
        RETURNING id
        "#,
        HubEntityType::Workflow.as_str(),
        workflow.id,
        hub_id,
        HubCategory::Workflow.as_str(),
        hub_version,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(SystemWorkflowInstallResult { workflow, hub_entity_id })
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

pub fn create_skill_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsInstall,)>(op)
        .id("Hub.createSkillFromHub")
        .tag("Hub")
        .tag("Skills")
        .summary("Install user-scope skill from hub catalog")
        .response::<201, Json<SkillFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub skill not found"))
        .response_with::<422, (), _>(|res| {
            res.description(
                "Bundle verification failed (sha256 mismatch, size cap, \
                 path traversal, non-regular tar entry, or SKILL.md \
                 frontmatter invalid)",
            )
        })
}

pub fn create_system_skill_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(SkillsManageSystem,)>(op)
        .id("Hub.createSystemSkillFromHub")
        .tag("Hub")
        .tag("Skills - System")
        .summary("Install SYSTEM-WIDE skill from hub catalog")
        .description(
            "Installs a hub skill entry as a system-wide skill \
             (`scope='system', owner_user_id=NULL`). Optional \
             `groups: [...]` body field assigns the skill to specific \
             groups in the same install. Requires `skills::manage_system` \
             (admin).",
        )
        .response::<201, Json<SkillFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub skill not found"))
        .response_with::<422, (), _>(|res| {
            res.description("Bundle verification or frontmatter parse failure")
        })
}

pub fn create_workflow_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsInstall,)>(op)
        .id("Hub.createWorkflowFromHub")
        .tag("Hub")
        .tag("Workflows")
        .summary("Install user-scope workflow from hub catalog")
        .response::<201, Json<WorkflowFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub workflow not found"))
        .response_with::<422, (), _>(|res| {
            res.description(
                "Bundle verification failed OR workflow.yaml structurally \
                 invalid (duplicate step id, depends_on cycle, unknown dependency)",
            )
        })
}

pub fn create_system_workflow_from_hub_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(WorkflowsManageSystem,)>(op)
        .id("Hub.createSystemWorkflowFromHub")
        .tag("Hub")
        .tag("Workflows - System")
        .summary("Install SYSTEM-WIDE workflow from hub catalog")
        .description(
            "Installs a hub workflow entry as a system-wide workflow \
             (`scope='system', owner_user_id=NULL`). Optional `groups: [...]` \
             body field assigns it to specific groups. Requires \
             `workflows::manage_system`.",
        )
        .response::<201, Json<WorkflowFromHubResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Hub workflow not found"))
        .response_with::<422, (), _>(|res| {
            res.description("Bundle verification or workflow.yaml validation failure")
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
        skills: 0,
        workflows: 0,
    };
    for item in &catalog.items {
        match item.category {
            HubCategory::Model => counts.models += 1,
            HubCategory::Assistant => counts.assistants += 1,
            HubCategory::McpServer => counts.mcp_servers += 1,
            HubCategory::Skill => counts.skills += 1,
            HubCategory::Workflow => counts.workflows += 1,
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

/// POST /api/hub/refresh — admin-only force fetch from the Pages
/// catalog. The catalog is index-only: per-entry manifests are fetched
/// lazily by `manifest()`. Network failure leaves the previous index
/// in place (the tmp/rename swap is atomic).
#[debug_handler]
pub async fn refresh_hub_catalog(
    _auth: RequirePermissions<(HubCatalogManage,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<Json<HubCatalogRefreshResponse>> {
    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let outcome = hub_manager.refresh().await?;

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

    sync_publish(SyncEntity::HubSettings, SyncAction::Update, uuid::Uuid::nil(), Audience::perm::<HubCatalogRead>(), origin.0);

    Ok((
        StatusCode::OK,
        Json(HubCatalogRefreshResponse {
            updated: outcome.updated,
            previous_version: outcome.previous_version,
            new_version: outcome.new_version,
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

/// GET /api/hub/installed — every tracked hub install the caller
/// can see. Per-user view by default; admins (anyone with
/// `hub::catalog::read`) additionally see system-wide installs
/// (template assistants, system MCP servers, models — which are
/// always system-scoped). Replaces the older `/hub/installed`
/// admin-only endpoint with a strictly richer payload.
#[debug_handler]
pub async fn get_hub_installed(
    auth: crate::modules::auth::jwt_extractor::JwtAuth,
) -> ApiResult<Json<HubInstalledResponse>> {
    // Resolve user + groups so we can check `hub::catalog::read`
    // inline (the union check needs both arrays). Same shape as the
    // `/api/auth/me` handler — JwtAuth gives us claims; we load the
    // rest from the DB.
    let user_id = uuid::Uuid::parse_str(&auth.claims.sub).map_err(|e| {
        AppError::internal_error(format!("Invalid user ID in token: {}", e))
    })?;
    let user = Repos
        .user
        .get_by_id(user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User"))?;
    let groups = Repos.user.get_user_groups(user_id).await?;

    let is_admin_view = crate::modules::permissions::checker::check_permission_union(
        &user,
        &groups,
        "hub::catalog::read",
    );

    let app_data_dir = crate::core::get_app_data_dir();
    let hub_manager = HubManager::new(app_data_dir)?;
    let catalog = hub_manager.catalog().await?;

    let rows = Repos
        .hub
        .list_installed_entities(Some(user_id), is_admin_view)
        .await?;

    // Build a `(category, id) -> version` lookup from the catalog so
    // each installed row reports its OWN current version rather than
    // the catalog-wide build marker. Falls back to `catalog.hub_version`
    // for entries that haven't been republished with a per-entry
    // `version` envelope yet.
    let entry_versions: std::collections::HashMap<(String, String), String> = catalog
        .items
        .iter()
        .map(|it| {
            (
                (it.category.as_str().to_string(), it.name.clone()),
                it.version
                    .clone()
                    .unwrap_or_else(|| catalog.hub_version.clone()),
            )
        })
        .collect();

    Ok((
        StatusCode::OK,
        Json(HubInstalledResponse {
            catalog_version: catalog.hub_version.clone(),
            items: rows
                .into_iter()
                .map(|r| {
                    let current_version = entry_versions
                        .get(&(r.hub_category.clone(), r.hub_id.clone()))
                        .cloned()
                        .unwrap_or_else(|| catalog.hub_version.clone());
                    HubInstalledRow {
                        hub_id: r.hub_id,
                        hub_category: r.hub_category,
                        entity_type: r.entity_type,
                        entity_id: r.entity_id,
                        name: r.name,
                        installed_version: r.installed_version,
                        current_version,
                        installed_at: r.installed_at,
                        is_system: r.is_system,
                        is_template_install: r.is_template_install,
                        is_system_mcp_install: r.is_system_mcp_install,
                    }
                })
                .collect(),
        }),
    ))
}

pub fn get_hub_installed_docs(op: TransformOperation) -> TransformOperation {
    op.id("Hub.getInstalled")
        .tag("Hub")
        .summary("Every tracked hub install visible to the caller")
        .response::<200, Json<HubInstalledResponse>>()
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

// There is no GET /hub/releases or POST /hub/activate. Pages doesn't
// publish multiple addressable catalog versions (the gh-pages branch
// is the latest, period), and per-entry semver on each IndexItem fills
// the role of a catalog-wide version picker.

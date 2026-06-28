// LLM Repository handlers - copied from react-test and refactored for ziee
// Source: react-test/src-tauri/src/api/repositories.rs
// IMPORTANT: ALL validation logic preserved from react-test

use aide::transform::TransformOperation;
use axum::{
    Extension, Json, debug_handler,
    extract::{Path, Query},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    common::{ApiResult, AppError, PaginationQuery},
    core::{events::EventBus, repository::Repos},
    modules::permissions::{RequirePermissions, with_permission},
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};
use std::sync::Arc;

use super::{
    connection_health::{self, LlmRepositoryWithHealthWarning},
    events::LlmRepositoryEvent,
    models::LlmRepository,
    permissions::*,
    types::{
        CreateLlmRepositoryRequest, LlmRepositoryListResponse, TestRepositoryConnectionRequest,
        TestRepositoryConnectionResponse, UpdateLlmRepositoryRequest,
    },
    utils,
};

// =====================================================
// Route Handlers
// =====================================================

/// List all LLM repositories (requires llm_repositories::read permission)
#[debug_handler]
pub async fn list_repositories(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Query(params): Query<PaginationQuery>,
) -> ApiResult<Json<LlmRepositoryListResponse>> {
    // Get all repositories
    let all_repositories = Repos.llm_repository.list().await.map_err(|e| {
        tracing::error!("Failed to get repositories: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Calculate pagination. Cast to i64 before multiply so the
    // PaginationQuery clamp can't be circumvented at the multiply
    // (defense-in-depth; the deserializer already bounds the inputs).
    // Closes 09-llm-repository F-11 (Medium).
    let total = all_repositories.len() as i64;
    let start = ((params.page as i64).saturating_sub(1))
        .saturating_mul(params.per_page as i64)
        .max(0) as usize;
    let end = start
        .saturating_add(params.per_page as usize)
        .min(all_repositories.len());

    let paginated_repositories = if start < all_repositories.len() {
        all_repositories[start..end].to_vec()
    } else {
        Vec::new()
    };

    Ok((
        StatusCode::OK,
        Json(LlmRepositoryListResponse {
            repositories: paginated_repositories,
            total,
            page: params.page,
            per_page: params.per_page,
        }),
    ))
}

/// Documentation for list_repositories endpoint
pub fn list_repositories_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.list")
        .tag("LLM Repositories")
        .summary("List all LLM repositories with pagination")
        .response::<200, Json<LlmRepositoryListResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Get LLM repository by ID (requires llm_repositories::read permission)
#[debug_handler]
pub async fn get_repository(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Path(repository_id): Path<Uuid>,
) -> ApiResult<Json<LlmRepository>> {
    let repository = Repos.llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    Ok((StatusCode::OK, Json(repository)))
}

/// Documentation for get_repository endpoint
pub fn get_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.get")
        .tag("LLM Repositories")
        .summary("Get LLM repository by ID")
        .response::<200, Json<LlmRepository>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Create a new LLM repository (requires llm_repositories::create permission)
/// ALL validation logic preserved from react-test
#[debug_handler]
pub async fn create_repository(
    _auth: RequirePermissions<(LlmRepositoriesCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<CreateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepositoryWithHealthWarning>> {
    // Validate auth type
    utils::validate_auth_type(&request.auth_type)?;

    // Validate URL format
    utils::validate_url(&request.url)?;

    // Validate authentication configuration
    utils::validate_auth_config_for_create(&request)?;

    // Create repository. Map the UNIQUE(name)/UNIQUE(url) violations to a clean
    // 400 conflict instead of an opaque 500 (the url uniqueness is enforced by
    // migration 116; name by migration 2).
    let repository = Repos.llm_repository.create(request).await.map_err(|e| {
        if let sqlx::Error::Database(db) = &e
            && db.is_unique_violation()
        {
            let field = match db.constraint() {
                Some("llm_repositories_url_key") => "URL",
                Some(c) if c.contains("name") => "name",
                _ => "name or URL",
            };
            return AppError::bad_request(
                "DUPLICATE_REPOSITORY",
                format!("A repository with this {field} already exists"),
            );
        }
        tracing::error!("Failed to create repository: {}", e);
        AppError::internal_error("Database operation failed")
    })?;

    // Emit event BEFORE the probe so downstream listeners see the
    // creation regardless of probe outcome — the AutoDisabled event
    // (if probe fails) emits inside `enforce_on_create`.
    event_bus.emit_async(LlmRepositoryEvent::created(repository.clone()).into());

    // Probe + auto-downgrade when `enabled: true`. On probe failure,
    // the row stays in the DB but flips to `enabled: false` and the
    // response carries the human-readable reason in
    // `connection_warning`. Always Ok — the row was created
    // successfully; the warning is informational.
    let wrapped = connection_health::enforce_on_create(repository, &event_bus).await?;

    // Cross-device sync: publish AFTER the probe so subscribers reload
    // and see the post-probe `enabled` state (the probe may have
    // flipped it to false on failure).
    sync_publish(
        SyncEntity::LlmRepository,
        SyncAction::Create,
        wrapped.repository.id,
        Audience::perm::<LlmRepositoriesRead>(),
        origin.0,
    );

    Ok((StatusCode::CREATED, Json(wrapped)))
}

/// Documentation for create_repository endpoint
pub fn create_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesCreate,)>(op)
        .id("LlmRepository.create")
        .tag("LLM Repositories")
        .summary("Create a new LLM repository")
        .response::<201, Json<LlmRepositoryWithHealthWarning>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Update an existing LLM repository (requires llm_repositories::edit permission)
/// ALL validation logic preserved from react-test including auth_config merging
#[debug_handler]
pub async fn update_repository(
    _auth: RequirePermissions<(LlmRepositoriesEdit,)>,
    Path(repository_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(request): Json<UpdateLlmRepositoryRequest>,
) -> ApiResult<Json<LlmRepository>> {
    // Validate auth type if provided
    if let Some(ref auth_type) = request.auth_type {
        utils::validate_auth_type(auth_type)?;
    }

    // Validate URL format if provided
    if let Some(ref url) = request.url {
        utils::validate_url(url)?;
    }

    // Get current repository to validate auth config updates
    let current_repository = Repos.llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // SECURITY: refuse to mutate the built-in repository's URL,
    // auth_type, or name. The delete handler already blocked this
    // for built-in repos, but the update handler didn't — so any
    // holder of llm_repositories::edit could swap the Hugging Face
    // URL to an attacker-controlled domain, then watch tokens flow
    // there on the next model download. Closes 09-llm-repository F-16.
    //
    // `auth_config` IS allowed to mutate (originally blocked, then
    // relaxed): the built-in HF repository ships with an empty
    // `api_key` in its seed `auth_config` and the operator must
    // populate it before downloads will authenticate. Blocking
    // auth_config writes would mean operators can never set the HF
    // token without an out-of-band migration. The URL/auth_type/name
    // immutability is sufficient — a malicious operator can supply a
    // bad api_key, but that exfiltrates only to the legitimate (still
    // pinned) HF URL.
    if current_repository.built_in {
        let touches_immutable = request.url.is_some()
            || request.auth_type.is_some()
            || request.name.is_some();
        if touches_immutable {
            return Err(AppError::bad_request(
                "BUILT_IN_REPOSITORY",
                "Cannot modify name / URL / auth_type on built-in repositories",
            )
            .into());
        }
    }

    // Validate auth fields based on auth type (use current or new values)
    utils::validate_auth_config_for_update(&current_repository, &request)?;

    // Snapshot the prior `enabled` state BEFORE the update so the
    // health enforcement can detect a false→true transition.
    let old_enabled = current_repository.enabled;

    // Update repository
    let updated_repository = Repos.llm_repository
        .update(repository_id, request)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e
                && db.is_unique_violation()
            {
                let field = match db.constraint() {
                    Some("llm_repositories_url_key") => "URL",
                    Some(c) if c.contains("name") => "name",
                    _ => "name or URL",
                };
                return AppError::bad_request(
                    "DUPLICATE_REPOSITORY",
                    format!("A repository with this {field} already exists"),
                );
            }
            tracing::error!("Failed to update repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;

    // Emit the update event BEFORE the enable-transition probe so
    // listeners see the canonical edit; the AutoDisabled event (if
    // probe fails) emits separately from `enforce_on_update_transition`.
    event_bus.emit_async(LlmRepositoryEvent::updated(updated_repository.clone()).into());

    // On a false→true enable transition, probe the persisted config.
    // On failure: revert `enabled` to false in the DB + return 400
    // with the failure reason. Other fields the user updated in the
    // same PUT stay persisted — the partial save is preferable to
    // losing every concurrent edit.
    let enforced = connection_health::enforce_on_update_transition(
        updated_repository,
        old_enabled,
        &event_bus,
    )
    .await?;

    // Cross-device sync: publish AFTER the probe so subscribers reload
    // and see the post-probe `enabled` state.
    sync_publish(
        SyncEntity::LlmRepository,
        SyncAction::Update,
        repository_id,
        Audience::perm::<LlmRepositoriesRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(enforced)))
}

/// Documentation for update_repository endpoint
pub fn update_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesEdit,)>(op)
        .id("LlmRepository.update")
        .tag("LLM Repositories")
        .summary("Update an existing LLM repository")
        .response::<200, Json<LlmRepository>>()
        .response_with::<400, (), _>(|res| res.description("Invalid input"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Delete an LLM repository (requires llm_repositories::delete permission)
/// Built-in repositories cannot be deleted
#[debug_handler]
pub async fn delete_repository(
    _auth: RequirePermissions<(LlmRepositoriesDelete,)>,
    Path(repository_id): Path<Uuid>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    // Get repository name before deletion for event. Propagate a real DB
    // error instead of masking it behind a synthetic "Unknown" name (which
    // would emit a misleading event and hide the failure from logs).
    let repository_name = Repos
        .llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch repository {} for delete: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?
        .name
        .clone();

    match Repos.llm_repository.delete(repository_id).await {
        Ok(Ok(true)) => {
            // Emit event
            event_bus
                .emit_async(LlmRepositoryEvent::deleted(repository_id, repository_name).into());
            sync_publish(SyncEntity::LlmRepository, SyncAction::Delete, repository_id, Audience::perm::<LlmRepositoriesRead>(), origin.0);
            Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
        }
        Ok(Ok(false)) => Err(AppError::not_found("Repository").into()),
        Ok(Err(error_message)) => {
            tracing::error!(
                "Cannot delete repository {}: {}",
                repository_id, error_message
            );
            Err(
                AppError::bad_request("INVALID_OPERATION", "Cannot delete built-in repository")
                    .into(),
            )
        }
        Err(e) => {
            tracing::error!("Failed to delete repository {}: {}", repository_id, e);
            Err(AppError::internal_error("Database operation failed").into())
        }
    }
}

/// Documentation for delete_repository endpoint
pub fn delete_repository_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesDelete,)>(op)
        .id("LlmRepository.delete")
        .tag("LLM Repositories")
        .summary("Delete an LLM repository")
        .response::<204, ()>()
        .response_with::<400, (), _>(|res| res.description("Cannot delete built-in repository"))
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

/// Test LLM repository connection (requires llm_repositories::read permission)
/// Tests connectivity with provided credentials without saving
/// ALL logic preserved from react-test including Hugging Face special handling
#[debug_handler]
pub async fn test_repository_connection(
    _auth: RequirePermissions<(LlmRepositoriesRead,)>,
    Json(request): Json<TestRepositoryConnectionRequest>,
) -> ApiResult<Json<TestRepositoryConnectionResponse>> {
    // Validate URL format
    if utils::validate_url(&request.url).is_err() {
        return Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: "Invalid URL format".to_string(),
            }),
        ));
    }

    // Test the repository connection
    match utils::test_repository_connectivity(&request).await {
        Ok(()) => Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: true,
                message: format!("Connection to {} successful", request.name),
            }),
        )),
        Err(e) => Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: format!("Connection to {} failed: {}", request.name, e),
            }),
        )),
    }
}

/// Documentation for test_repository_connection endpoint
pub fn test_repository_connection_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesRead,)>(op)
        .id("LlmRepository.test")
        .tag("LLM Repositories")
        .summary("Test repository connection without saving")
        .response::<200, Json<TestRepositoryConnectionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
}

/// Test an EXISTING repository's connection, recording the outcome to
/// `last_health_check_*` columns. Form overrides in the request body
/// merge over the persisted row using the same `merge_over` /
/// `pruned_for` semantics the update handler uses — form-empty secret
/// fields fall back to the persisted (decrypted) secret so the user
/// doesn't have to re-type write-only credentials.
///
/// Side effects (mirror `connection_health::enforce_on_*`):
/// - Always: record health-check outcome (status + reason + timestamp).
/// - Probe failure on a currently-enabled row: auto-disable + emit
///   `auto_disabled` event so the list / drawer reflect the flip
///   in real time. Probe failure on a disabled row: just record
///   unhealthy, no enable mutation, no event.
/// - Probe success: emit `updated` so the drawer's `editingRepository`
///   subscription picks up the new `last_health_check_*` fields and
///   the inline Alert clears without a page reload.
///
/// Cross-target secret guard: when EITHER the form override changes the
/// repository's `url` OR the `auth_test_api_endpoint` (the URL the
/// probe actually POSTs to when set), secret fallback is DROPPED. The
/// probe's actual destination is `auth_test_api_endpoint` when
/// configured, so guarding only on `url` would leak the persisted
/// token: an operator with `edit` permission could leave `url`
/// unchanged, point `auth_test_api_endpoint` at attacker.example, and
/// observe the persisted Hugging Face / GitHub token POSTed to that
/// host. Mirrors the URL-match guard in MCP's `resolve_oauth` at
/// `mcp/handlers/test_connection.rs:252-269`, generalized for the LLM
/// repository's two-URL surface.
///
/// SSRF: after the merge, `auth_test_api_endpoint` is run through the
/// same allowlist (`utils::validate_test_endpoint`) the update path
/// uses, so the probe can't be steered to RFC1918 / `169.254.x` /
/// loopback hosts.
///
/// Side effects (mirror `connection_health::record_test_outcome`):
/// records the probe outcome on `last_health_check_*`; on
/// failure-of-currently-enabled-row, auto-disables + emits
/// `auto_disabled`. On any other outcome, emits `updated` so the
/// drawer's `editingRepository` re-syncs with the new health columns.
#[debug_handler]
pub async fn test_repository_connection_by_id(
    _auth: RequirePermissions<(LlmRepositoriesEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(repository_id): Path<Uuid>,
    origin: SyncOrigin,
    Json(overrides): Json<UpdateLlmRepositoryRequest>,
) -> ApiResult<Json<TestRepositoryConnectionResponse>> {
    // 1. Load persisted row (`get_by_id` returns the decrypted
    //    auth_config — that's how the runtime spawn path reads it too).
    let mut working = Repos
        .llm_repository
        .get_by_id(repository_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get repository {}: {}", repository_id, e);
            AppError::internal_error("Database operation failed")
        })?
        .ok_or_else(|| AppError::not_found("Repository"))?;
    let old_enabled = working.enabled;

    // 2a. Snapshot the PRE-mutation state for ALL fields the
    //     security guard / pruning logic reads. Without these
    //     snapshots:
    //   - `auth_type` pruning would compare `working.auth_type`
    //     against itself (a no-op) because the override has already
    //     overwritten it by the time the comparison runs.
    //   - The cross-target secret guard would compare the post-
    //     override `auth_test_api_endpoint` against itself.
    let old_url = working.url.clone();
    let old_auth_type = working.auth_type.clone();
    let old_test_endpoint = working.auth_config.auth_test_api_endpoint.clone();

    // 2b. Apply non-secret overrides. `url` validation rejects
    //     malformed schemes (`javascript:`, etc.) so we never POST
    //     the persisted secret to a bogus target even if the form
    //     supplied one.
    if let Some(name) = overrides.name.clone() {
        working.name = name;
    }
    if let Some(url) = overrides.url.clone() {
        if utils::validate_url(&url).is_err() {
            return Ok((
                StatusCode::OK,
                Json(TestRepositoryConnectionResponse {
                    success: false,
                    message: "Invalid URL format".to_string(),
                }),
            ));
        }
        working.url = url;
    }
    if let Some(auth_type) = overrides.auth_type.clone() {
        working.auth_type = auth_type;
    }

    // 2c. Merge the form's auth_config over the persisted one. When
    //     EITHER URL target shifted (the row's main `url` OR the
    //     probe-time `auth_test_api_endpoint`), the persisted secret
    //     MUST NOT flow through — the form must supply credentials
    //     for the new host explicitly. See module-level doc comment
    //     above for the threat model.
    let url_changed = working.url != old_url;
    if let Some(form_auth) = overrides.auth_config {
        let test_endpoint_changed = matches!(
            &form_auth.auth_test_api_endpoint,
            Some(form_ep) if Some(form_ep) != old_test_endpoint.as_ref()
        );
        let drop_secret_base = url_changed || test_endpoint_changed;
        let base = if drop_secret_base {
            super::models::RepositoryAuthConfig::default()
        } else {
            working.auth_config.clone()
        };
        let merged = form_auth.merge_over(&base);
        // auth_type-switch pruning: when the form switched to a new
        // auth_type, drop secret fields belonging to the PREVIOUS
        // type. Compares against `old_auth_type` (the snapshot)
        // because `working.auth_type` was already overwritten above.
        working.auth_config = match overrides.auth_type.as_deref() {
            Some(new_type) if new_type != old_auth_type.as_str() => {
                merged.pruned_for(new_type)
            }
            _ => merged,
        };
    }

    // 2d. SSRF guard on the FINAL auth_test_api_endpoint. The update
    //     handler validates this before persisting; this path
    //     bypasses persistence, so we run the same allowlist here so
    //     the probe can't be steered at RFC1918 / IMDS / loopback.
    if let Err(e) = utils::validate_test_endpoint(
        &working.auth_config.auth_test_api_endpoint,
    ) {
        return Ok((
            StatusCode::OK,
            Json(TestRepositoryConnectionResponse {
                success: false,
                message: format!("Invalid auth_test_api_endpoint: {}", e),
            }),
        ));
    }

    // 3. Probe via the same code path as boot / enable-transition /
    //    create-flow. One probe implementation, one set of error
    //    semantics — manual test green = runtime green.
    let probe_result = connection_health::probe(&working).await;
    let working_name = working.name.clone();

    // 4. Hand off to the shared bookkeeping helper: records the
    //    outcome on `last_health_check_*`; on failure-of-enabled-row,
    //    auto-disables + emits `auto_disabled`. The helper's branch
    //    matrix is unit-tested in `connection_health.rs::tests`.
    let ops = connection_health::ProductionHealthOps::new(&event_bus);
    let outcome = connection_health::record_test_outcome(
        repository_id,
        old_enabled,
        probe_result,
        &ops,
    )
    .await?;

    // 5. If the helper DIDN'T already emit `auto_disabled` (success
    //    path, failure-on-already-disabled, OR failure-on-enabled
    //    where the disable itself failed), emit `updated` so the
    //    drawer's editingRepository subscription picks up the new
    //    `last_health_check_*` columns and the Alert clears /
    //    refreshes without a page reload. We avoid double-firing for
    //    the auto-disabled case — the store's auto_disabled listener
    //    already reloads the list.
    if !outcome.already_emitted_auto_disabled {
        match Repos.llm_repository.get_by_id(repository_id).await {
            Ok(Some(refreshed)) => {
                event_bus.emit_async(LlmRepositoryEvent::updated(refreshed).into());
            }
            Ok(None) => {
                // Row was deleted concurrently between probe + refetch.
                // No event to emit; the listener stores will catch up
                // on their own delete-event paths.
            }
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    repo_id = %repository_id,
                    "llm_repository::health: failed to refetch row for updated emit (non-fatal)",
                );
            }
        }
    }

    // Cross-device sync: the probe persisted new `last_health_check_*`
    // columns (and may have auto-disabled the row), so notify other
    // surfaces to reload — mirrors update_repository / delete.
    sync_publish(
        SyncEntity::LlmRepository,
        SyncAction::Update,
        repository_id,
        Audience::perm::<LlmRepositoriesRead>(),
        origin.0,
    );

    let message = if outcome.success {
        format!("Connection to {} successful", working_name)
    } else {
        format!(
            "Connection to {} failed: {}",
            working_name,
            outcome.reason.as_deref().unwrap_or("(unknown)")
        )
    };
    Ok((
        StatusCode::OK,
        Json(TestRepositoryConnectionResponse {
            success: outcome.success,
            message,
        }),
    ))
}

pub fn test_repository_connection_by_id_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(LlmRepositoriesEdit,)>(op)
        .id("LlmRepository.testById")
        .tag("LLM Repositories")
        .summary("Test an existing repository's connection and record the outcome")
        .description(
            "Probe a saved repository's connection. Form overrides in the request body merge \
             over the persisted row; empty secret fields fall back to the saved secret. The \
             outcome is recorded to `last_health_check_*` columns. A currently-enabled row \
             that fails the probe is auto-disabled.",
        )
        .response::<200, Json<TestRepositoryConnectionResponse>>()
        .response_with::<401, (), _>(|res| res.description("Unauthorized"))
        .response_with::<404, (), _>(|res| res.description("Repository not found"))
}

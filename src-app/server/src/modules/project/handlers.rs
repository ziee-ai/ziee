// Project handlers.

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Path, Query},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::events::ProjectEvent;
use super::models::Project;
use super::permissions::*;
use super::types::{CreateProjectRequest, ProjectListResponse, UpdateProjectRequest};
use crate::common::{ApiResult, AppError};
use crate::core::{EventBus, Repos};
use crate::modules::permissions::{extractors::RequirePermissions, with_permission};

// =====================================================
// Query parameters
// =====================================================

const PROJECT_MAX_LIMIT: i64 = 100;

/// Pagination params for project list endpoints. Both `page` and
/// `limit` are **optional** in the wire schema (defaults: page=1,
/// limit=20). The custom `Deserialize` clamps values into [1, 100]
/// silently so callers can't cause unbounded materialization.
///
/// The fields are `Option<i64>` in the schema-visible layout so the
/// generated OpenAPI marks them `required: false` (closes audit N15).
/// The handler bodies call `.resolved()` to get clamped i64s.
#[derive(Debug, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed). Defaults to 1.
    pub page: Option<i64>,
    /// Items per page. Defaults to 20, clamped to [1, 100].
    pub limit: Option<i64>,
}

/// Upper bound on the page number a caller can request. With
/// `PROJECT_MAX_LIMIT = 100`, this lets a client paginate up to 100M
/// rows — well past anything a user could plausibly own, but tight
/// enough that `(page-1) * limit` cannot overflow i64 even on
/// adversarial input.
const PROJECT_MAX_PAGE: i64 = 1_000_000;

impl PaginationQuery {
    /// Resolve optional + raw values into the clamped (page, limit)
    /// pair the handlers use. Page is clamped to [1, PROJECT_MAX_PAGE]
    /// so the downstream `(page-1) * limit` cannot overflow.
    pub fn resolved(&self) -> (i64, i64) {
        (
            self.page.unwrap_or(1).clamp(1, PROJECT_MAX_PAGE),
            self.limit.unwrap_or(20).clamp(1, PROJECT_MAX_LIMIT),
        )
    }
}

impl<'de> Deserialize<'de> for PaginationQuery {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            page: Option<i64>,
            limit: Option<i64>,
        }
        let raw = Raw::deserialize(d)?;
        Ok(PaginationQuery {
            page: raw.page,
            limit: raw.limit,
        })
    }
}

// =====================================================
// Validation
// =====================================================

const PROJECT_MAX_INSTRUCTIONS_BYTES: usize = 65_536;
const PROJECT_MAX_DESCRIPTION_BYTES: usize = 4_096;

/// Reject names that are empty or whitespace-only. Extracted from the
/// create-handler body so it's Tier-1 unit-testable independently of
/// the HTTP layer.
fn validate_project_name(name: &str) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Project name cannot be empty",
        ));
    }
    if name.len() > 255 {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Project name must be ≤ 255 characters",
        ));
    }
    Ok(())
}

fn validate_project_text_lengths(
    description: Option<&str>,
    instructions: Option<&str>,
) -> Result<(), AppError> {
    if let Some(d) = description
        && d.len() > PROJECT_MAX_DESCRIPTION_BYTES
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!(
                "description exceeds {} bytes",
                PROJECT_MAX_DESCRIPTION_BYTES
            ),
        ));
    }
    if let Some(i) = instructions
        && i.len() > PROJECT_MAX_INSTRUCTIONS_BYTES
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!(
                "instructions exceeds {} bytes",
                PROJECT_MAX_INSTRUCTIONS_BYTES
            ),
        ));
    }
    Ok(())
}

// =====================================================
// CRUD handlers
// =====================================================

/// Verify a default_assistant_id (if set) is one the user can actually
/// use — their own assistant OR a public template. Mirrors the
/// security model in `extensions/assistant/assistant.rs` (closes
/// 04-chat F-02 High). Returns 422 if the FK is dangling or foreign.
async fn validate_default_assistant_access(
    user_id: Uuid,
    assistant_id: Option<Uuid>,
) -> Result<(), AppError> {
    // Note: get_for_user filters assistants by user ownership or
    // public-template flag — archived/disabled assistants are
    // rejected implicitly through that filter.
    if let Some(aid) = assistant_id
        && Repos.assistant.get_for_user(aid, user_id).await?.is_none()
    {
        return Err(AppError::unprocessable_entity(
            "DEFAULT_ASSISTANT_INACCESSIBLE",
            "The selected assistant is not available. You can only choose your own \
             assistants or shared templates. Pick a different assistant or create one.",
        ));
    }
    Ok(())
}

// `validate_mcp_server_access` moved to
// `modules/mcp/project_extension/handlers.rs` (the only place that
// still validates server access — the PUT /mcp-settings handler).
// Project create no longer accepts MCP fields, so no validation at
// create time.

/// Verify a default_model_id (if set) exists. Per project memory
/// `llm_models_system_wide`, models are admin-curated and shared
/// across users — there's no per-user access column to check. We
/// only verify the FK isn't dangling so the project save can't
/// silently store a deleted model id. The actual access gate is at
/// chat send time (provider group assignments).
async fn validate_default_model_exists(model_id: Option<Uuid>) -> Result<(), AppError> {
    if let Some(mid) = model_id
        && Repos.llm_model.get_by_id(mid).await?.is_none()
    {
        return Err(AppError::unprocessable_entity(
            "DEFAULT_MODEL_NOT_FOUND",
            "default_model_id refers to a model that no longer exists",
        ));
    }
    Ok(())
}

#[debug_handler]
pub async fn create_project(
    auth: RequirePermissions<(ProjectsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(request): Json<CreateProjectRequest>,
) -> ApiResult<Json<Project>> {
    validate_project_name(&request.name)?;
    validate_project_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;
    // MCP validation moved to mcp/project_extension's
    // update_project_mcp_settings handler — project create no longer
    // accepts MCP fields (they're set via separate PUT after create).
    validate_default_assistant_access(auth.user.id, request.default_assistant_id).await?;
    validate_default_model_exists(request.default_model_id).await?;

    let project = Repos.project.create(auth.user.id, request).await?;
    tracing::info!(
        project_id = %project.id,
        user_id = %auth.user.id,
        "project: created"
    );
    event_bus.emit_async(ProjectEvent::created(project.id, auth.user.id));

    Ok((StatusCode::CREATED, Json(project)))
}

pub fn create_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsCreate,)>(op)
        .id("Project.create")
        .tag("Projects")
        .summary("Create a new chat project")
        .description(
            "Create a personal chat project. The project is owned by the authenticated user.\n\
             \n\
             Error codes (in the `error_code` response field):\n\
             - `VALIDATION_ERROR` (400) — name empty, instructions/description over caps, malformed MCP shapes.\n\
             - `DEFAULT_ASSISTANT_INACCESSIBLE` (422) — default_assistant_id isn't owned by user or a public template.\n\
             - `DEFAULT_MODEL_NOT_FOUND` (422) — default_model_id refers to a non-existent model.",
        )
        .response::<201, Json<Project>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<422, (), _>(|res| res.description("Default-asset access denied or not found"))
}

#[debug_handler]
pub async fn list_projects(
    auth: RequirePermissions<(ProjectsRead,)>,
    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<ProjectListResponse>> {
    let (page, limit) = query.resolved();
    let response = Repos
        .project
        .list_for_user(auth.user.id, page, limit)
        .await?;
    Ok((StatusCode::OK, Json(response)))
}

pub fn list_projects_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.list")
        .tag("Projects")
        .summary("List user's projects")
        .response::<200, Json<ProjectListResponse>>()
}

#[debug_handler]
pub async fn get_project(
    auth: RequirePermissions<(ProjectsRead,)>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    let project = Repos
        .project
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Project"))?;
    Ok((StatusCode::OK, Json(project)))
}

pub fn get_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsRead,)>(op)
        .id("Project.get")
        .tag("Projects")
        .summary("Get project by ID")
        .response::<200, Json<Project>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
}

#[debug_handler]
pub async fn update_project(
    auth: RequirePermissions<(ProjectsEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateProjectRequest>,
) -> ApiResult<Json<Project>> {
    if let Some(name) = &request.name {
        validate_project_name(name)?;
        // Pre-flight name uniqueness check (audit N8) so renaming to a
        // name already taken by the same user returns 422
        // PROJECT_NAME_DUPLICATE instead of a 500 unique-constraint
        // violation. The DB constraint is the final backstop if a race
        // beats this check.
        let collision: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM projects WHERE user_id = $1 AND name = $2 AND id != $3",
            auth.user.id,
            name,
            id
        )
        .fetch_one(Repos.pool())
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(0);
        if collision > 0 {
            return Err(AppError::unprocessable_entity(
                "PROJECT_NAME_DUPLICATE",
                format!("A project named \"{}\" already exists", name),
            )
            .into());
        }
    }
    validate_project_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;
    // Tri-state default_assistant_id: Some(Some(uuid)) = set + validate;
    // Some(None) = clear (skip); None = no change.
    if let Some(Some(aid)) = request.default_assistant_id {
        validate_default_assistant_access(auth.user.id, Some(aid)).await?;
    }
    if let Some(Some(mid)) = request.default_model_id {
        validate_default_model_exists(Some(mid)).await?;
    }

    let project = Repos.project.update(id, auth.user.id, request).await?;
    event_bus.emit_async(ProjectEvent::updated(project.id, auth.user.id));

    Ok((StatusCode::OK, Json(project)))
}

pub fn update_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsEdit,)>(op)
        .id("Project.update")
        .tag("Projects")
        .summary("Update project")
        .description(
            "Update a project's fields. All fields optional.\n\
             \n\
             Error codes:\n\
             - `VALIDATION_ERROR` (400) — name empty, caps exceeded.\n\
             - `PROJECT_NAME_DUPLICATE` (422) — renaming would collide with another of your projects.\n\
             - `DEFAULT_ASSISTANT_INACCESSIBLE` (422) — see create_project.\n\
             - `DEFAULT_MODEL_NOT_FOUND` (422) — see create_project.",
        )
        .response::<200, Json<Project>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
        .response_with::<422, (), _>(|res| res.description("Name collision or default-asset error"))
}

#[debug_handler]
pub async fn delete_project(
    auth: RequirePermissions<(ProjectsDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let deleted = Repos.project.delete(id, auth.user.id).await?;
    if !deleted {
        return Err(AppError::not_found("Project").into());
    }
    tracing::info!(
        project_id = %id,
        user_id = %auth.user.id,
        "project: deleted"
    );
    event_bus.emit(ProjectEvent::deleted(id, auth.user.id)).await;
    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsDelete,)>(op)
        .id("Project.delete")
        .tag("Projects")
        .summary("Delete project")
        .description(
            "Delete a project. Conversations under the project are preserved with project_id = NULL \
             (no longer receive project knowledge or instructions on future sends).",
        )
        .response_with::<204, (), _>(|res| res.description("Project deleted"))
        .response_with::<404, (), _>(|res| res.description("Project not found"))
}

#[debug_handler]
pub async fn duplicate_project(
    auth: RequirePermissions<(ProjectsCreate, ProjectsRead)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Extension(extension_registry): Extension<Arc<crate::modules::project::ProjectExtensionRegistry>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Project>> {
    // Open a single outer transaction so the project row insert AND every
    // extension's `on_project_duplicated` hook (e.g. file module cloning
    // project_files rows) share atomicity. If any extension errors, the
    // commit is never reached and the duplicate fails as a whole.
    let mut tx = Repos
        .pool()
        .begin()
        .await
        .map_err(AppError::database_error)?;
    let project = Repos
        .project
        .duplicate_in_tx(&mut tx, id, auth.user.id)
        .await?;
    extension_registry
        .fire_on_project_duplicated(id, project.id, &mut tx)
        .await?;
    tx.commit().await.map_err(AppError::database_error)?;
    event_bus.emit_async(ProjectEvent::created(project.id, auth.user.id));
    Ok((StatusCode::CREATED, Json(project)))
}

pub fn duplicate_project_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ProjectsCreate, ProjectsRead)>(op)
        .id("Project.duplicate")
        .tag("Projects")
        .summary("Duplicate a project")
        .description(
            "Clone a project's instructions + files + defaults into a new project with a \
             \" (copy)\" suffix on the name. Does NOT copy conversations or messages.\n\
             \n\
             Error codes:\n\
             - `PROJECT_DUPLICATE_LIMIT` (422) — too many \"(copy N)\" variants already exist (limit 999).",
        )
        .response::<201, Json<Project>>()
        .response_with::<404, (), _>(|res| res.description("Project not found"))
        .response_with::<422, (), _>(|res| res.description("Duplicate suffix limit reached"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_under_cap_passes() {
        let ok = "x".repeat(PROJECT_MAX_DESCRIPTION_BYTES);
        assert!(validate_project_text_lengths(Some(&ok), None).is_ok());
    }

    #[test]
    fn description_over_cap_rejected() {
        let over = "x".repeat(PROJECT_MAX_DESCRIPTION_BYTES + 1);
        let err = validate_project_text_lengths(Some(&over), None).unwrap_err();
        // The bad_request constructor surfaces a 400 with a code we can
        // probe via the error's debug form; we don't need the exact
        // wire shape here — just that it rejects.
        let s = format!("{:?}", err);
        assert!(
            s.contains("VALIDATION_ERROR") || s.contains("description"),
            "expected validation error, got: {s}"
        );
    }

    #[test]
    fn instructions_under_cap_passes() {
        let ok = "x".repeat(PROJECT_MAX_INSTRUCTIONS_BYTES);
        assert!(validate_project_text_lengths(None, Some(&ok)).is_ok());
    }

    #[test]
    fn instructions_over_cap_rejected() {
        let over = "x".repeat(PROJECT_MAX_INSTRUCTIONS_BYTES + 1);
        let err = validate_project_text_lengths(None, Some(&over)).unwrap_err();
        let s = format!("{:?}", err);
        assert!(
            s.contains("VALIDATION_ERROR") || s.contains("instructions"),
            "expected validation error, got: {s}"
        );
    }

    #[test]
    fn validator_accepts_none() {
        // Both None = no fields to validate = pass.
        assert!(validate_project_text_lengths(None, None).is_ok());
    }

    #[test]
    fn validator_accepts_empty_strings() {
        assert!(validate_project_text_lengths(Some(""), Some("")).is_ok());
    }

    // `project_max_files_is_one_hundred` test relocated to
    // `modules/file/project_extension/repository.rs` along with the
    // `PROJECT_MAX_FILES` constant (project↔file inversion).

    /// Description cap matches the assistant module's cap so messages
    /// that include both don't get rejected by one and accepted by the
    /// other inconsistently.
    #[test]
    fn description_cap_matches_assistant_module() {
        assert_eq!(PROJECT_MAX_DESCRIPTION_BYTES, 4_096);
    }

    /// Instructions cap matches the assistant module's cap for the same
    /// reason.
    #[test]
    fn instructions_cap_matches_assistant_module() {
        assert_eq!(PROJECT_MAX_INSTRUCTIONS_BYTES, 65_536);
    }

    // ─── name validator ───────────────────────────────────────────

    #[test]
    fn name_validator_rejects_empty() {
        assert!(validate_project_name("").is_err());
    }

    #[test]
    fn name_validator_rejects_whitespace_only() {
        assert!(validate_project_name("   ").is_err());
        assert!(validate_project_name("\t\n").is_err());
    }

    #[test]
    fn name_validator_accepts_one_char() {
        assert!(validate_project_name("x").is_ok());
    }

    #[test]
    fn name_validator_accepts_255() {
        let ok = "x".repeat(255);
        assert!(validate_project_name(&ok).is_ok());
    }

    #[test]
    fn name_validator_rejects_over_255() {
        let over = "x".repeat(256);
        assert!(validate_project_name(&over).is_err());
    }

    #[test]
    fn name_validator_accepts_leading_trailing_whitespace_with_content() {
        // "  Foo  " is trimmed for the empty check but the value itself
        // is kept (the DB stores the raw string). Validator's contract:
        // "trim() must produce ≥ 1 char". The "Foo" inside satisfies it.
        assert!(validate_project_name("  Foo  ").is_ok());
    }
}

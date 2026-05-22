// Assistant handlers - separate routes for user assistants and template assistants

use aide::transform::TransformOperation;
use axum::{
    Json, debug_handler,
    extract::{Extension, Path, Query},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::{
    events::AssistantEvent,
    models::Assistant,
    permissions::*,
    types::{AssistantListResponse, CreateAssistantRequest, UpdateAssistantRequest},
};
use crate::{
    common::{ApiResult, AppError},
    core::{EventBus, Repos},
    modules::permissions::{extractors::RequirePermissions, with_permission},
};

// =====================================================
// Query Parameters
// =====================================================

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: i64,

    /// Items per page
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_page() -> i64 {
    1
}
fn default_limit() -> i64 {
    20
}

// =====================================================
// USER ASSISTANT HANDLERS
// =====================================================

/// Create a new user assistant
#[debug_handler]
pub async fn create_user_assistant(
    auth: RequirePermissions<(AssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(mut request): Json<CreateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    // Validate name is not empty
    if request.name.trim().is_empty() {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "Assistant name cannot be empty").into(),
        );
    }

    // Force is_template to false for user assistants
    request.is_template = Some(false);

    let assistant = Repos.assistant.create(Some(auth.user.id), request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(AssistantEvent::created(assistant.id, Some(auth.user.id)));

    Ok((StatusCode::CREATED, Json(assistant)))
}

pub fn create_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsCreate,)>(op)
        .id("Assistant.create")
        .tag("Assistants")
        .summary("Create a new user assistant")
        .description(
            "Create a user assistant. The assistant will be owned by the authenticated user.",
        )
        .response::<201, Json<Assistant>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}

/// List user's assistants
#[debug_handler]
pub async fn list_user_assistants(
    auth: RequirePermissions<(AssistantsRead,)>,

    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<AssistantListResponse>> {
    let response = Repos
        .assistant
        .list(
            Some(auth.user.id),
            false, // Only user assistants (never returns templates)
            query.page,
            query.limit,
        )
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn list_user_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsRead,)>(op)
        .id("Assistant.list")
        .tag("Assistants")
        .summary("List user assistants")
        .description("List all assistants owned by the authenticated user.")
        .response::<200, Json<AssistantListResponse>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}

/// Get user assistant by ID
#[debug_handler]
pub async fn get_user_assistant(
    auth: RequirePermissions<(AssistantsRead,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<Json<Assistant>> {
    let assistant = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant"))?;

    // Check ownership
    if assistant.created_by != Some(auth.user.id) {
        return Err(AppError::forbidden(
            "ACCESS_DENIED",
            "You can only access your own assistants",
        )
        .into());
    }

    // Ensure it's not a template
    if assistant.is_template {
        return Err(AppError::not_found("Assistant").into());
    }

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn get_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsRead,)>(op)
        .id("Assistant.get")
        .tag("Assistants")
        .summary("Get user assistant by ID")
        .description("Get a specific user assistant. Only the owner can access their assistants.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions or not owner"))
        .response_with::<404, (), _>(|res| res.description("Assistant not found"))
}

/// Update user assistant
#[debug_handler]
pub async fn update_user_assistant(
    auth: RequirePermissions<(AssistantsEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    let existing = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant"))?;

    // Check ownership
    if existing.created_by != Some(auth.user.id) {
        return Err(
            AppError::forbidden("ACCESS_DENIED", "You can only edit your own assistants").into(),
        );
    }

    // Ensure it's not a template
    if existing.is_template {
        return Err(AppError::not_found("Assistant").into());
    }

    let assistant = Repos.assistant.update(id, request).await?;

    // Emit update event for other modules to react
    event_bus.emit_async(AssistantEvent::updated(assistant.id, Some(auth.user.id)));

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn update_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsEdit,)>(op)
        .id("Assistant.update")
        .tag("Assistants")
        .summary("Update user assistant")
        .description("Update a user assistant. Only the owner can edit their assistants.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions or not owner"))
        .response_with::<404, (), _>(|res| res.description("Assistant not found"))
}

/// Delete user assistant
#[debug_handler]
pub async fn delete_user_assistant(
    auth: RequirePermissions<(AssistantsDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,

    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let existing = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant"))?;

    // Check ownership
    if existing.created_by != Some(auth.user.id) {
        return Err(AppError::forbidden(
            "ACCESS_DENIED",
            "You can only delete your own assistants",
        )
        .into());
    }

    // Ensure it's not a template
    if existing.is_template {
        return Err(AppError::not_found("Assistant").into());
    }

    Repos.assistant.delete(id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(AssistantEvent::deleted(id, Some(auth.user.id))).await;

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsDelete,)>(op)
        .id("Assistant.delete")
        .tag("Assistants")
        .summary("Delete user assistant")
        .description("Delete a user assistant. Only the owner can delete their assistants.")
        .response_with::<204, (), _>(|res| res.description("Assistant deleted successfully"))
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions or not owner"))
        .response_with::<404, (), _>(|res| res.description("Assistant not found"))
}

/// Get user's default assistant
#[debug_handler]
pub async fn get_default_user_assistant(
    auth: RequirePermissions<(AssistantsRead,)>,
) -> ApiResult<Json<Assistant>> {
    // Get user's default (or fall back to template default)
    let assistant = Repos
        .assistant
        .get_default(Some(auth.user.id))
        .await?
        .ok_or_else(|| AppError::not_found("Default assistant"))?;

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn get_default_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsRead,)>(op)
        .id("Assistant.getDefault")
        .tag("Assistants")
        .summary("Get default user assistant")
        .description("Get the default assistant for the user. Falls back to default template if no user default is set.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Default assistant not found"))
}

// =====================================================
// TEMPLATE ASSISTANT HANDLERS
// =====================================================

/// Create a new template assistant
#[debug_handler]
pub async fn create_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Json(mut request): Json<CreateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    // Validate name is not empty
    if request.name.trim().is_empty() {
        return Err(
            AppError::bad_request("VALIDATION_ERROR", "Assistant name cannot be empty").into(),
        );
    }

    // Force is_template to true for template assistants
    request.is_template = Some(true);

    // Templates have no owner
    let assistant = Repos.assistant.create(None, request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(AssistantEvent::created(assistant.id, None));

    Ok((StatusCode::CREATED, Json(assistant)))
}

pub fn create_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateCreate,)>(op)
        .id("AssistantTemplate.create")
        .tag("Assistant Templates")
        .summary("Create a new template assistant")
        .description("Create a template assistant. Templates are system-wide and have no owner.")
        .response::<201, Json<Assistant>>()
        .response_with::<400, (), _>(|res| res.description("Invalid request"))
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}

/// List template assistants
#[debug_handler]
pub async fn list_template_assistants(
    _auth: RequirePermissions<(AssistantsTemplateRead,)>,

    Query(query): Query<PaginationQuery>,
) -> ApiResult<Json<AssistantListResponse>> {
    let response = Repos
        .assistant
        .list(
            None, // No user filter for templates
            true, // Only templates
            query.page,
            query.limit,
        )
        .await?;

    Ok((StatusCode::OK, Json(response)))
}

pub fn list_template_assistants_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateRead,)>(op)
        .id("AssistantTemplate.list")
        .tag("Assistant Templates")
        .summary("List template assistants")
        .description("List all template assistants. Templates are system-wide assistants available to all users.")
        .response::<200, Json<AssistantListResponse>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
}

/// Get template assistant by ID
#[debug_handler]
pub async fn get_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateRead,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<Json<Assistant>> {
    let assistant = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant template"))?;

    // Ensure it's a template
    if !assistant.is_template {
        return Err(AppError::not_found("Assistant template").into());
    }

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn get_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateRead,)>(op)
        .id("AssistantTemplate.get")
        .tag("Assistant Templates")
        .summary("Get template assistant by ID")
        .description("Get a specific template assistant.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Assistant template not found"))
}

/// Update template assistant
#[debug_handler]
pub async fn update_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    let existing = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant template"))?;

    // Ensure it's a template
    if !existing.is_template {
        return Err(AppError::not_found("Assistant template").into());
    }

    let assistant = Repos.assistant.update(id, request).await?;

    // Emit update event for other modules to react
    event_bus.emit_async(AssistantEvent::updated(assistant.id, None));

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn update_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateEdit,)>(op)
        .id("AssistantTemplate.update")
        .tag("Assistant Templates")
        .summary("Update template assistant")
        .description("Update a template assistant.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Assistant template not found"))
}

/// Delete template assistant
#[debug_handler]
pub async fn delete_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
) -> ApiResult<()> {
    let existing = Repos
        .assistant
        .get(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant template"))?;

    // Ensure it's a template
    if !existing.is_template {
        return Err(AppError::not_found("Assistant template").into());
    }

    Repos.assistant.delete(id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(AssistantEvent::deleted(id, None)).await;

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateDelete,)>(op)
        .id("AssistantTemplate.delete")
        .tag("Assistant Templates")
        .summary("Delete template assistant")
        .description("Delete a template assistant.")
        .response_with::<204, (), _>(|res| {
            res.description("Assistant template deleted successfully")
        })
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Assistant template not found"))
}

/// Get default template assistant
#[debug_handler]
pub async fn get_default_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateRead,)>,
) -> ApiResult<Json<Assistant>> {
    // Get default template
    let assistant = Repos
        .assistant
        .get_default(None)
        .await?
        .ok_or_else(|| AppError::not_found("Default template assistant"))?;

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn get_default_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateRead,)>(op)
        .id("AssistantTemplate.getDefault")
        .tag("Assistant Templates")
        .summary("Get default template assistant")
        .description("Get the default template assistant.")
        .response::<200, Json<Assistant>>()
        .response_with::<403, (), _>(|res| res.description("Insufficient permissions"))
        .response_with::<404, (), _>(|res| res.description("Default template assistant not found"))
}

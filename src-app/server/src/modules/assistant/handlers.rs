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
    modules::sync::{Audience, SyncAction, SyncEntity, SyncOrigin, publish as sync_publish},
};

// =====================================================
// Query Parameters
// =====================================================

/// Maximum page size accepted from the client. Larger values are
/// clamped silently at deserialize to prevent DoS via unbounded
/// result-set materialization. Closes 10-assistant F-03 (Medium).
const ASSISTANT_MAX_LIMIT: i64 = 100;

#[derive(Debug, schemars::JsonSchema)]
pub struct PaginationQuery {
    /// Page number (1-indexed); clamped to ≥1 at deserialize.
    pub page: i64,
    /// Items per page; clamped to [1, ASSISTANT_MAX_LIMIT] at deserialize.
    pub limit: i64,
}

impl<'de> Deserialize<'de> for PaginationQuery {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default = "default_page")]
            page: i64,
            #[serde(default = "default_limit")]
            limit: i64,
        }
        let raw = Raw::deserialize(d)?;
        Ok(PaginationQuery {
            page: raw.page.max(1),
            limit: raw.limit.max(1).min(ASSISTANT_MAX_LIMIT),
        })
    }
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

/// Max length caps for assistant text fields. The same values appear
/// as `#[schemars(length(max = ...))]` annotations on
/// CreateAssistantRequest / UpdateAssistantRequest so OpenAPI
/// consumers see the same numbers. Closes 10-assistant F-02 (Medium):
/// without these, an authenticated user can store multi-MB
/// instructions that the chat path then ships to the LLM on every
/// turn, amplifying token cost.
const ASSISTANT_MAX_INSTRUCTIONS_BYTES: usize = 65_536;
const ASSISTANT_MAX_DESCRIPTION_BYTES: usize = 4_096;

/// Max assistant name length, in characters.
///
/// `assistants.name` is `character varying(255)` (see
/// `assistant/migrations/202607140100_assistant_schema.sql`), and Postgres
/// bounds varchar by CHARACTERS — so this is counted in `chars()`, not bytes.
/// Without this gate a 256-character name overflowed the column and the
/// resulting `22001` surfaced to the client as a generic 500
/// SYSTEM_DATABASE_ERROR. The value also matches the
/// `#[schemars(length(max = 255))]` annotation already on
/// Create/UpdateAssistantRequest, so OpenAPI consumers see the same number.
const ASSISTANT_MAX_NAME_CHARS: usize = 255;

/// Reject empty/whitespace-only, over-long, and control/bidi-bearing assistant
/// names. Extracted so it is Tier-1 unit-testable independently of the HTTP
/// layer (mirrors `validate_assistant_text_lengths` next door).
pub(crate) fn validate_assistant_name(name: &str) -> Result<(), AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Assistant name cannot be empty",
        ));
    }
    if trimmed.chars().count() > ASSISTANT_MAX_NAME_CHARS {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            format!("Assistant name must be ≤ {ASSISTANT_MAX_NAME_CHARS} characters"),
        ));
    }
    // Control / bidi-override / zero-width characters let an assistant name
    // reorder or hide adjacent text wherever the picker is rendered. Same gate
    // the username validator applies (13-misc F-06).
    if trimmed
        .chars()
        .any(|c| c.is_control() || crate::modules::auth::username::is_bidi_or_zero_width(c))
    {
        return Err(AppError::bad_request(
            "VALIDATION_ERROR",
            "Assistant name cannot contain control characters",
        ));
    }
    Ok(())
}

pub(crate) fn validate_assistant_text_lengths(
    description: Option<&str>,
    instructions: Option<&str>,
) -> Result<(), AppError> {
    if let Some(d) = description {
        if d.len() > ASSISTANT_MAX_DESCRIPTION_BYTES {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!(
                    "description exceeds {} bytes",
                    ASSISTANT_MAX_DESCRIPTION_BYTES
                ),
            ));
        }
        // The length cap does not catch U+0000 (a NUL-bearing value can be
        // arbitrarily short), and these are `text` columns, so an unguarded
        // NUL reached the INSERT and came back as a generic 500.
        crate::common::text_guard::reject_nul(d, "description")?;
    }
    if let Some(i) = instructions {
        if i.len() > ASSISTANT_MAX_INSTRUCTIONS_BYTES {
            return Err(AppError::bad_request(
                "VALIDATION_ERROR",
                format!(
                    "instructions exceeds {} bytes",
                    ASSISTANT_MAX_INSTRUCTIONS_BYTES
                ),
            ));
        }
        crate::common::text_guard::reject_nul(i, "instructions")?;
    }
    Ok(())
}

/// Create a new user assistant
#[debug_handler]
pub async fn create_user_assistant(
    auth: RequirePermissions<(AssistantsCreate,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    origin: SyncOrigin,
    Json(mut request): Json<CreateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    validate_assistant_name(&request.name)?;
    validate_assistant_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;

    // Force is_template to false for user assistants
    request.is_template = Some(false);

    let assistant = Repos.assistant.create(Some(auth.user.id), request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(AssistantEvent::created(assistant.id, Some(auth.user.id)));

    sync_publish(
        SyncEntity::Assistant,
        SyncAction::Create,
        assistant.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

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
}

/// Get user assistant by ID
#[debug_handler]
pub async fn get_user_assistant(
    auth: RequirePermissions<(AssistantsRead,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<Json<Assistant>> {
    // Owner-facing read: `get_for_user` enforces ownership AND the
    // `enabled = true` filter, so a soft-disabled assistant 404s here (it
    // remains manageable via the update path). Closes the disabled-filter gap.
    let assistant = Repos
        .assistant
        .get_for_user(id, auth.user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant"))?;

    // Ensure it's not a template (this endpoint serves user assistants only).
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
        .response_with::<404, (), _>(|res| res.description("Assistant not found"))
}

/// Update user assistant
#[debug_handler]
pub async fn update_user_assistant(
    auth: RequirePermissions<(AssistantsEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    // A supplied name is bound-checked before it reaches the varchar(255)
    // column; an absent one leaves the existing name alone.
    if let Some(name) = request.name.as_deref() {
        validate_assistant_name(name)?;
    }
    validate_assistant_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;

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

    sync_publish(
        SyncEntity::Assistant,
        SyncAction::Update,
        assistant.id,
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn update_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsEdit,)>(op)
        .id("Assistant.update")
        .tag("Assistants")
        .summary("Update user assistant")
        .description("Update a user assistant. Only the owner can edit their assistants.")
        .response::<200, Json<Assistant>>()
        .response_with::<404, (), _>(|res| res.description("Assistant not found"))
}

/// Delete user assistant
#[debug_handler]
pub async fn delete_user_assistant(
    auth: RequirePermissions<(AssistantsDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,

    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<()> {
    let existing = Repos
        .assistant
        .get_any(id)
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

    sync_publish(
        SyncEntity::Assistant,
        SyncAction::Delete,
        id,
        Audience::owner(auth.user.id),
        origin.0,
    );

    Ok((StatusCode::NO_CONTENT, ()))
}

pub fn delete_user_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsDelete,)>(op)
        .id("Assistant.delete")
        .tag("Assistants")
        .summary("Delete user assistant")
        .description("Delete a user assistant. Only the owner can delete their assistants.")
        .response_with::<204, (), _>(|res| res.description("Assistant deleted successfully"))
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
    origin: SyncOrigin,
    Json(mut request): Json<CreateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    validate_assistant_name(&request.name)?;
    validate_assistant_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;

    // Force is_template to true for template assistants
    request.is_template = Some(true);

    // Templates have no owner
    let assistant = Repos.assistant.create(None, request).await?;

    // Emit creation event for other modules to react
    event_bus.emit_async(AssistantEvent::created(assistant.id, None));

    sync_publish(
        SyncEntity::AssistantTemplate,
        SyncAction::Create,
        assistant.id,
        Audience::perm::<AssistantsTemplateRead>(),
        origin.0,
    );

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
}

/// Get template assistant by ID
#[debug_handler]
pub async fn get_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateRead,)>,

    Path(id): Path<Uuid>,
) -> ApiResult<Json<Assistant>> {
    let assistant = Repos
        .assistant
        .get_any(id)
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
        .response_with::<404, (), _>(|res| res.description("Assistant template not found"))
}

/// Update template assistant
#[debug_handler]
pub async fn update_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateEdit,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
    Json(request): Json<UpdateAssistantRequest>,
) -> ApiResult<Json<Assistant>> {
    // A supplied name is bound-checked before it reaches the varchar(255)
    // column; an absent one leaves the existing name alone.
    if let Some(name) = request.name.as_deref() {
        validate_assistant_name(name)?;
    }
    validate_assistant_text_lengths(
        request.description.as_deref(),
        request.instructions.as_deref(),
    )?;

    let existing = Repos
        .assistant
        .get_any(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant template"))?;

    // Ensure it's a template
    if !existing.is_template {
        return Err(AppError::not_found("Assistant template").into());
    }

    let assistant = Repos.assistant.update(id, request).await?;

    // Emit update event for other modules to react
    event_bus.emit_async(AssistantEvent::updated(assistant.id, None));

    sync_publish(
        SyncEntity::AssistantTemplate,
        SyncAction::Update,
        assistant.id,
        Audience::perm::<AssistantsTemplateRead>(),
        origin.0,
    );

    Ok((StatusCode::OK, Json(assistant)))
}

pub fn update_template_assistant_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(AssistantsTemplateEdit,)>(op)
        .id("AssistantTemplate.update")
        .tag("Assistant Templates")
        .summary("Update template assistant")
        .description("Update a template assistant.")
        .response::<200, Json<Assistant>>()
        .response_with::<404, (), _>(|res| res.description("Assistant template not found"))
}

/// Delete template assistant
#[debug_handler]
pub async fn delete_template_assistant(
    _auth: RequirePermissions<(AssistantsTemplateDelete,)>,
    Extension(event_bus): Extension<Arc<EventBus>>,
    Path(id): Path<Uuid>,
    origin: SyncOrigin,
) -> ApiResult<()> {
    let existing = Repos
        .assistant
        .get_any(id)
        .await?
        .ok_or_else(|| AppError::not_found("Assistant template"))?;

    // Ensure it's a template
    if !existing.is_template {
        return Err(AppError::not_found("Assistant template").into());
    }

    Repos.assistant.delete(id).await?;

    // Emit deletion event for other modules to react (synchronous so cleanup completes before response)
    event_bus.emit(AssistantEvent::deleted(id, None)).await;

    sync_publish(
        SyncEntity::AssistantTemplate,
        SyncAction::Delete,
        id,
        Audience::perm::<AssistantsTemplateRead>(),
        origin.0,
    );

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
        .response_with::<404, (), _>(|res| res.description("Default template assistant not found"))
}

#[cfg(test)]
mod tests {
    use super::{
        ASSISTANT_MAX_DESCRIPTION_BYTES, ASSISTANT_MAX_INSTRUCTIONS_BYTES,
        ASSISTANT_MAX_NAME_CHARS, validate_assistant_name, validate_assistant_text_lengths,
    };

    // First inline unit coverage for the assistant module: the shared
    // create/update text-length validator (the F-02 token-cost amplification
    // guard). Pure function — no DB, no async.
    #[test]
    fn text_length_validator_accepts_none_and_boundary_sizes() {
        // Both absent → ok.
        assert!(validate_assistant_text_lengths(None, None).is_ok());

        // Exactly at the byte caps → ok (boundary is inclusive: rejects `>` only).
        let max_desc = "d".repeat(ASSISTANT_MAX_DESCRIPTION_BYTES);
        let max_instr = "i".repeat(ASSISTANT_MAX_INSTRUCTIONS_BYTES);
        assert!(
            validate_assistant_text_lengths(Some(&max_desc), Some(&max_instr)).is_ok(),
            "exactly-at-cap text must be accepted"
        );
    }

    #[test]
    fn text_length_validator_rejects_oversized_description() {
        // Only the description is over-cap (instructions None) → the
        // description check is what fires.
        let too_long = "d".repeat(ASSISTANT_MAX_DESCRIPTION_BYTES + 1);
        let err = validate_assistant_text_lengths(Some(&too_long), None)
            .expect_err("over-cap description must be rejected");
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
        assert_eq!(err.status_code(), 400);
    }

    // ─── name validator (D2: >255 overflowed varchar(255) → generic 500) ───

    #[test]
    fn name_validator_rejects_empty_and_whitespace_only() {
        for n in ["", "   ", "\t\n"] {
            let err = validate_assistant_name(n).expect_err("must reject");
            assert_eq!(err.error_code(), "VALIDATION_ERROR");
            assert_eq!(err.status_code(), 400);
        }
    }

    #[test]
    fn name_validator_accepts_one_char_and_exactly_max() {
        assert!(validate_assistant_name("a").is_ok());
        assert!(
            validate_assistant_name(&"a".repeat(ASSISTANT_MAX_NAME_CHARS)).is_ok(),
            "exactly-at-cap name must be accepted (255 fits varchar(255))"
        );
    }

    #[test]
    fn name_validator_rejects_over_max() {
        // The exact D2 boundary observed against the live instance:
        // 255 -> 200 OK, 256 -> 500 SYSTEM_DATABASE_ERROR. Now 256 -> 400.
        for len in [ASSISTANT_MAX_NAME_CHARS + 1, 300, 500] {
            let err = validate_assistant_name(&"b".repeat(len))
                .expect_err("over-cap name must be rejected");
            assert_eq!(err.error_code(), "VALIDATION_ERROR", "len {len}");
            assert_eq!(err.status_code(), 400, "len {len}");
        }
    }

    #[test]
    fn name_validator_bounds_characters_not_bytes() {
        // 255 CJK chars = 765 bytes but fits varchar(255), which counts
        // characters. A byte bound would wrongly reject this.
        let cjk = "\u{4e2d}".repeat(ASSISTANT_MAX_NAME_CHARS);
        assert!(
            cjk.len() > ASSISTANT_MAX_NAME_CHARS,
            "precondition: multibyte"
        );
        assert!(validate_assistant_name(&cjk).is_ok());
        assert!(validate_assistant_name(&"\u{4e2d}".repeat(ASSISTANT_MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn name_validator_measures_after_trimming() {
        let padded = format!("  {}  ", "a".repeat(ASSISTANT_MAX_NAME_CHARS));
        assert!(validate_assistant_name(&padded).is_ok());
    }

    #[test]
    fn name_validator_rejects_control_and_bidi_characters() {
        for n in ["a\u{202E}spoof", "a\u{200B}hidden", "a\u{0007}bell"] {
            assert_eq!(
                validate_assistant_name(n)
                    .expect_err("must reject")
                    .error_code(),
                "VALIDATION_ERROR"
            );
        }
    }

    #[test]
    fn name_validator_allows_ordinary_punctuation() {
        // Negative control: the gate is length + control chars, not a
        // character blacklist.
        assert!(validate_assistant_name("Research Helper (v2) — EN/FR").is_ok());
    }

    #[test]
    fn text_length_validator_rejects_oversized_instructions() {
        // Only the instructions are over-cap (description None) → the
        // instructions check is what fires.
        let too_long = "i".repeat(ASSISTANT_MAX_INSTRUCTIONS_BYTES + 1);
        let err = validate_assistant_text_lengths(None, Some(&too_long))
            .expect_err("over-cap instructions must be rejected");
        assert_eq!(err.error_code(), "VALIDATION_ERROR");
        assert_eq!(err.status_code(), 400);
    }
}

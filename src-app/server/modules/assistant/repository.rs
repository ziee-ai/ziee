// Assistant repository - copied from ziee-chat-ref and adapted for ziee-chat
// Source: ziee-chat-ref/src-tauri/src/database/queries/assistants.rs

use sqlx::PgPool;
use uuid::Uuid;
use chrono::DateTime;

use super::models::{Assistant, AssistantListResponse, CreateAssistantRequest, UpdateAssistantRequest};
use crate::common::AppError;

/// Helper function to convert a database row to Assistant struct
fn row_to_assistant(
    id: uuid::Uuid,
    name: String,
    description: Option<String>,
    instructions: Option<String>,
    parameters: Option<serde_json::Value>,
    created_by: Option<uuid::Uuid>,
    is_template: bool,
    is_default: bool,
    enabled: bool,
    created_at: time::OffsetDateTime,
    updated_at: time::OffsetDateTime,
) -> Assistant {
    Assistant {
        id,
        name,
        description,
        instructions,
        parameters: parameters.unwrap_or_else(|| serde_json::json!({})),
        created_by,
        is_template,
        is_default,
        enabled,
        created_at: DateTime::from_timestamp(created_at.unix_timestamp(), 0).unwrap(),
        updated_at: DateTime::from_timestamp(updated_at.unix_timestamp(), 0).unwrap(),
    }
}

/// Create a new assistant
/// For templates: user_id should be None, is_template=true
/// For user assistants: user_id should be Some(id), is_template=false or omitted
pub async fn create_assistant(
    pool: &PgPool,
    user_id: Option<Uuid>,
    request: CreateAssistantRequest,
) -> Result<Assistant, AppError> {
    let assistant_id = Uuid::new_v4();
    let is_default = request.is_default.unwrap_or(false);
    let is_template = request.is_template.unwrap_or(false);
    let parameters_json = request.parameters_to_json();

    // Start a transaction to handle default assistant logic
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // If this assistant is being set as default, unset all other defaults for the same context
    if is_default {
        if is_template {
            // For template assistants, unset all other default templates
            sqlx::query!("UPDATE assistants SET is_default = false WHERE is_template = true")
                .execute(&mut *tx)
                .await
                .map_err(AppError::database_error)?;
        } else if let Some(uid) = user_id {
            // For user assistants, unset all other default assistants for this user
            sqlx::query!(
                "UPDATE assistants SET is_default = false WHERE created_by = $1 AND is_template = false",
                uid
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
    }

    let row = sqlx::query!(
        r#"INSERT INTO assistants (id, name, description, instructions, parameters, created_by, is_template, is_default)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at"#,
        assistant_id,
        &request.name,
        request.description.as_deref(),
        request.instructions.as_deref(),
        parameters_json,
        user_id,
        is_template,
        is_default
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    let assistant = row_to_assistant(
        row.id,
        row.name,
        row.description,
        row.instructions,
        row.parameters,
        row.created_by,
        row.is_template,
        row.is_default,
        row.enabled,
        row.created_at,
        row.updated_at,
    );

    // Commit the transaction
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(assistant)
}

/// Get assistant by ID
/// Returns the assistant if it exists and is active
/// Does not check ownership - permission check should be done in handler
pub async fn get_assistant(pool: &PgPool, id: Uuid) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
        FROM assistants
        WHERE id = $1 AND enabled = true"#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(row.map(|r| row_to_assistant(
        r.id,
        r.name,
        r.description,
        r.instructions,
        r.parameters,
        r.created_by,
        r.is_template,
        r.is_default,
        r.enabled,
        r.created_at,
        r.updated_at,
    )))
}

/// List assistants with pagination and filtering
///
/// Parameters:
/// - user_id: User ID for ownership filtering (None means no user-specific filtering)
/// - is_template: Filter by template status
///   - None: Return all accessible (user's assistants + all templates)
///   - Some(true): Return only templates
///   - Some(false): Return only user's assistants (requires user_id)
/// - page: Page number (1-indexed)
/// - limit: Items per page
pub async fn list_assistants(
    pool: &PgPool,
    user_id: Option<Uuid>,
    is_template: Option<bool>,
    page: i64,
    limit: i64,
) -> Result<AssistantListResponse, AppError> {
    let offset = (page - 1) * limit;

    // Build query based on filters
    match is_template {
        Some(true) => {
            // Only templates
            let count: i64 = sqlx::query_scalar!(
                "SELECT COUNT(*) FROM assistants WHERE is_template = true AND enabled = true"
            )
            .fetch_one(pool)
            .await
            .map_err(AppError::database_error)?
            .unwrap_or(0);

            let rows = sqlx::query!(
                r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
                 FROM assistants
                 WHERE is_template = true AND enabled = true
                 ORDER BY created_at DESC
                 LIMIT $1 OFFSET $2"#,
                limit,
                offset
            )
            .fetch_all(pool)
            .await
            .map_err(AppError::database_error)?;

            let assistants = rows.into_iter().map(|r| row_to_assistant(
                r.id, r.name, r.description, r.instructions, r.parameters,
                r.created_by, r.is_template, r.is_default, r.enabled,
                r.created_at, r.updated_at
            )).collect();

            return Ok(AssistantListResponse { assistants, total: count });
        }
        Some(false) => {
            // Only user's assistants
            if let Some(uid) = user_id {
                let count: i64 = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM assistants WHERE created_by = $1 AND is_template = false AND enabled = true",
                    uid
                )
                .fetch_one(pool)
                .await
                .map_err(AppError::database_error)?
                .unwrap_or(0);

                let rows = sqlx::query!(
                    r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
                     FROM assistants
                     WHERE created_by = $1 AND is_template = false AND enabled = true
                     ORDER BY created_at DESC
                     LIMIT $2 OFFSET $3"#,
                    uid,
                    limit,
                    offset
                )
                .fetch_all(pool)
                .await
                .map_err(AppError::database_error)?;

                let assistants = rows.into_iter().map(|r| row_to_assistant(
                    r.id, r.name, r.description, r.instructions, r.parameters,
                    r.created_by, r.is_template, r.is_default, r.enabled,
                    r.created_at, r.updated_at
                )).collect();

                return Ok(AssistantListResponse { assistants, total: count });
            } else {
                // No user_id provided but filtering for user assistants - return empty
                return Ok(AssistantListResponse { assistants: vec![], total: 0 });
            }
        }
        None => {
            // All accessible: user's assistants + all templates
            if let Some(uid) = user_id {
                let count: i64 = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM assistants WHERE enabled = true AND (is_template = true OR created_by = $1)",
                    uid
                )
                .fetch_one(pool)
                .await
                .map_err(AppError::database_error)?
                .unwrap_or(0);

                let rows = sqlx::query!(
                    r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
                     FROM assistants
                     WHERE enabled = true AND (is_template = true OR created_by = $1)
                     ORDER BY created_at DESC
                     LIMIT $2 OFFSET $3"#,
                    uid,
                    limit,
                    offset
                )
                .fetch_all(pool)
                .await
                .map_err(AppError::database_error)?;

                let assistants = rows.into_iter().map(|r| row_to_assistant(
                    r.id, r.name, r.description, r.instructions, r.parameters,
                    r.created_by, r.is_template, r.is_default, r.enabled,
                    r.created_at, r.updated_at
                )).collect();

                return Ok(AssistantListResponse { assistants, total: count });
            } else {
                // No user_id - only return templates
                let count: i64 = sqlx::query_scalar!(
                    "SELECT COUNT(*) FROM assistants WHERE is_template = true AND enabled = true"
                )
                .fetch_one(pool)
                .await
                .map_err(AppError::database_error)?
                .unwrap_or(0);

                let rows = sqlx::query!(
                    r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
                     FROM assistants
                     WHERE is_template = true AND enabled = true
                     ORDER BY created_at DESC
                     LIMIT $1 OFFSET $2"#,
                    limit,
                    offset
                )
                .fetch_all(pool)
                .await
                .map_err(AppError::database_error)?;

                let assistants = rows.into_iter().map(|r| row_to_assistant(
                    r.id, r.name, r.description, r.instructions, r.parameters,
                    r.created_by, r.is_template, r.is_default, r.enabled,
                    r.created_at, r.updated_at
                )).collect();

                return Ok(AssistantListResponse { assistants, total: count });
            }
        }
    }
}

/// Update assistant
/// Does not check ownership or permissions - should be done in handler
/// Note: is_template is IMMUTABLE and cannot be changed after creation
pub async fn update_assistant(
    pool: &PgPool,
    id: Uuid,
    request: UpdateAssistantRequest,
) -> Result<Assistant, AppError> {
    // Check if there are any updates
    if !request.has_updates() {
        return get_assistant(pool, id).await?
            .ok_or_else(|| AppError::not_found("Assistant"));
    }

    // Start a transaction
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Get current assistant to check its type for default logic
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
         FROM assistants WHERE id = $1"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Assistant"))?;

    let current = row_to_assistant(
        row.id, row.name, row.description, row.instructions, row.parameters,
        row.created_by, row.is_template, row.is_default, row.enabled,
        row.created_at, row.updated_at
    );

    // If this assistant is being set as default, unset all other defaults for the same context
    if let Some(true) = request.is_default {
        if current.is_template {
            // For template assistants, unset all other default templates
            sqlx::query!(
                "UPDATE assistants SET is_default = false WHERE is_template = true AND id != $1",
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        } else if let Some(user_id) = current.created_by {
            // For user assistants, unset all other default assistants for this user
            sqlx::query!(
                "UPDATE assistants SET is_default = false WHERE created_by = $1 AND is_template = false AND id != $2",
                user_id,
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(AppError::database_error)?;
        }
    }

    // Execute updates for each field individually
    if let Some(name) = &request.name {
        sqlx::query!(
            "UPDATE assistants SET name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            name,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(description) = &request.description {
        sqlx::query!(
            "UPDATE assistants SET description = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            description,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(instructions) = &request.instructions {
        sqlx::query!(
            "UPDATE assistants SET instructions = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            instructions,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(parameters_json) = request.parameters_to_json() {
        sqlx::query!(
            "UPDATE assistants SET parameters = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            parameters_json,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(is_default) = request.is_default {
        sqlx::query!(
            "UPDATE assistants SET is_default = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            is_default,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    if let Some(enabled) = request.enabled {
        sqlx::query!(
            "UPDATE assistants SET enabled = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
            enabled,
            id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    // Fetch the updated assistant
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
         FROM assistants WHERE id = $1"#,
        id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    let assistant = row_to_assistant(
        row.id, row.name, row.description, row.instructions, row.parameters,
        row.created_by, row.is_template, row.is_default, row.enabled,
        row.created_at, row.updated_at
    );

    // Commit the transaction
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(assistant)
}

/// Delete assistant (hard delete)
/// Does not check ownership or permissions - should be done in handler
pub async fn delete_assistant(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query!("DELETE FROM assistants WHERE id = $1", id)
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;

    if result.rows_affected() == 0 {
        return Err(AppError::not_found("Assistant"));
    }

    Ok(())
}

/// Get default assistant for a given context
/// - user_id = None: Get default template assistant
/// - user_id = Some(id): Get user's default assistant (user assistant or template if no user default)
pub async fn get_default_assistant(pool: &PgPool, user_id: Option<Uuid>) -> Result<Option<Assistant>, AppError> {
    if let Some(uid) = user_id {
        // Try to get user's default assistant first
        let user_default_row = sqlx::query!(
            r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
             FROM assistants
             WHERE created_by = $1 AND is_default = true AND enabled = true
             ORDER BY created_at DESC
             LIMIT 1"#,
            uid
        )
        .fetch_optional(pool)
        .await
        .map_err(AppError::database_error)?;

        if let Some(r) = user_default_row {
            return Ok(Some(row_to_assistant(
                r.id, r.name, r.description, r.instructions, r.parameters,
                r.created_by, r.is_template, r.is_default, r.enabled,
                r.created_at, r.updated_at
            )));
        }

        // Fall back to default template if no user default
        let template_default_row = sqlx::query!(
            r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
             FROM assistants
             WHERE is_template = true AND is_default = true AND enabled = true
             ORDER BY created_at DESC
             LIMIT 1"#
        )
        .fetch_optional(pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(template_default_row.map(|r| row_to_assistant(
            r.id, r.name, r.description, r.instructions, r.parameters,
            r.created_by, r.is_template, r.is_default, r.enabled,
            r.created_at, r.updated_at
        )))
    } else {
        // No user context - return default template
        let template_default_row = sqlx::query!(
            r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
             FROM assistants
             WHERE is_template = true AND is_default = true AND enabled = true
             ORDER BY created_at DESC
             LIMIT 1"#
        )
        .fetch_optional(pool)
        .await
        .map_err(AppError::database_error)?;

        Ok(template_default_row.map(|r| row_to_assistant(
            r.id, r.name, r.description, r.instructions, r.parameters,
            r.created_by, r.is_template, r.is_default, r.enabled,
            r.created_at, r.updated_at
        )))
    }
}

/// List all accessible assistants for a user (their own + all templates)
/// This is a convenience function that wraps list_assistants with is_template=None
pub async fn list_accessible_assistants(
    pool: &PgPool,
    user_id: Uuid,
    page: i64,
    limit: i64,
) -> Result<AssistantListResponse, AppError> {
    list_assistants(pool, Some(user_id), None, page, limit).await
}

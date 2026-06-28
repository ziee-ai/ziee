// Assistant repository - copied from ziee-ref and adapted for ziee
// Source: ziee-ref/src-tauri/src/database/queries/assistants.rs

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::models::Assistant;
use super::types::{AssistantListResponse, CreateAssistantRequest, UpdateAssistantRequest};
use crate::common::AppError;

/// Assistant Repository
pub struct AssistantRepository {
    pool: PgPool,
}

impl AssistantRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: Option<Uuid>,
        request: CreateAssistantRequest,
    ) -> Result<Assistant, AppError> {
        create_assistant(&self.pool, user_id, request).await
    }

    /// Atomically delete the given prior-install assistant ids and create the
    /// new one, in ONE transaction — the hub "replace existing install" path.
    /// Previously these were separate awaits, so a create failure after the
    /// deletes left the user with NO assistant. Returns the created assistant
    /// plus the ids that were actually deleted (so the caller can emit
    /// `assistant.deleted` events AFTER the commit).
    pub async fn replace_from_hub(
        &self,
        existing_ids: &[Uuid],
        user_id: Option<Uuid>,
        request: CreateAssistantRequest,
    ) -> Result<(Assistant, Vec<Uuid>), AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let mut deleted = Vec::new();
        for id in existing_ids {
            if delete_assistant_tx(&mut tx, *id).await? {
                deleted.push(*id);
            }
        }
        let assistant = create_assistant_tx(&mut tx, user_id, request).await?;
        tx.commit().await.map_err(AppError::database_error)?;
        Ok((assistant, deleted))
    }

    /// Like [`replace_from_hub`] but ALSO records the new assistant in
    /// `hub_entities` inside the SAME transaction, so the install (delete prior
    /// + create + track) commits atomically. Previously the `track_hub_entity`
    /// call was a separate await after the create tx committed, so a tracking
    /// failure left an untracked assistant (invisible to the Updates tab).
    /// Returns the created assistant, the ids actually deleted, and the
    /// hub-tracking row.
    pub async fn replace_from_hub_tracked(
        &self,
        existing_ids: &[Uuid],
        user_id: Option<Uuid>,
        request: CreateAssistantRequest,
        hub_id: &str,
        hub_version: Option<&str>,
    ) -> Result<
        (
            Assistant,
            Vec<Uuid>,
            crate::modules::hub::models::HubEntity,
        ),
        AppError,
    > {
        use crate::modules::hub::models::{HubCategory, HubEntityType};
        let mut tx = self.pool.begin().await.map_err(AppError::database_error)?;
        let mut deleted = Vec::new();
        for id in existing_ids {
            if delete_assistant_tx(&mut tx, *id).await? {
                deleted.push(*id);
            }
        }
        let assistant = create_assistant_tx(&mut tx, user_id, request).await?;
        let hub_entity = crate::modules::hub::repository::track_hub_entity_in_tx(
            &mut tx,
            HubEntityType::Assistant,
            assistant.id,
            hub_id,
            HubCategory::Assistant,
            user_id,
            hub_version,
        )
        .await?;
        tx.commit().await.map_err(AppError::database_error)?;
        Ok((assistant, deleted, hub_entity))
    }

    pub async fn list(
        &self,
        user_id: Option<Uuid>,
        is_template: bool,
        page: i64,
        limit: i64,
    ) -> Result<AssistantListResponse, AppError> {
        list_assistants(&self.pool, user_id, is_template, page, limit).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Assistant>, AppError> {
        get_assistant(&self.pool, id).await
    }

    /// Get an assistant by ID regardless of `enabled` status.
    /// Used by owner-facing GET/DELETE management handlers so a user can
    /// still view and delete an assistant they have disabled, and by
    /// admin/template management — see [`get_assistant_any`].
    pub async fn get_any(&self, id: Uuid) -> Result<Option<Assistant>, AppError> {
        get_assistant_any(&self.pool, id).await
    }

    /// Get an assistant by ID, scoped to a user. Returns Some only when
    /// the assistant is either owned by `user_id` OR is a public template
    /// (`is_template = TRUE`). Returns None for assistants belonging to
    /// other users, preventing cross-tenant prompt-injection (04-chat
    /// F-02 High).
    ///
    /// Prefer this over `get(id)` in any code path reached by a
    /// per-conversation request; the unscoped `get` should only be used
    /// by admin/system paths.
    pub async fn get_for_user(
        &self,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Assistant>, AppError> {
        get_assistant_for_user(&self.pool, id, user_id).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        request: UpdateAssistantRequest,
    ) -> Result<Assistant, AppError> {
        update_assistant(&self.pool, id, request).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        delete_assistant(&self.pool, id).await
    }

    pub async fn get_default(&self, user_id: Option<Uuid>) -> Result<Option<Assistant>, AppError> {
        get_default_assistant(&self.pool, user_id).await
    }
}

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
        created_at: DateTime::from_timestamp(created_at.unix_timestamp(), 0).unwrap_or_else(Utc::now),
        updated_at: DateTime::from_timestamp(updated_at.unix_timestamp(), 0).unwrap_or_else(Utc::now),
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
    // Start a transaction to handle default assistant logic.
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;
    let assistant = create_assistant_tx(&mut tx, user_id, request).await?;
    tx.commit().await.map_err(AppError::database_error)?;
    Ok(assistant)
}

/// Transaction-scoped assistant create. Runs the clear-defaults UPDATE + the
/// INSERT on the caller's connection so it can be composed atomically with
/// other writes (e.g. the hub "replace existing install" delete-then-create).
/// The caller owns begin/commit.
pub async fn create_assistant_tx(
    conn: &mut sqlx::PgConnection,
    user_id: Option<Uuid>,
    request: CreateAssistantRequest,
) -> Result<Assistant, AppError> {
    let assistant_id = Uuid::new_v4();
    let is_default = request.is_default.unwrap_or(false);
    let is_template = request.is_template.unwrap_or(false);
    let enabled = request.enabled.unwrap_or(true);
    let parameters_json = request.parameters_to_json();

    // If this assistant is being set as default, unset all other defaults for the same context
    if is_default {
        if is_template {
            // For template assistants, unset all other default templates
            sqlx::query!("UPDATE assistants SET is_default = false WHERE is_template = true")
                .execute(&mut *conn)
                .await
                .map_err(AppError::database_error)?;
        } else if let Some(uid) = user_id {
            // For user assistants, unset all other default assistants for this user
            sqlx::query!(
                "UPDATE assistants SET is_default = false WHERE created_by = $1 AND is_template = false",
                uid
            )
            .execute(&mut *conn)
            .await
            .map_err(AppError::database_error)?;
        }
    }

    let row = sqlx::query!(
        r#"INSERT INTO assistants (id, name, description, instructions, parameters, created_by, is_template, is_default, enabled)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at"#,
        assistant_id,
        &request.name,
        request.description.as_deref(),
        request.instructions.as_deref(),
        parameters_json,
        user_id,
        is_template,
        is_default,
        enabled
    )
    .fetch_one(&mut *conn)
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

    Ok(assistant)
}

/// Transaction-scoped assistant delete. Like `delete_assistant` but runs on the
/// caller's connection. Returns whether a row was actually deleted (the hub
/// replace path tolerates an already-gone prior install).
pub async fn delete_assistant_tx(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query!("DELETE FROM assistants WHERE id = $1", id)
        .execute(&mut *conn)
        .await
        .map_err(AppError::database_error)?;
    Ok(result.rows_affected() > 0)
}

/// Get assistant by ID
/// Returns the assistant if it exists, regardless of its `enabled` state, so
/// management/admin paths can read (and re-enable) a disabled assistant. Chat
/// resolution must keep using `get_assistant_for_user`, which still filters
/// `enabled = true`.
/// Does not check ownership - permission check should be done in handler
pub async fn get_assistant(pool: &PgPool, id: Uuid) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
        FROM assistants
        WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(row.map(|r| {
        row_to_assistant(
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
        )
    }))
}

/// Like `get_assistant` but does NOT filter on `enabled`, so a disabled
/// (owner-toggled-off) assistant is still returned. Owner-facing GET/DELETE
/// management handlers use this — the `enabled = true` filter would
/// otherwise 404 a disabled assistant, leaving it impossible to view or
/// delete. Ownership + template checks still gate access in the handler.
/// Admin/template management paths also rely on this: the template list
/// intentionally surfaces disabled templates, so the per-id get/update/delete
/// handlers must be able to resolve them too (the `enabled = true` filter in
/// [`get_assistant`] is for chat resolution, which must never pick a disabled
/// assistant).
pub async fn get_assistant_any(pool: &PgPool, id: Uuid) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
        FROM assistants
        WHERE id = $1"#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(row.map(|r| {
        row_to_assistant(
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
        )
    }))
}

/// SSRF-equivalent for cross-tenant assistant lookup: returns Some only
/// when the row is the user's own assistant or a public template. Closes
/// 04-chat F-02 (High).
pub async fn get_assistant_for_user(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<Assistant>, AppError> {
    let row = sqlx::query!(
        r#"SELECT id, name, description, instructions, parameters, created_by, is_template, is_default, enabled, created_at, updated_at
        FROM assistants
        WHERE id = $1
          AND enabled = true
          AND (created_by = $2 OR is_template = true)"#,
        id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(row.map(|r| {
        row_to_assistant(
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
        )
    }))
}

/// List assistants with pagination and filtering
///
/// Parameters:
/// - user_id: User ID for ownership filtering (required for user assistants)
/// - is_template: Filter by template status
///   - true: Return only templates (user_id is ignored)
///   - false: Return only user's own assistants (requires user_id, never returns templates)
/// - page: Page number (1-indexed)
/// - limit: Items per page
pub async fn list_assistants(
    pool: &PgPool,
    user_id: Option<Uuid>,
    is_template: bool,
    page: i64,
    limit: i64,
) -> Result<AssistantListResponse, AppError> {
    let offset = (page - 1) * limit;

    // Build query based on is_template flag
    if is_template {
        // Only templates - use window function to get count and records in single query
        // Note: Template list shows ALL templates (enabled and disabled) for admin management
        let rows = sqlx::query!(
            r#"SELECT
                id, name, description, instructions, parameters, created_by,
                is_template, is_default, enabled, created_at, updated_at,
                COUNT(*) OVER() as "total_count!"
             FROM assistants
             WHERE is_template = true
             ORDER BY created_at DESC
             LIMIT $1 OFFSET $2"#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::database_error)?;

        let total = rows.first().map(|r| r.total_count).unwrap_or(0);

        let assistants = rows
            .into_iter()
            .map(|r| {
                row_to_assistant(
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
                )
            })
            .collect();

        Ok(AssistantListResponse { assistants, total })
    } else {
        // Only user's own assistants (never return templates)
        if let Some(uid) = user_id {
            // Use window function to get count and records in single query
            let rows = sqlx::query!(
                r#"SELECT
                    id, name, description, instructions, parameters, created_by,
                    is_template, is_default, enabled, created_at, updated_at,
                    COUNT(*) OVER() as "total_count!"
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

            let total = rows.first().map(|r| r.total_count).unwrap_or(0);

            let assistants = rows
                .into_iter()
                .map(|r| {
                    row_to_assistant(
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
                    )
                })
                .collect();

            Ok(AssistantListResponse { assistants, total })
        } else {
            // No user_id provided but filtering for user assistants - return empty
            Ok(AssistantListResponse {
                assistants: vec![],
                total: 0,
            })
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
        return get_assistant(pool, id)
            .await?
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
pub async fn get_default_assistant(
    pool: &PgPool,
    user_id: Option<Uuid>,
) -> Result<Option<Assistant>, AppError> {
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

        Ok(template_default_row.map(|r| {
            row_to_assistant(
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
            )
        }))
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

        Ok(template_default_row.map(|r| {
            row_to_assistant(
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
            )
        }))
    }
}

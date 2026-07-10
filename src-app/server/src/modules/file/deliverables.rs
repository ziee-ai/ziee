// Conversation deliverables: the files a conversation produced, surfaced as a
// curatable list. The base list is DERIVED (files the model authored in the
// conversation: file_versions.source_message_id in the conversation AND
// files.created_by IN ('mcp','llm')); the `conversation_deliverables` table lets
// the user PIN an extra file into the list or HIDE a derived one.
//
// Routes live under /conversations/{id}/deliverables (registered in the file
// router) and are ownership-scoped via the chat conversation lookup.

use std::collections::HashSet;

use aide::transform::TransformOperation;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::chat::core::permissions::{ConversationsEdit, ConversationsRead};
use crate::modules::file::models::File;
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::openapi::with_permission;
use crate::modules::sync::{publish as sync_publish, Audience, SyncAction, SyncEntity, SyncOrigin};

// ---------------------------------------------------------------------------
// repository
// ---------------------------------------------------------------------------

/// Resolve the deliverables of a conversation: (derived − hidden) ∪ pinned,
/// preserving derived order first, then pins not already present. Returns the
/// owned `File` rows.
pub async fn list_deliverable_files(
    conversation_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<File>, AppError> {
    // Derived: files the model authored somewhere in this conversation.
    let derived: Vec<Uuid> = sqlx::query_scalar!(
        r#"
        SELECT DISTINCT fv.file_id AS "file_id!"
        FROM file_versions fv
        JOIN branch_messages bm ON bm.message_id = fv.source_message_id
        JOIN branches br ON br.id = bm.branch_id
        JOIN files f ON f.id = fv.file_id
        WHERE br.conversation_id = $1
          AND f.user_id = $2
          AND f.created_by IN ('mcp', 'llm')
        "#,
        conversation_id,
        user_id
    )
    .fetch_all(Repos.pool())
    .await
    .map_err(AppError::database_error)?;

    // Curation rows: pinned=true promotes, pinned=false hides.
    let curated = sqlx::query!(
        r#"SELECT file_id, pinned FROM conversation_deliverables WHERE conversation_id = $1"#,
        conversation_id
    )
    .fetch_all(Repos.pool())
    .await
    .map_err(AppError::database_error)?;

    let mut hidden: HashSet<Uuid> = HashSet::new();
    let mut pinned: Vec<Uuid> = Vec::new();
    for row in curated {
        if row.pinned {
            pinned.push(row.file_id);
        } else {
            hidden.insert(row.file_id);
        }
    }

    let mut ids: Vec<Uuid> = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    for id in derived {
        if !hidden.contains(&id) && seen.insert(id) {
            ids.push(id);
        }
    }
    for id in pinned {
        if seen.insert(id) {
            ids.push(id);
        }
    }

    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // Ownership double-check + full File load in one batched query. The `id = ANY`
    // query does NOT preserve input order, so re-sort to the derived∪pinned order
    // we built above.
    let mut files = Repos.file.get_by_ids_and_user(&ids, user_id).await?;
    let pos: std::collections::HashMap<Uuid, usize> =
        ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    files.sort_by_key(|f| pos.get(&f.id).copied().unwrap_or(usize::MAX));
    Ok(files)
}

async fn upsert_pin(conversation_id: Uuid, file_id: Uuid, pinned: bool) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO conversation_deliverables (conversation_id, file_id, pinned)
        VALUES ($1, $2, $3)
        ON CONFLICT (conversation_id, file_id) DO UPDATE SET pinned = EXCLUDED.pinned
        "#,
        conversation_id,
        file_id,
        pinned
    )
    .execute(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

async fn delete_pin(conversation_id: Uuid, file_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM conversation_deliverables WHERE conversation_id = $1 AND file_id = $2"#,
        conversation_id,
        file_id
    )
    .execute(Repos.pool())
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Verify the conversation exists and is owned by the user (→ 404 otherwise).
async fn require_owned_conversation(conversation_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    Repos
        .chat
        .core
        .get_conversation(conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;
    Ok(())
}

fn publish_deliverable_changed(user_id: Uuid, conversation_id: Uuid, origin: Option<Uuid>) {
    sync_publish(
        SyncEntity::Deliverable,
        SyncAction::Update,
        conversation_id,
        Audience::owner(user_id),
        origin,
    );
}

// ---------------------------------------------------------------------------
// handlers
// ---------------------------------------------------------------------------

/// Body for `POST /conversations/{id}/deliverables/{file_id}`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PinDeliverableRequest {
    /// `true` promotes the file into the deliverables list; `false` hides a
    /// derived file. Defaults to `true`.
    #[serde(default = "default_true")]
    pub pinned: bool,
}

fn default_true() -> bool {
    true
}

/// List a conversation's deliverables (derived ∪ pinned − hidden).
pub async fn list_deliverables(
    auth: RequirePermissions<(ConversationsRead,)>,
    Path(conversation_id): Path<Uuid>,
) -> ApiResult<Json<Vec<File>>> {
    let user_id = auth.user.id;
    require_owned_conversation(conversation_id, user_id).await?;
    let files = list_deliverable_files(conversation_id, user_id).await?;
    Ok((StatusCode::OK, Json(files)))
}

pub fn list_deliverables_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsRead,)>(op)
        .id("File.listDeliverables")
        .tag("Files")
        .summary("List conversation deliverables")
        .description(
            "The files a conversation produced: model-authored files (derived) plus \
             user-pinned files, minus user-hidden ones.",
        )
}

/// Pin (or hide) a file as a deliverable of the conversation.
pub async fn pin_deliverable(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path((conversation_id, file_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
    body: Option<Json<PinDeliverableRequest>>,
) -> ApiResult<StatusCode> {
    let user_id = auth.user.id;
    require_owned_conversation(conversation_id, user_id).await?;
    // The file must be owned by the caller.
    Repos
        .file
        .get_by_id_and_user(file_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    let pinned = body.map(|b| b.0.pinned).unwrap_or(true);
    upsert_pin(conversation_id, file_id, pinned).await?;
    publish_deliverable_changed(user_id, conversation_id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn pin_deliverable_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("File.pinDeliverable")
        .tag("Files")
        .summary("Pin/hide a conversation deliverable")
        .description("Promote a file into (pinned=true) or hide it from (pinned=false) the deliverables list.")
}

/// Remove a file's deliverable curation (revert to the derived default).
pub async fn unpin_deliverable(
    auth: RequirePermissions<(ConversationsEdit,)>,
    Path((conversation_id, file_id)): Path<(Uuid, Uuid)>,
    origin: SyncOrigin,
) -> ApiResult<StatusCode> {
    let user_id = auth.user.id;
    require_owned_conversation(conversation_id, user_id).await?;
    delete_pin(conversation_id, file_id).await?;
    publish_deliverable_changed(user_id, conversation_id, origin.0);
    Ok((StatusCode::NO_CONTENT, StatusCode::NO_CONTENT))
}

pub fn unpin_deliverable_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(ConversationsEdit,)>(op)
        .id("File.unpinDeliverable")
        .tag("Files")
        .summary("Remove a conversation deliverable curation")
        .description("Delete the pin/hide row for a file, reverting it to the derived default.")
}

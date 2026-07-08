// Messages repository - Refactored for junction table architecture

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Message;
use crate::modules::chat::core::types::{
    EditMessageRequest, EditMessageResponse, MessageSearchMatch, MessageSearchResults,
    MessageWindowMode, MessageWithContent, PaginatedMessages, build_snippet,
};

use super::contents::{get_message_contents, get_message_contents_batch};

/// Create a new message and add it to a branch
/// Note: This creates the message AND the branch_messages junction record
pub async fn create_message(
    pool: &PgPool,
    branch_id: Uuid,
    role: &str,
    model_id: Option<Uuid>,
) -> Result<Message, AppError> {
    let message_id = Uuid::new_v4();

    // Start transaction
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Create message (originated_from_id = self for new messages).
    // Per-extension state (mcp's server-list snapshot, assistant
    // attribution, etc.) is written by extensions themselves via the
    // `after_user_message_created` hook on the ChatExtension trait,
    // AFTER this commit returns. Only `model_id` remains as an
    // opaque storage column on chat's row.
    let message = sqlx::query_as!(
        Message,
        r#"
        INSERT INTO messages (id, role, originated_from_id, edit_count, model_id)
        VALUES ($1, $2, $1, 0, $3)
        RETURNING id, role,
                  originated_from_id as "originated_from_id!",
                  edit_count,
                  model_id as "model_id: _",
                  created_at as "created_at: _"
        "#,
        message_id,
        role,
        model_id as _,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Add message to branch (not a clone)
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone)
        VALUES ($1, $2, false)
        "#,
        branch_id,
        message_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(message)
}

/// Get message by ID
pub async fn get_message(pool: &PgPool, id: Uuid) -> Result<Option<Message>, AppError> {
    let message = sqlx::query_as!(
        Message,
        r#"
        SELECT id, role,
               originated_from_id as "originated_from_id!",
               edit_count,
               model_id as "model_id: _",
               created_at as "created_at: _"
        FROM messages
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(message)
}

/// Get message with all its content blocks
pub async fn get_message_with_content(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<MessageWithContent>, AppError> {
    let message = get_message(pool, id).await?;

    match message {
        Some(msg) => {
            let contents = get_message_contents(pool, msg.id).await?;
            Ok(Some(MessageWithContent {
                message: msg,
                contents,
            }))
        }
        None => Ok(None),
    }
}

/// List all messages in a branch (ordered by when they were added to branch)
/// This joins through the branch_messages junction table
pub async fn list_messages_in_branch(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<Message>, AppError> {
    let messages = sqlx::query_as!(
        Message,
        r#"
        SELECT m.id, m.role,
               m.originated_from_id as "originated_from_id!",
               m.edit_count,
               m.model_id as "model_id: _",
               m.created_at as "created_at: _"
        FROM messages m
        INNER JOIN branch_messages bm ON m.id = bm.message_id
        WHERE bm.branch_id = $1
        ORDER BY bm.created_at ASC
        "#,
        branch_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(messages)
}

/// Get conversation history (messages with content) for AI context
/// This is used for building the context for AI API calls
/// Optimized: uses batch query to fetch all content blocks in 1 query instead of N
pub async fn get_conversation_history(
    pool: &PgPool,
    branch_id: Uuid,
) -> Result<Vec<MessageWithContent>, AppError> {
    let messages = list_messages_in_branch(pool, branch_id).await?;

    // Collect message IDs for batch query
    let message_ids: Vec<Uuid> = messages.iter().map(|m| m.id).collect();

    // Fetch all contents in one query (instead of N queries)
    let mut contents_map = get_message_contents_batch(pool, &message_ids).await?;

    // Build history with contents
    let history = messages
        .into_iter()
        .map(|message| {
            let contents = contents_map.remove(&message.id).unwrap_or_default();
            MessageWithContent { message, contents }
        })
        .collect();

    Ok(history)
}

// =====================================================
// Keyset windowed history (lazy-load) + in-conversation search
//
// NOTE: `get_conversation_history` above (full branch load) is intentionally
// left untouched — it is the AI-context path (summarization / memory / mcp /
// title / streaming all need the COMPLETE branch history). The functions below
// serve ONLY the HTTP/UI display path, which paginates.
// =====================================================

/// Rows for one message-history window, chronological ascending.
struct WindowRows {
    messages: Vec<Message>,
    has_more_before: bool,
    has_more_after: bool,
}

/// Resolve a cursor message's junction `created_at` within a branch. `None`
/// means the message is not in this branch (handler maps to 404).
async fn resolve_cursor_created_at(
    pool: &PgPool,
    branch_id: Uuid,
    message_id: Uuid,
) -> Result<Option<time::OffsetDateTime>, AppError> {
    let ca = sqlx::query_scalar!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        branch_id,
        message_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(ca)
}

/// Newest `limit` messages of a branch (DESC; caller reverses to ASC).
async fn fetch_tail_desc(
    pool: &PgPool,
    branch_id: Uuid,
    limit: i64,
) -> Result<Vec<Message>, AppError> {
    sqlx::query_as!(
        Message,
        r#"
        SELECT m.id, m.role,
               m.originated_from_id as "originated_from_id!",
               m.edit_count,
               m.model_id as "model_id: _",
               m.created_at as "created_at: _"
        FROM messages m
        INNER JOIN branch_messages bm ON m.id = bm.message_id
        WHERE bm.branch_id = $1
        ORDER BY bm.created_at DESC, bm.message_id DESC
        LIMIT $2
        "#,
        branch_id,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// Up to `limit` messages strictly OLDER than the `(cursor_created_at,
/// cursor_id)` position (DESC; caller reverses to ASC).
async fn fetch_before_desc(
    pool: &PgPool,
    branch_id: Uuid,
    cursor_created_at: time::OffsetDateTime,
    cursor_id: Uuid,
    limit: i64,
) -> Result<Vec<Message>, AppError> {
    sqlx::query_as!(
        Message,
        r#"
        SELECT m.id, m.role,
               m.originated_from_id as "originated_from_id!",
               m.edit_count,
               m.model_id as "model_id: _",
               m.created_at as "created_at: _"
        FROM messages m
        INNER JOIN branch_messages bm ON m.id = bm.message_id
        WHERE bm.branch_id = $1
          AND (bm.created_at < $2 OR (bm.created_at = $2 AND bm.message_id < $3))
        ORDER BY bm.created_at DESC, bm.message_id DESC
        LIMIT $4
        "#,
        branch_id,
        cursor_created_at,
        cursor_id,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// Up to `limit` messages strictly NEWER than the `(cursor_created_at,
/// cursor_id)` position (already ASC).
async fn fetch_after_asc(
    pool: &PgPool,
    branch_id: Uuid,
    cursor_created_at: time::OffsetDateTime,
    cursor_id: Uuid,
    limit: i64,
) -> Result<Vec<Message>, AppError> {
    sqlx::query_as!(
        Message,
        r#"
        SELECT m.id, m.role,
               m.originated_from_id as "originated_from_id!",
               m.edit_count,
               m.model_id as "model_id: _",
               m.created_at as "created_at: _"
        FROM messages m
        INNER JOIN branch_messages bm ON m.id = bm.message_id
        WHERE bm.branch_id = $1
          AND (bm.created_at > $2 OR (bm.created_at = $2 AND bm.message_id > $3))
        ORDER BY bm.created_at ASC, bm.message_id ASC
        LIMIT $4
        "#,
        branch_id,
        cursor_created_at,
        cursor_id,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)
}

/// Keyset window over the active branch path. `Ok(None)` when a cursor id is not
/// in the branch. Uses the fetch-`limit+1` idiom to compute `has_more_*`.
async fn list_message_window(
    pool: &PgPool,
    branch_id: Uuid,
    mode: MessageWindowMode,
    limit: i64,
) -> Result<Option<WindowRows>, AppError> {
    match mode {
        MessageWindowMode::Tail => {
            let mut rows = fetch_tail_desc(pool, branch_id, limit + 1).await?;
            let has_more_before = rows.len() as i64 > limit;
            rows.truncate(limit as usize);
            rows.reverse(); // → ASC
            Ok(Some(WindowRows {
                messages: rows,
                has_more_before,
                has_more_after: false,
            }))
        }
        MessageWindowMode::Before(id) => {
            let Some(ca) = resolve_cursor_created_at(pool, branch_id, id).await? else {
                return Ok(None);
            };
            let mut rows = fetch_before_desc(pool, branch_id, ca, id, limit + 1).await?;
            let has_more_before = rows.len() as i64 > limit;
            rows.truncate(limit as usize);
            rows.reverse(); // → ASC
            Ok(Some(WindowRows {
                messages: rows,
                has_more_before,
                // The cursor and everything newer than it exist by definition.
                has_more_after: true,
            }))
        }
        MessageWindowMode::After(id) => {
            let Some(ca) = resolve_cursor_created_at(pool, branch_id, id).await? else {
                return Ok(None);
            };
            let mut rows = fetch_after_asc(pool, branch_id, ca, id, limit + 1).await?;
            let has_more_after = rows.len() as i64 > limit;
            rows.truncate(limit as usize);
            // already ASC
            Ok(Some(WindowRows {
                messages: rows,
                // The cursor and everything older than it exist by definition.
                has_more_before: true,
                has_more_after,
            }))
        }
        MessageWindowMode::Around(id) => {
            let Some(ca) = resolve_cursor_created_at(pool, branch_id, id).await? else {
                return Ok(None);
            };
            let half = (limit / 2).max(1);

            let mut older = fetch_before_desc(pool, branch_id, ca, id, half + 1).await?;
            let has_more_before = older.len() as i64 > half;
            older.truncate(half as usize);
            older.reverse(); // → ASC

            let mut newer = fetch_after_asc(pool, branch_id, ca, id, half + 1).await?;
            let has_more_after = newer.len() as i64 > half;
            newer.truncate(half as usize);

            // The cursor message itself (guaranteed in-branch by the resolve above).
            let mut messages = older;
            if let Some(cursor_msg) = get_message(pool, id).await? {
                messages.push(cursor_msg);
            }
            messages.extend(newer);

            Ok(Some(WindowRows {
                messages,
                has_more_before,
                has_more_after,
            }))
        }
    }
}

/// Public: a page of the active branch's messages WITH content blocks (batched),
/// or `None` when a supplied cursor id is not in the branch (→ 404).
pub async fn get_message_window(
    pool: &PgPool,
    branch_id: Uuid,
    mode: MessageWindowMode,
    limit: i64,
) -> Result<Option<PaginatedMessages>, AppError> {
    let Some(window) = list_message_window(pool, branch_id, mode, limit).await? else {
        return Ok(None);
    };

    let message_ids: Vec<Uuid> = window.messages.iter().map(|m| m.id).collect();
    let mut contents_map = get_message_contents_batch(pool, &message_ids).await?;

    let messages = window
        .messages
        .into_iter()
        .map(|message| {
            let contents = contents_map.remove(&message.id).unwrap_or_default();
            MessageWithContent { message, contents }
        })
        .collect();

    Ok(Some(PaginatedMessages {
        messages,
        has_more_before: window.has_more_before,
        has_more_after: window.has_more_after,
    }))
}

/// In-conversation message search over the ACTIVE branch, paginated. Mirrors the
/// active-branch text-match join in `conversations::list_conversations`, scoped
/// to one branch. `term` must be pre-trimmed and non-empty (caller returns an
/// empty result for a blank query without calling this).
///
/// NOTE: matches `list_conversations`' un-escaped `ILIKE` semantics for
/// consistency — a term containing `%`/`_` acts as a wildcard.
pub async fn search_messages_in_conversation(
    pool: &PgPool,
    branch_id: Uuid,
    term: &str,
    page: i64,
    per_page: i64,
) -> Result<MessageSearchResults, AppError> {
    let offset = (page - 1).max(0) * per_page;

    // Escape LIKE metacharacters so a term containing `%` / `_` matches
    // literally (and `q = "%"` doesn't degrade into a match-everything scan).
    // Backslash first so we don't double-escape the escapes we add. Paired with
    // `ESCAPE '\'` on each ILIKE below. `term` (unescaped) is still used for the
    // snippet window.
    let like_term = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");

    let total: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM messages m
        INNER JOIN branch_messages bm ON bm.message_id = m.id
        WHERE bm.branch_id = $1
          AND EXISTS (
            SELECT 1 FROM message_contents mc
            WHERE mc.message_id = m.id
              AND mc.content_type = 'text'
              AND mc.content->>'text' ILIKE '%' || $2 || '%' ESCAPE '\'
          )
        "#,
        branch_id,
        like_term
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    let rows = sqlx::query!(
        r#"
        SELECT m.id as "id!", m.role as "role!",
               m.created_at as "created_at!: chrono::DateTime<chrono::Utc>",
               (
                 SELECT mc2.content->>'text'
                 FROM message_contents mc2
                 WHERE mc2.message_id = m.id
                   AND mc2.content_type = 'text'
                   AND mc2.content->>'text' ILIKE '%' || $2 || '%' ESCAPE '\'
                 ORDER BY mc2.sequence_order ASC
                 LIMIT 1
               ) as snippet_text
        FROM messages m
        INNER JOIN branch_messages bm ON bm.message_id = m.id
        WHERE bm.branch_id = $1
          AND EXISTS (
            SELECT 1 FROM message_contents mc
            WHERE mc.message_id = m.id
              AND mc.content_type = 'text'
              AND mc.content->>'text' ILIKE '%' || $2 || '%' ESCAPE '\'
          )
        ORDER BY bm.created_at ASC, m.id ASC
        LIMIT $3 OFFSET $4
        "#,
        branch_id,
        like_term,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let matches = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let raw = row.snippet_text.unwrap_or_default();
            MessageSearchMatch {
                message_id: row.id,
                role: row.role,
                created_at: row.created_at,
                snippet: build_snippet(&raw, term),
                ordinal: offset + i as i64 + 1,
            }
        })
        .collect();

    Ok(MessageSearchResults {
        matches,
        total,
        page,
        per_page,
    })
}

/// Create a new branch from a message (for unified streaming endpoint)
/// Clones all messages from parent branch up to (but not including) the specified message
pub async fn create_branch_from_message(
    pool: &PgPool,
    conversation_id: Uuid,
    parent_branch_id: Uuid,
    message_id: Uuid,
    fork_level: &str,
) -> Result<crate::modules::chat::core::models::Branch, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // Verify message exists in the parent branch
    let message_created_at = sqlx::query_scalar!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        parent_branch_id,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message not in branch"))?;

    // Create new branch
    let new_branch = sqlx::query_as!(
        crate::modules::chat::core::models::Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id, fork_level)
        VALUES ($1, $2, $3, $4)
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  fork_level, created_at as "created_at: _"
        "#,
        conversation_id,
        Some(parent_branch_id),
        Some(message_id),
        fork_level,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Clone messages from parent branch up to (but not including) the branching message
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
        SELECT $1, message_id, true, created_at
        FROM branch_messages
        WHERE branch_id = $2 AND created_at < $3
        "#,
        new_branch.id,
        parent_branch_id,
        message_created_at
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // Set new branch as active
    sqlx::query!(
        r#"
        UPDATE conversations SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        new_branch.id,
        conversation_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(new_branch)
}

/// Edit a message (creates new branch with updated message)
/// This is the key operation for edit/regenerate functionality
pub async fn edit_message(
    pool: &PgPool,
    message_id: Uuid,
    conversation_id: Uuid,
    request: EditMessageRequest,
    current_branch_id: Uuid,
) -> Result<EditMessageResponse, AppError> {
    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    // 1. Get original message
    let original = sqlx::query_as!(
        Message,
        r#"
        SELECT id, role,
               originated_from_id as "originated_from_id!",
               edit_count,
               model_id as "model_id: _",
               created_at as "created_at: _"
        FROM messages
        WHERE id = $1
        "#,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message"))?;

    // Get the created_at from branch_messages for cloning cutoff
    let original_created_at = sqlx::query_scalar!(
        r#"
        SELECT created_at
        FROM branch_messages
        WHERE branch_id = $1 AND message_id = $2
        "#,
        current_branch_id,
        message_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?
    .ok_or_else(|| AppError::not_found("Message not in branch"))?;

    // 2. Create new branch (edit_message is always a 'user' level fork)
    let new_branch = sqlx::query_as!(
        crate::modules::chat::core::models::Branch,
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id, fork_level)
        VALUES ($1, $2, $3, 'user')
        RETURNING id, conversation_id, parent_branch_id, created_from_message_id,
                  fork_level, created_at as "created_at: _"
        "#,
        conversation_id,
        Some(current_branch_id),
        Some(message_id)
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 3. Clone messages from current branch up to (but not including) edited message
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone, created_at)
        SELECT $1, message_id, true, created_at
        FROM branch_messages
        WHERE branch_id = $2 AND created_at < $3
        "#,
        new_branch.id,
        current_branch_id,
        original_created_at
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 4. Create the edited message (model/assistant/mcp context not set here — set via streaming)
    let new_message_id = Uuid::new_v4();
    let new_message = sqlx::query_as!(
        Message,
        r#"
        INSERT INTO messages (id, role, originated_from_id, edit_count)
        VALUES ($1, $2, $3, $4)
        RETURNING id, role,
                  originated_from_id as "originated_from_id!",
                  edit_count,
                  model_id as "model_id: _",
                  created_at as "created_at: _"
        "#,
        new_message_id,
        original.role,
        original.originated_from_id, // Keep same origin
        original.edit_count          // Will be incremented later
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 5. Add message content
    let content_data = serde_json::json!({"text": request.content});
    sqlx::query!(
        r#"
        INSERT INTO message_contents (message_id, content_type, content, sequence_order)
        VALUES ($1, 'text', $2, 0)
        "#,
        new_message_id,
        content_data
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 6. Add new message to branch (not a clone)
    sqlx::query!(
        r#"
        INSERT INTO branch_messages (branch_id, message_id, is_clone)
        VALUES ($1, $2, false)
        "#,
        new_branch.id,
        new_message_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 7. Set new branch as active
    sqlx::query!(
        r#"
        UPDATE conversations SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        "#,
        new_branch.id,
        conversation_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    // 8. Increment edit_count for all messages with same originated_from_id
    sqlx::query!(
        r#"
        UPDATE messages SET edit_count = edit_count + 1
        WHERE originated_from_id = $1
        "#,
        original.originated_from_id
    )
    .execute(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;

    Ok(EditMessageResponse {
        message: new_message,
        branch: new_branch,
    })
}

/// Verify that a message exists and user owns the conversation containing it
/// Returns the conversation if ownership is verified, None otherwise
///
/// This joins through: messages → branch_messages → branches → conversations
/// to verify ownership since messages don't have a direct conversation_id FK
pub async fn verify_message_ownership(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<Option<crate::modules::chat::core::models::Conversation>, AppError> {
    let result = sqlx::query_as!(
        crate::modules::chat::core::models::Conversation,
        r#"
        SELECT DISTINCT c.id, c.user_id, c.model_id as "model_id: _", c.title, c.active_branch_id,
               c.created_at as "created_at: _", c.updated_at as "updated_at: _"
        FROM conversations c
        INNER JOIN branches b ON b.conversation_id = c.id
        INNER JOIN branch_messages bm ON bm.branch_id = b.id
        WHERE bm.message_id = $1 AND c.user_id = $2
        LIMIT 1
        "#,
        message_id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result)
}

/// Delete a single message. The schema-level `ON DELETE CASCADE` on
/// `branch_messages.message_id` removes the junction rows in every
/// branch that referenced the message.
///
/// Note on the previous name (`delete_message_and_descendants`): the
/// chat model is CoW-branch-based, NOT a hierarchical tree — messages
/// have no parent_id column. "Descendants" in a branched chat is
/// ambiguous: per-branch chronological successors? Across all
/// branches that cloned this message? The original implementation
/// silently did neither (just one DELETE on the message row), and the
/// audit's 04-chat F-03 (High) flagged the contract mismatch. Renaming
/// to `delete_message` makes the contract honest; a richer
/// "trim from here onward" operation can be designed separately.
pub async fn delete_message(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM messages
        WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected())
}

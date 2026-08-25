// Conversations repository

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::Conversation;
use crate::modules::chat::core::types::ConversationResponse;

/// Convert time::OffsetDateTime to chrono::DateTime<Utc>
fn to_chrono_datetime(odt: OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond()).expect("valid timestamp")
}

/// Insert a conversation + default branch + active_branch_id update in
/// the caller's transaction.
pub async fn create_conversation_in_tx<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::Postgres>,
    user_id: Uuid,
    model_id: Option<Uuid>,
    title: Option<String>,
) -> Result<Conversation, AppError> {
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        INSERT INTO conversations (user_id, model_id, title)
        VALUES ($1, $2, $3)
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        user_id,
        model_id as Option<Uuid>,
        title,
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::database_error)?;

    let branch = sqlx::query!(
        r#"
        INSERT INTO branches (conversation_id, parent_branch_id, created_from_message_id)
        VALUES ($1, NULL, NULL)
        RETURNING id
        "#,
        conversation.id
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::database_error)?;

    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        UPDATE conversations
        SET active_branch_id = $1, updated_at = NOW()
        WHERE id = $2
        RETURNING id, user_id, model_id as "model_id: _", title, active_branch_id,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        branch.id,
        conversation.id
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Create a new conversation with a default branch (pool-based wrapper).
pub async fn create_conversation(
    pool: &PgPool,
    user_id: Uuid,
    model_id: Option<Uuid>,
    title: Option<String>,
) -> Result<Conversation, AppError> {
    if let Some(mid) = model_id {
        let model_exists = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM llm_models WHERE id = $1)",
            mid
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::database_error)?
        .unwrap_or(false);

        if !model_exists {
            return Err(AppError::not_found("Model"));
        }
    }

    let mut tx = pool.begin().await.map_err(AppError::database_error)?;
    let conversation = create_conversation_in_tx(&mut tx, user_id, model_id, title).await?;
    tx.commit().await.map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Get conversation by ID (with user ownership check)
pub async fn get_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<Conversation>, AppError> {
    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(conversation)
}

/// Normalize an incoming `sort` query value to a whitelisted canonical key.
///
/// Only these four keys ever reach the query's ORDER BY CASE; any unknown /
/// missing value collapses to `recent` (the historical `updated_at DESC`
/// default). The value is passed as a BOUND parameter, never interpolated, so
/// this is defense-in-depth against injection AND the single source of the sort
/// vocabulary.
pub fn normalize_sort(sort: Option<&str>) -> &'static str {
    match sort {
        Some("oldest") => "oldest",
        Some("alpha") => "alpha",
        Some("most_messages") => "most_messages",
        // "recent" + anything unknown/None → the default.
        _ => "recent",
    }
}

/// Server-side truncation cap for [`ConversationResponse::first_message_preview`].
///
/// A fixed constant rather than an admin setting: this is a display-string
/// length, not an operational tunable — it has no resource, retention, quota, or
/// security dimension. Mirrors `TITLE_MAX_CHARS` in the title extension. Named
/// (not inlined) so it can be promoted to configurable without a rewrite.
/// 120 chars fills the widest sidebar row while keeping the list payload small.
pub const CONVERSATION_PREVIEW_MAX_CHARS: i32 = 120;

/// List the user's conversations with optional content search + sort.
///
/// - `search`: when `Some`, filters to conversations whose title OR any
///   `text` message-content block contains the term (case-insensitive
///   substring, `ILIKE`). `None`/empty → no filter.
/// - `sort`: a whitelisted key (see [`normalize_sort`]) driving a bound-param
///   CASE `ORDER BY`, keeping the whole query compile-time verified.
pub async fn list_conversations(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
    search: Option<&str>,
    sort: Option<&str>,
) -> Result<Vec<ConversationResponse>, AppError> {
    let sort_key = normalize_sort(sort);
    let rows = sqlx::query!(
        r#"
        SELECT
            c.id, c.user_id, c.model_id, c.title, c.active_branch_id,
            c.created_at, c.updated_at,
            COUNT(bm.message_id) as message_count,
            LEFT(fm.text, $6) as first_message_preview
        FROM conversations c
        LEFT JOIN branches b ON b.conversation_id = c.id
        LEFT JOIN branch_messages bm ON bm.branch_id = b.id
        -- First user text on the ACTIVE branch, for the client's display label
        -- when `title` is NULL. A LATERAL (not a correlated scalar subquery)
        -- so it contributes exactly one row per conversation and therefore
        -- cannot disturb the COUNT(bm.message_id) aggregate or the GROUP BY.
        -- Active-branch-only for the same reason the search filter below is
        -- (superseded edit-branch content is invisible when opened).
        LEFT JOIN LATERAL (
            SELECT mc.content->>'text' AS text
            FROM branch_messages bm3
            JOIN messages m ON m.id = bm3.message_id
            JOIN message_contents mc ON mc.message_id = m.id
            WHERE bm3.branch_id = c.active_branch_id
              AND m.role = 'user'
              AND mc.content_type = 'text'
              AND NULLIF(TRIM(mc.content->>'text'), '') IS NOT NULL
            ORDER BY bm3.created_at ASC, m.id ASC, mc.sequence_order ASC
            LIMIT 1
        ) fm ON TRUE
        WHERE c.user_id = $1
          AND (
            $4::text IS NULL
            OR c.title ILIKE '%' || $4 || '%'
            OR EXISTS (
              SELECT 1
              FROM message_contents mc
              JOIN branch_messages bm2 ON bm2.message_id = mc.message_id
              -- Only the conversation's ACTIVE branch: content in a superseded
              -- edit branch is invisible when the conversation is opened, and
              -- the client find bar searches the active branch only.
              WHERE bm2.branch_id = c.active_branch_id
                AND mc.content_type = 'text'
                AND mc.content->>'text' ILIKE '%' || $4 || '%'
            )
          )
        -- fm.text joins one row per conversation, so grouping by it cannot
        -- change cardinality; Postgres needs it listed because functional
        -- dependency on c.id is only inferred for columns of `c` itself.
        GROUP BY c.id, fm.text
        ORDER BY
          CASE WHEN $5 = 'oldest' THEN c.updated_at END ASC NULLS LAST,
          CASE WHEN $5 = 'alpha' THEN c.title END ASC NULLS LAST,
          CASE WHEN $5 = 'most_messages' THEN COUNT(bm.message_id) END DESC NULLS LAST,
          c.updated_at DESC
        LIMIT $2 OFFSET $3
        "#,
        user_id,
        limit,
        offset,
        search,
        sort_key,
        CONVERSATION_PREVIEW_MAX_CHARS,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(rows
        .into_iter()
        .map(|row| ConversationResponse {
            conversation: Conversation {
                id: row.id,
                user_id: row.user_id,
                model_id: row.model_id,
                title: row.title,
                active_branch_id: row.active_branch_id,
                created_at: to_chrono_datetime(row.created_at),
                updated_at: to_chrono_datetime(row.updated_at),
            },
            message_count: row.message_count.unwrap_or(0),
            first_message_preview: row.first_message_preview,
        })
        .collect())
}

/// Count the user's conversations (for paginated list responses), honoring the
/// same optional content `search` filter as [`list_conversations`] so the
/// paginated `total` reflects the filtered result set.
pub async fn count_conversations(
    pool: &PgPool,
    user_id: Uuid,
    search: Option<&str>,
) -> Result<i64, AppError> {
    let total = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*)
        FROM conversations c
        WHERE c.user_id = $1
          AND (
            $2::text IS NULL
            OR c.title ILIKE '%' || $2 || '%'
            OR EXISTS (
              SELECT 1
              FROM message_contents mc
              JOIN branch_messages bm2 ON bm2.message_id = mc.message_id
              -- Active branch only (see list_conversations).
              WHERE bm2.branch_id = c.active_branch_id
                AND mc.content_type = 'text'
                AND mc.content->>'text' ILIKE '%' || $2 || '%'
            )
          )
        "#,
        user_id,
        search,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?
    .unwrap_or(0);

    Ok(total)
}

/// Update conversation metadata (title only).
pub async fn update_conversation(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    title: Option<Option<String>>,
) -> Result<Option<Conversation>, AppError> {
    let _existing = sqlx::query!(
        "SELECT id FROM conversations WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;

    if _existing.is_none() {
        return Ok(None);
    }

    let mut tx = pool.begin().await.map_err(AppError::database_error)?;

    if let Some(new_title) = title {
        sqlx::query!(
            "UPDATE conversations SET title = $1, updated_at = NOW() WHERE id = $2 AND user_id = $3",
            new_title as Option<String>,
            id,
            user_id
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::database_error)?;
    }

    let conversation = sqlx::query_as!(
        Conversation,
        r#"
        SELECT id, user_id, model_id as "model_id: _", title, active_branch_id,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(AppError::database_error)?;

    tx.commit().await.map_err(AppError::database_error)?;
    Ok(conversation)
}

/// Delete conversation (cascades to branches and messages).
pub async fn delete_conversation(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM conversations
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(result.rows_affected() > 0)
}

/// Update conversation model and optionally active branch
pub async fn update_conversation_state(
    pool: &PgPool,
    conversation_id: Uuid,
    user_id: Uuid,
    model_id: Uuid,
    branch_id: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(branch_id) = branch_id {
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, active_branch_id = $2, updated_at = NOW()
            WHERE id = $3 AND user_id = $4
            "#,
            model_id,
            branch_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    } else {
        sqlx::query!(
            r#"
            UPDATE conversations
            SET model_id = $1, updated_at = NOW()
            WHERE id = $2 AND user_id = $3
            "#,
            model_id,
            conversation_id,
            user_id
        )
        .execute(pool)
        .await
        .map_err(AppError::database_error)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_sort;

    #[test]
    fn normalize_sort_whitelists_known_keys() {
        assert_eq!(normalize_sort(Some("recent")), "recent");
        assert_eq!(normalize_sort(Some("oldest")), "oldest");
        assert_eq!(normalize_sort(Some("alpha")), "alpha");
        assert_eq!(normalize_sort(Some("most_messages")), "most_messages");
    }

    #[test]
    fn normalize_sort_defaults_unknown_and_none_to_recent() {
        assert_eq!(normalize_sort(None), "recent");
        assert_eq!(normalize_sort(Some("")), "recent");
        assert_eq!(normalize_sort(Some("bogus")), "recent");
        // Defense-in-depth: an injection attempt is not a known key → default.
        assert_eq!(normalize_sort(Some("updated_at; DROP TABLE conversations")), "recent");
    }
}

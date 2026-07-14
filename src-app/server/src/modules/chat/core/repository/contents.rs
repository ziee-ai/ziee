// Contents repository - Handles message content blocks with delta accumulation

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;
use crate::modules::chat::core::models::{MessageContent, MessageContentData};

/// Create a new content block
pub async fn create_content(
    pool: &PgPool,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
    sequence_order: i32,
) -> Result<MessageContent, AppError> {
    let content_json =
        serde_json::to_value(&initial_data).map_err(AppError::database_error)?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        INSERT INTO message_contents (message_id, content_type, content, sequence_order)
        VALUES ($1, $2, $3, $4)
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        message_id,
        content_type,
        content_json,
        sequence_order
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Append a content block, computing the next `sequence_order` as
/// `MAX(sequence_order) + 1` for the message *inside the INSERT itself*.
///
/// Eliminates the cache↔DB drift that used to let a later tool_use collide
/// with an earlier tool_result on the parallel-tool path: each caller reads
/// the current MAX at write time instead of from a stale in-memory snapshot.
///
/// **Assumes appends to a single `message_id` are sequential** — the streaming
/// agentic loop awaits each append in one task, which is the only production
/// caller. The subquery runs at READ COMMITTED isolation, so two truly-
/// concurrent transactions appending to the SAME message could still race for
/// the same slot.
///
/// That race is now CAUGHT, not silent: `UNIQUE (message_id, sequence_order)`
/// exists (constraint `uq_message_contents_message_sequence`, migration 124), so
/// the losing INSERT fails with a hard DB error instead of duplicating a slot.
///
/// A concurrent caller DOES exist, so this is not hypothetical: the MCP extension's
/// detached elicitation task calls `append_content_with_id` (same MAX+1-inside-INSERT
/// slot computation, same `message_id`) while the approval loop's own
/// `append_content` calls run. Neither retries, and the approval-loop site swallows
/// the error with `let _ =`, so a lost race there is a DROPPED content row rather
/// than a loud failure. That is a real (pre-existing, narrow — it needs an
/// elicitation notification to land in the same instant as a result append) gap and
/// NOT something this comment should paper over: fixing it means a retry-on-unique-
/// violation loop here, which is deliberately out of scope for the change that
/// corrected this comment.
pub async fn append_content(
    pool: &PgPool,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
) -> Result<MessageContent, AppError> {
    let content_json =
        serde_json::to_value(&initial_data).map_err(AppError::database_error)?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        INSERT INTO message_contents (message_id, content_type, content, sequence_order)
        VALUES (
            $1, $2, $3,
            (SELECT COALESCE(MAX(sequence_order), -1) + 1
               FROM message_contents WHERE message_id = $1)
        )
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        message_id,
        content_type,
        content_json
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Append a content block with a pre-registered UUID, computing the next
/// `sequence_order` as `MAX+1` inside the INSERT. Id-preserving sibling of
/// `append_content` for elicitation rows whose content id is registered before
/// insertion. Same sequential-callers assumption — see `append_content`.
pub async fn append_content_with_id(
    pool: &PgPool,
    id: Uuid,
    message_id: Uuid,
    content_type: &str,
    initial_data: MessageContentData,
) -> Result<MessageContent, AppError> {
    let content_json =
        serde_json::to_value(&initial_data).map_err(AppError::database_error)?;

    let content = sqlx::query_as!(
        MessageContent,
        r#"
        INSERT INTO message_contents (id, message_id, content_type, content, sequence_order)
        VALUES (
            $1, $2, $3, $4,
            (SELECT COALESCE(MAX(sequence_order), -1) + 1
               FROM message_contents WHERE message_id = $2)
        )
        RETURNING id, message_id, content_type, content, sequence_order,
                  created_at as "created_at: _", updated_at as "updated_at: _"
        "#,
        id,
        message_id,
        content_type,
        content_json
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(content)
}

/// Get all content blocks for a message (ordered by sequence)
pub async fn get_message_contents(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<Vec<MessageContent>, AppError> {
    let contents = sqlx::query_as!(
        MessageContent,
        r#"
        SELECT id, message_id, content_type, content, sequence_order,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM message_contents
        WHERE message_id = $1
        ORDER BY sequence_order ASC, created_at ASC, id ASC
        "#,
        message_id
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(contents)
}

/// Get all content blocks for multiple messages in a single query
/// Returns a HashMap mapping message_id -> Vec<MessageContent>
/// This is much more efficient than calling get_message_contents() N times
pub async fn get_message_contents_batch(
    pool: &PgPool,
    message_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<MessageContent>>, AppError> {
    use std::collections::HashMap;

    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let contents = sqlx::query_as!(
        MessageContent,
        r#"
        SELECT id, message_id, content_type, content, sequence_order,
               created_at as "created_at: _", updated_at as "updated_at: _"
        FROM message_contents
        WHERE message_id = ANY($1)
        ORDER BY message_id, sequence_order ASC, created_at ASC, id ASC
        "#,
        message_ids
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    // Group contents by message_id
    let mut map: HashMap<Uuid, Vec<MessageContent>> = HashMap::new();
    for content in contents {
        map.entry(content.message_id).or_default().push(content);
    }

    Ok(map)
}

/// Cancel any pending elicitation_request content blocks for the given message.
/// Called when the streaming task ends to ensure stale 'pending' rows are resolved.
pub async fn cancel_pending_elicitations(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE message_contents
        SET content = content || '{"status": "cancelled"}'::jsonb, updated_at = NOW()
        WHERE message_id = $1
          AND content_type = 'elicitation_request'
          AND content->>'status' = 'pending'
        "#,
        message_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

/// Merge JSONB fields into an existing content block (shallow merge using `||` operator).
/// Only the provided keys are updated; all other fields are preserved.
pub async fn update_content_json(
    pool: &PgPool,
    content_id: Uuid,
    patch: serde_json::Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE message_contents
        SET content = content || $2, updated_at = NOW()
        WHERE id = $1
        "#,
        content_id,
        patch,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    /// TEST-12: `append_content`'s header tells the reader that
    /// `UNIQUE (message_id, sequence_order)` already exists and names the constraint
    /// enforcing it — it used to describe that constraint as future work ("the next
    /// step") long after migration 124 shipped it, which sends the next reader
    /// looking for a guard that is already there, or worse, adding a third one.
    ///
    /// Rather than lint the prose (any paraphrase would slip through), this checks
    /// the doc against the migrations: the constraint the comment cites must actually
    /// be created by one. So RENAMING the constraint without updating the comment
    /// fails here — a doc that names a guard nobody can find is exactly the defect
    /// this replaces.
    ///
    /// Honest limit: it proves a migration ADDS the name, not that the guard still
    /// exists — a later `DROP CONSTRAINT` would leave this green. The live-schema
    /// half is covered by the integration test
    /// `chat::append_content_ordering_test::message_contents_has_exactly_one_unique_sequence_guard`,
    /// which queries `pg_index` on a real migrated DB and asserts the constraint both
    /// survives and still rejects a colliding slot. The two together are the property.
    ///
    /// Source-scanning shape mirrors `code_sandbox::backend::wsl2`'s
    /// `med3_wslenv_credential_leak_regression`; the production section is scanned
    /// alone so the test mod's own strings can't satisfy it.
    #[test]
    fn append_content_doc_cites_a_constraint_that_really_exists() {
        let normalized = include_str!("contents.rs").replace("\r\n", "\n");
        let prod_src = normalized
            .split_once("#[cfg(test)]\nmod tests {")
            .map(|(prod, _)| prod)
            .expect("test mod marker present");

        let cited = "uq_message_contents_message_sequence";
        assert!(
            prod_src.contains(cited),
            "append_content's doc must name the constraint that enforces the \
             (message_id, sequence_order) invariant, so a reader can find it."
        );

        // The cited name must be real: some migration must create it.
        let migrations = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");
        let mut found_in = None;
        for entry in std::fs::read_dir(migrations).expect("read migrations dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }
            let sql = std::fs::read_to_string(&path).expect("read migration");
            if sql.contains(&format!("ADD CONSTRAINT\n    {cited}"))
                || sql.contains(&format!("ADD CONSTRAINT {cited}"))
            {
                found_in = Some(path);
                break;
            }
        }
        assert!(
            found_in.is_some(),
            "append_content's doc cites constraint `{cited}`, but no migration creates it \
             — the comment points the reader at a guard that does not exist."
        );
    }
}

//! DB access for `mcp_tool_calls` (free functions over `&PgPool`, mirroring
//! `workflow/repository.rs`).

use sqlx::PgPool;
use uuid::Uuid;

use crate::common::AppError;

use super::models::{CreateMcpToolCall, McpToolCall};

/// Insert one recorded tool call, returning the full row.
pub async fn insert_call(pool: &PgPool, req: CreateMcpToolCall) -> Result<McpToolCall, AppError> {
    let row = sqlx::query_as!(
        McpToolCall,
        r#"
        INSERT INTO mcp_tool_calls (
            server_id, server_name, is_built_in, user_id, conversation_id,
            branch_id, message_id, tool_use_id, tool_name, arguments_json,
            source, status, is_error, result_json, content_kinds, result_bytes,
            error_message, started_at, finished_at, duration_ms, workflow_run_id,
            review_classification
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)
        RETURNING
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        "#,
        req.server_id,
        req.server_name,
        req.is_built_in,
        req.user_id,
        req.conversation_id,
        req.branch_id,
        req.message_id,
        req.tool_use_id,
        req.tool_name,
        req.arguments_json,
        req.source.as_str(),
        req.status.as_str(),
        req.is_error,
        req.result_json,
        &req.content_kinds,
        req.result_bytes,
        req.error_message,
        req.started_at,
        req.finished_at,
        req.duration_ms,
        req.workflow_run_id,
        req.review_classification,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// EVERY column [`list_calls_for_user`] narrows on under `user_id = $1` — read off
/// the query, NOT off the table's column list.
///
/// Lives OUTSIDE `#[cfg(test)]` on purpose (FIX_ROUND-7): the authoritative
/// owner-leading index guard is an INTEGRATION test
/// (`tests/mcp/tool_call_index_test.rs`), which is a separate crate linked against
/// the lib built WITHOUT `cfg(test)`. A `pub(crate)` const inside the unit-test
/// module was invisible to it, so that test hand-duplicated this list and the
/// "the two cannot drift" claim was false — a sixth narrowing would have updated
/// only one copy and silently narrowed a security guard.
pub const FILTERED_LOOKUP_COLUMNS: [&str; 5] = [
    "server_id",
    "conversation_id",
    "is_built_in",
    "tool_use_id",
    "message_id",
];

/// The optional filters `GET /api/mcp/tool-calls` accepts, grouped so adding one
/// does not grow every signature between the handler and the SQL (clippy's
/// `too_many_arguments` was already at the limit).
///
/// EVERY field is an OPTIONAL narrowing applied ON TOP of the mandatory
/// `user_id = $1` predicate — see [`list_calls_for_user`]. None of them can widen
/// the result set past the caller's own rows.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolCallFilters<'a> {
    pub server_id: Option<Uuid>,
    pub conversation_id: Option<Uuid>,
    pub is_built_in: Option<bool>,
    /// ITEM-13: join the chat transcript to its recorded call. `tool_use_id` is
    /// the id the LLM stamped on the `tool_use` content block, so this is how a
    /// rail step resolves duration / source / size for the call it renders.
    pub tool_use_id: Option<&'a str>,
    /// ITEM-13: every call recorded under one assistant message (the whole rail).
    pub message_id: Option<Uuid>,
}

/// List a user's tool calls, newest-first, with the optional
/// server/conversation/built-in/tool-use/message filters. Returns `(rows, total)`;
/// the handler derives `total_pages`.
///
/// SECURITY: `user_id = $1` is unconditional and is the cross-user guard. Every
/// filter below it is a `($n IS NULL OR col = $n)` NARROWING — a filter can only
/// ever remove rows from the owner's set, never add another user's. The COUNT
/// query repeats the identical predicate list so `total` can't disagree with the
/// page. Pinned by `filters_never_drop_the_owner_predicate` (TEST-17).
pub async fn list_calls_for_user(
    pool: &PgPool,
    user_id: Uuid,
    filters: ToolCallFilters<'_>,
    page: i64,
    per_page: i64,
) -> Result<(Vec<McpToolCall>, i64), AppError> {
    let ToolCallFilters {
        server_id,
        conversation_id,
        is_built_in,
        tool_use_id,
        message_id,
    } = filters;
    let per_page = per_page.clamp(1, 200);
    let offset = (page - 1).max(0) * per_page;

    let rows = sqlx::query_as!(
        McpToolCall,
        r#"
        SELECT
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM mcp_tool_calls
        WHERE user_id = $1
          AND ($2::uuid IS NULL OR server_id = $2)
          AND ($3::uuid IS NULL OR conversation_id = $3)
          AND ($4::bool IS NULL OR is_built_in = $4)
          AND ($5::text IS NULL OR tool_use_id = $5)
          AND ($6::uuid IS NULL OR message_id = $6)
        ORDER BY created_at DESC
        LIMIT $7 OFFSET $8
        "#,
        user_id,
        server_id,
        conversation_id,
        is_built_in,
        tool_use_id,
        message_id,
        per_page,
        offset,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::database_error)?;

    let total = sqlx::query!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM mcp_tool_calls
        WHERE user_id = $1
          AND ($2::uuid IS NULL OR server_id = $2)
          AND ($3::uuid IS NULL OR conversation_id = $3)
          AND ($4::bool IS NULL OR is_built_in = $4)
          AND ($5::text IS NULL OR tool_use_id = $5)
          AND ($6::uuid IS NULL OR message_id = $6)
        "#,
        user_id,
        server_id,
        conversation_id,
        is_built_in,
        tool_use_id,
        message_id,
    )
    .fetch_one(pool)
    .await
    .map_err(AppError::database_error)?
    .count;

    Ok((rows, total))
}

/// Fetch a single tool-call row by id, scoped to its owner. Ownership is
/// enforced in SQL (not just the handler) so a future caller can't leak a
/// cross-user row.
pub async fn find_call_for_user(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<McpToolCall>, AppError> {
    let row = sqlx::query_as!(
        McpToolCall,
        r#"
        SELECT
            id,
            server_id,
            server_name,
            is_built_in,
            user_id,
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            tool_name,
            arguments_json as "arguments_json: _",
            source,
            status,
            is_error,
            result_json as "result_json: _",
            content_kinds as "content_kinds: _",
            result_bytes,
            error_message,
            started_at as "started_at: _",
            finished_at as "finished_at: _",
            duration_ms,
            created_at as "created_at: _",
            updated_at as "updated_at: _"
        FROM mcp_tool_calls
        WHERE id = $1 AND user_id = $2
        "#,
        id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(row)
}

/// ITEM-17 / DEC-1 reveal: the RAW, UNREDACTED arguments for a recorded call.
///
/// Deliberately NOT read from `mcp_tool_calls.arguments_json`:
/// `record::cap_arguments` redacts BEFORE the insert, so that column has never
/// held the raw value — a reveal reading it would just echo `[redacted]`. The raw
/// arguments live on the paired `message_contents` `tool_use` block's `input`
/// (persisted by the chat path exactly as the model emitted it).
///
/// Owner-scoped INDEPENDENTLY of the caller's tool-call row: the block is reached
/// only through `branch_messages → branches → conversations` with
/// `conversations.user_id = $3`, so this cannot pull another user's transcript
/// even if it were handed a foreign `message_id`.
///
/// `Ok(None)` means the block is gone (message deleted / branch pruned) — the
/// handler falls back to the recorded (redacted) arguments rather than erroring.
pub async fn find_raw_tool_use_input(
    pool: &PgPool,
    message_id: Uuid,
    tool_use_id: &str,
    user_id: Uuid,
) -> Result<Option<serde_json::Value>, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT mc.content -> 'input' AS "input?: serde_json::Value"
        FROM message_contents mc
        JOIN branch_messages bm ON bm.message_id = mc.message_id
        JOIN branches b ON b.id = bm.branch_id
        JOIN conversations c ON c.id = b.conversation_id
        WHERE mc.message_id = $1
          AND mc.content_type = 'tool_use'
          AND mc.content ->> 'id' = $2
          AND c.user_id = $3
        LIMIT 1
        "#,
        message_id,
        tool_use_id,
        user_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(AppError::database_error)?;
    // A block with no `input` key yields SQL NULL; one with `"input": null` yields
    // JSON null. Neither is a revealable value, so both degrade to the recorded
    // arguments rather than reporting `raw: true` with nothing in it.
    Ok(row
        .and_then(|r| r.input)
        .filter(|v| !v.is_null()))
}

/// Delete every row older than `cutoff` (the retention prune). Returns the
/// number of rows removed.
pub async fn prune_calls_older_than(
    pool: &PgPool,
    cutoff: time::OffsetDateTime,
) -> Result<u64, AppError> {
    let res = sqlx::query!(
        r#"DELETE FROM mcp_tool_calls WHERE created_at < $1"#,
        cutoff,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    /// This module's own source, so the tests below can assert on the SQL the
    /// `query_as!` macros embed (the macros consume the literal at compile time,
    /// so there is no runtime handle to it). Same "read the real artifact"
    /// technique `chat/core/repository/contents.rs` uses against `migrations/`.
    const SOURCE: &str = include_str!("repository.rs");

    /// Every `SELECT`/`UPDATE`/`DELETE` body in this file that touches
    /// `mcp_tool_calls`, as raw SQL text.
    fn tool_call_statements() -> Vec<&'static str> {
        SOURCE
            .split("r#\"")
            .skip(1)
            .filter_map(|chunk| chunk.split("\"#").next())
            .filter(|sql| sql.contains("mcp_tool_calls"))
            .collect()
    }

    /// TEST-17 (ITEM-13): the new `tool_use_id` / `message_id` filters compose
    /// into the owner-scoped query WITHOUT dropping the `user_id` predicate —
    /// the cross-user guard. Asserted on the real SQL, not a paraphrase.
    #[test]
    fn filters_never_drop_the_owner_predicate() {
        let statements = tool_call_statements();
        assert!(
            statements.len() >= 4,
            "expected the insert + page + count + single-row statements, saw {}",
            statements.len()
        );

        for sql in &statements {
            // The retention prune is deployment-wide by design (it deletes by
            // `created_at` across all users); everything else is owner-scoped.
            if sql.contains("DELETE FROM mcp_tool_calls") {
                continue;
            }
            if sql.contains("INSERT INTO mcp_tool_calls") {
                assert!(sql.contains("user_id"), "insert must carry an owner: {sql}");
                continue;
            }
            assert!(
                sql.contains("WHERE user_id = $1") || sql.contains("WHERE id = $1 AND user_id = $2"),
                "every read of mcp_tool_calls must be owner-scoped in SQL: {sql}"
            );
        }

        // The two LIST statements (page + count) carry the identical predicate
        // list, including the two ITEM-13 filters, so `total` can never disagree
        // with the page.
        let listing: Vec<&&str> = statements
            .iter()
            .filter(|s| s.contains("WHERE user_id = $1"))
            .collect();
        assert_eq!(listing.len(), 2, "expected exactly the page + count statements");
        for sql in listing {
            assert!(sql.contains("WHERE user_id = $1"), "owner predicate present: {sql}");
            assert!(
                sql.contains("($2::uuid IS NULL OR server_id = $2)"),
                "server filter preserved: {sql}"
            );
            assert!(
                sql.contains("($3::uuid IS NULL OR conversation_id = $3)"),
                "conversation filter preserved: {sql}"
            );
            assert!(
                sql.contains("($4::bool IS NULL OR is_built_in = $4)"),
                "built-in filter preserved: {sql}"
            );
            assert!(
                sql.contains("($5::text IS NULL OR tool_use_id = $5)"),
                "ITEM-13 tool_use_id filter present: {sql}"
            );
            assert!(
                sql.contains("($6::uuid IS NULL OR message_id = $6)"),
                "ITEM-13 message_id filter present: {sql}"
            );
            // Every filter is an AND-narrowing under the owner predicate — never
            // an OR that could widen past it.
            let after_owner = sql.split("WHERE user_id = $1").nth(1).unwrap();
            let filter_section = after_owner
                .split("ORDER BY")
                .next()
                .unwrap()
                .trim_end()
                .trim_end_matches(|c: char| c.is_whitespace());
            for line in filter_section.lines().map(str::trim).filter(|l| !l.is_empty()) {
                assert!(
                    line.starts_with("AND "),
                    "filter `{line}` must AND-narrow under the owner predicate"
                );
            }
        }
    }

    /// The reveal lookup is owner-scoped through the CONVERSATION, not merely
    /// through the tool-call row it was resolved from (defense in depth: a
    /// mis-scoped call row still cannot pull another user's transcript).
    #[test]
    fn raw_tool_use_lookup_is_conversation_owner_scoped() {
        let sql = SOURCE
            .split("r#\"")
            .skip(1)
            .filter_map(|chunk| chunk.split("\"#").next())
            .find(|s| s.contains("content_type = 'tool_use'"))
            .expect("the raw tool_use lookup must exist");
        assert!(sql.contains("JOIN branch_messages"), "joins the branch: {sql}");
        assert!(sql.contains("JOIN conversations"), "joins the conversation: {sql}");
        assert!(sql.contains("c.user_id = $3"), "owner-scoped on the conversation: {sql}");
        assert!(
            !sql.contains("arguments_json"),
            "the reveal must NOT read the pre-redacted mcp_tool_calls column: {sql}"
        );
    }

    /// FIX_ROUND-6: the hand-rolled SQL replay this used to be is DELETED.
    ///
    /// FIX_ROUND-2 introduced a unit test that re-implemented enough of a SQL
    /// parser to replay every migration's `CREATE`/`DROP INDEX` and assert the
    /// owner-leading rule over the result. Rounds 3, 4 and 5 each hardened it and
    /// each subsequent blind audit found more valid SQL that evaded it —
    /// `CREATE UNIQUE INDEX`, lowercase DDL, `CONCURRENTLY` on the DROP side,
    /// `ALTER TABLE … ADD CONSTRAINT`, an uppercase table reference, a
    /// column-level `UNIQUE`, an inline constraint in `CREATE TABLE`, a
    /// dollar-quoted body, a multi-action `ALTER`, and a comment/string-literal
    /// interaction that silently discarded a whole file. Round 6 finally
    /// constructed a working bypass: a migration that ADDS a non-owner-leading
    /// index on a filtered column and passes.
    ///
    /// The lesson is not "harden it again". A text replay of SQL is unsound by
    /// construction, and a SECURITY guard that is unsound by construction is
    /// worse than none, because it reads as coverage. The authoritative source of
    /// truth for "which indexes exist on this table" is the DATABASE, after the
    /// migrations have actually run — so the guard moved to
    /// `tests/mcp/tool_call_index_test.rs`, which queries `pg_indexes`. That
    /// version needs no parser, cannot be evaded by a spelling, and covers index
    /// creation by ANY route (explicit DDL, a table or column constraint, a
    /// primary key, or anything a future migration invents).
    ///
    /// What remains here is the column list itself, shared with that test.
    #[test]
    fn filtered_lookup_columns_match_the_query() {
        // The list is only meaningful if it tracks the SQL, so parse the ACTUAL
        // WHERE clause of `list_calls_for_user` and compare.
        //
        // FIX_ROUND-7: bounded by the real `ORDER BY`, not a hardcoded 400-byte
        // window that silently truncated a 7th narrowing mid-predicate — and
        // every `AND (` line must PARSE, so a narrowing written in any other
        // shape (`AND (($7…`, a function on the left, `= ANY($7)`) fails loudly
        // instead of being skipped into agreement with a stale const.
        let src = include_str!("repository.rs");
        let start = src
            .find("FROM mcp_tool_calls")
            .expect("locate the list query");
        let rest = &src[start..];
        let end = rest.find("ORDER BY").expect("locate the end of the WHERE clause");
        let where_clause = &rest[..end];

        // Parse EVERY optional narrowing, wherever it sits.
        //
        // FIX_ROUND-8: the previous loop only inspected lines that START with
        // `AND (`, so its "fails loudly" property was conditional on formatting.
        // Two ordinary spellings were verified to slip through SILENTLY and leave
        // a stale const in agreement: a narrowing appended to the
        // `WHERE user_id = $1` line, and two narrowings on one line. Split on the
        // `AND (` TOKEN instead, and require every fragment to parse.
        let mut found: Vec<String> = Vec::new();
        let flat = where_clause.split_whitespace().collect::<Vec<_>>().join(" ");
        for frag in flat.split("AND (").skip(1) {
            let predicate = frag.split(')').next().unwrap_or_default();
            let col = predicate
                .split(" OR ")
                .nth(1)
                .and_then(|c| c.split(" =").next())
                .map(str::trim)
                .filter(|c| {
                    !c.is_empty() && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                });
            match col {
                Some(c) => found.push(c.to_string()),
                None => panic!(
                    "an optional narrowing this test cannot parse: `AND ({predicate})`. \
                     It would be SILENTLY dropped, letting a stale \
                     FILTERED_LOOKUP_COLUMNS agree with a query it no longer matches. \
                     Teach this parser the new shape."
                ),
            }
        }

        assert!(
            !found.is_empty(),
            "parsed no narrowings at all — the anchor no longer matches and this \
             guard would be vacuous"
        );

        found.sort();
        let mut expected: Vec<String> = super::FILTERED_LOOKUP_COLUMNS
            .iter()
            .map(|c| c.to_string())
            .collect();
        expected.sort();
        assert_eq!(
            found, expected,
            "FILTERED_LOOKUP_COLUMNS must equal the optional narrowings in \
             list_calls_for_user's WHERE clause — the owner-leading index guard in \
             tests/mcp/tool_call_index_test.rs is scoped by this list, so a drift \
             here silently narrows a security guard"
        );
    }

}

# PLAN — fix-duplicate-tool-result

## Problem

A turn calling MULTIPLE tools intermittently dies with:

```
AI provider error: Invalid request: messages.14.content.2: each tool_use must have a single
result. Found multiple `tool_result` blocks with id: toolu_016kKfomh1bC1HqJR5QtmbU6
```

**Root cause (confirmed by code trace, not hypothesis).** A **mixed parallel batch** — ≥1
approval-**exempt** built-in tool + ≥1 approval-**required** tool — plus the approval-resume path:

1. Model emits `tool_use A` (built-in) + `tool_use B` (external, needs approval). `finalize()`
   persists both.
2. `after_llm_call` runs A, persists **only A's** result, pauses for B — `mcp.rs:3094-3110`.
   DB: `[tool_use A, tool_use B, tool_result A]`.
3. On resume, `group_assistant_blocks` hits the trailing branch with `batch_has_result == true`
   (`streaming.rs:1775`, an `.any()` over the batch) → misreads B as a *gap*, not
   *awaiting-approval* → synthesizes an `is_error` placeholder for B (`streaming.rs:1649`,
   emitted `:1683-1685`).
4. `before_llm_call` executes the now-approved B and appends B's **real** result as an extra
   `User` message (`mcp.rs:1546-1554`).

Result: `Assistant{use A, use B}` / `Tool{result A, result B SYNTHETIC}` / `User{result B REAL}`
— two `tool_result` blocks for B. The keep-first dedup at `streaming.rs:1746` never sees the
second: the `User` message is appended AFTER `group_assistant_blocks` returned.

**Refuted:** `clear_old_tool_results` mutates `content` in place via `&mut` and never
pushes/inserts (`:1921-1936`, `:1942-1962`) — block count cannot change. Duplicate `tool_result`
DB *rows* are also not the cause (deduped keep-first at `:1746`).

## Items

- **ITEM-1**: `mcp.rs` — extract a pure `replace_or_collect_tool_results(&mut [ChatMessage], Vec<ContentBlock>) -> Vec<ContentBlock>`: a freshly-executed result OVERWRITES any existing `tool_result` with the same `tool_use_id` in `request.messages`; only ids with no existing block are returned for the `User` message. Wire it into `before_llm_call` at the `mcp.rs:1546` push site; skip the push when the leftovers are empty. This is the SOURCE fix — in-place (not drop-the-placeholder) because Anthropic also requires the result in the message immediately AFTER the tool_use turn; dropping it would re-break pairing into the sibling `chat-toolresult-pairing` 400.
- **ITEM-2**: `streaming.rs` — new pure `dedup_tool_results_by_id(&mut Vec<ChatMessage>)`: keep the FIRST `tool_result` per `tool_use_id`, drop later duplicates, `tracing::warn!` the dropped id, and remove any message left with empty content. Call it at the single chokepoint (`streaming.rs:468`, immediately before `clear_old_tool_results` and the lone `chat_stream` at `:475`) so it runs after every extension mutation. DEFENSE — with ITEM-1 in place it should never fire; the warn is the tripwire.
- **ITEM-3**: `streaming.rs` — `results_by_id.clear()` at the end of `flush_assistant_tool_pair` (`:1700`). Today the map is never cleared between flushes, so a stale orphan result for id X is preferred by `or_insert` over a later REAL result for X (wrong content, not a duplicate). Leftovers there are orphans the caller already intends to drop; matches the docstring "results captured since the last flush".
- **ITEM-4**: `mcp.rs:1074-1085` — claim-then-execute. Today a failed approval-row DELETE is logged and swallowed, execution proceeds, and the next `before_llm_call` re-executes the tool and appends a SECOND `tool_result` row. Delete the approval row BEFORE running the tool; if the delete fails, skip execution and record an `is_error` result so the `tool_use` stays paired. **Amended per DRIFT-1.1:** the claim becomes the loop's SINGLE delete point — three further `delete_tool_approval` calls (the server-not-found, sampling-no-session and connect-fail arms, each of which `continue`s before the post-execution delete and so carries its own copy) become dead double-deletes once the row is claimed up front, and are removed. This makes the anti-spin property structural rather than something every new error arm must remember to re-implement.
- **ITEM-5**: `contents.rs:51-52` — the header comment prescribes `UNIQUE (message_id, sequence_order)` + retry as "the next step"; the constraint shipped in migration 124 (and 114). Correct the comment to state the actual current state: the constraint EXISTS, no retry was added, so a genuinely-concurrent append hard-errors rather than silently colliding. No retry added (the sole production caller is sequential — that is scope creep).
- **ITEM-6**: New migration `00000000000158_drop_redundant_message_contents_index.sql` — migrations 114 and 124 BOTH create a unique index on `message_contents(message_id, sequence_order)` (`idx_message_contents_message_seq_unique` and constraint `uq_message_contents_message_sequence`). Both survive → double write cost + a confusing constraint name in violation errors. `DROP INDEX IF EXISTS idx_message_contents_message_seq_unique`, keeping 124's named constraint. Shipped migrations cannot be edited.
- **ITEM-7**: `lib.rs` `test_internals` — re-export `dedup_tool_results_by_id` (and keep `group_assistant_blocks`) so `tests/chat/assistant_block_grouping_test.rs` can assert the request-level invariant.

## Files to touch

- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — ITEM-1 (~1546 + new helper), ITEM-4 (~1074)
- `src-app/server/src/modules/chat/core/services/streaming.rs` — ITEM-2 (~468 + new fn), ITEM-3 (~1700)
- `src-app/server/src/modules/chat/core/repository/contents.rs` — ITEM-5 (comment only)
- `src-app/server/migrations/00000000000158_drop_redundant_message_contents_index.sql` — ITEM-6 (new)
- `src-app/server/src/lib.rs` — ITEM-7 (`test_internals` re-export)
- `src-app/server/tests/chat/assistant_block_grouping_test.rs` — TEST-9

## Patterns to follow

- **Pure-fn + in-module unit tests** → mirror `modules/project/chat_extension/project.rs:80`
  (`apply_project_context`): the async DB-coupled hook delegates to a sync pure fn taking
  already-resolved values and mutating through `&mut`, with `#[cfg(test)] mod tests` in the same
  file. `group_assistant_blocks` (`streaming.rs:1726`) is the same shape already applied in chat —
  both new helpers follow it.
- **Test fixtures** → reuse the existing `tool_use` / `tool_result` / `result_ids` / `tool_use_ids`
  builders in `streaming.rs:2060-2247` and `mod trim_tests`' `tool_result_msg` (`:2511`). Do not
  invent a parallel fixture vocabulary.
- **Integration assertion** → extend `assert_valid_tool_pairing`
  (`tests/chat/assistant_block_grouping_test.rs:58`) rather than writing a new invariant checker;
  reach private fns via the `ziee::test_internals` bridge (`lib.rs:308`), the established pattern.
- **Migration** → mirror `00000000000124_message_contents_sequence_unique.sql` (header comment
  explaining WHY, then the DDL). Numbering continues from 157.
- **Approval repo access** → ITEM-4 uses the existing
  `Repos.chat.mcp.delete_tool_approval(tool_use_id, message_id)`; no new repository fn.

## Non-goals (explicit)

- No retry loop in `append_content` (ITEM-5 corrects the comment only).
- No `(message_id, tool_use_id)` uniqueness constraint — `tool_use_id` lives inside the `content`
  JSONB; enforcing it would need an expression index + a backfill. Out of scope.
- No change to `clear_old_tool_results` (exonerated).
- No UI surface — backend-only diff, so no e2e/gallery/`npm run check` gate applies.

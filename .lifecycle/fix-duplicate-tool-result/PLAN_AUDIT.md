# PLAN_AUDIT — fix-duplicate-tool-result

Plan audited against the codebase BEFORE writing code. Every claim below was checked by reading
the cited file:line in this worktree.

## Breakage risk

- **ITEM-1 (`mcp.rs:1546` push site).** Existing callers of `before_llm_call` read only the
  `BeforeLlmAction` return; the `request.messages` mutation is a side effect. Changing "always
  push a User message" → "replace in place, push only the leftovers" is observable ONLY in the
  assembled request. Risk: if a fresh result's id has NO existing block (the pure
  awaiting-approval batch — `batch_has_result == false` → bare Assistant turn, no Tool message),
  we MUST still push it, or we re-introduce the sibling `chat-toolresult-pairing` 400. The
  helper's leftover-vec return is exactly that path; TEST-7 pins it.
- **ITEM-2 (dedup at `:468`).** Runs on `chat_request.messages` after every extension mutation and
  before the lone `chat_stream` call (`:475` — verified the only call site in the chat module;
  `title.rs:58` is a separate request the extension builds itself and is untouched). Dropping a
  LATER duplicate can never orphan a tool_use: `group_assistant_blocks` places the FIRST
  occurrence in the Tool turn immediately after the Assistant turn, so keep-first preserves
  adjacency. Removing a message emptied by the drop is required — Anthropic rejects an empty
  `content` array.
- **ITEM-3 (`results_by_id.clear()`).** Behavior change inside the trailing/flush walk. Audited
  the existing tests: TEST-8 `group_assistant_blocks_dedups_duplicate_result` (`:2397`) inserts
  the duplicate BEFORE the flush, so it exercises `or_insert` while the entry is live — unaffected
  by a post-flush clear. `group_assistant_blocks_drops_orphan_tool_result` (`:2347`) and
  `..._drops_mid_stream_orphan_result` (`:2374`) both ASSERT orphans are dropped, which the clear
  makes *more* true, not less. No test depends on a result surviving across a flush.
- **ITEM-4 (claim-then-execute).** Real behavior change: a crash/panic between the DELETE and the
  result append leaves the tool un-run with no approval row → its `tool_use` gets a synthesized
  `is_error` placeholder on the next turn (VALID, degraded). Today's order instead risks
  re-running an expensive tool and appending a second `tool_result` row. Accepted trade-off,
  recorded as DEC-4. The loop at `mcp.rs:598` (`for approval in approved_pending`) already
  deletes the row early in the no-`server_id` arm (`:653`, "delete the approval row so the loop
  can't spin to max_iteration"), so claim-first matches an existing precedent in the SAME loop.
- **ITEM-6 (migration 158).** `DROP INDEX IF EXISTS idx_message_contents_message_seq_unique` is
  safe ONLY because migration 124's `uq_message_contents_message_sequence` constraint provides
  byte-identical protection on the same columns — verified both exist and both are on
  `(message_id, sequence_order)`. `IF EXISTS` keeps it idempotent on a DB that never ran 114.
  Postgres will not let the constraint's backing index be dropped, so a mistaken name cannot
  silently remove the real guard (it would error).
- **ITEM-5.** Comment-only. Zero breakage risk.

## Pattern conformance

- Both new helpers follow **`apply_project_context`** (`modules/project/chat_extension/project.rs:80`):
  async DB-coupled hook → sync pure fn over already-resolved values, mutating via `&mut`, with
  `#[cfg(test)] mod tests` in the same file. `group_assistant_blocks` (`streaming.rs:1726`) is the
  same shape already applied in chat, and its docstring states the intent verbatim ("Pure +
  registry-free so the invariant is directly unit-testable"). CONFORMS.
- Test fixtures reuse `tool_use`/`tool_result`/`result_ids`/`tool_use_ids` (`streaming.rs:2060-2247`)
  and `tool_result_msg` (`:2511`); the integration side reuses `assert_valid_tool_pairing`
  (`tests/chat/assistant_block_grouping_test.rs:58`) and the `ziee::test_internals` bridge
  (`lib.rs:308`). No parallel fixture vocabulary. CONFORMS.
- Migration mirrors `00000000000124_message_contents_sequence_unique.sql` (why-comment then DDL).
  CONFORMS.
- ITEM-4 reuses `Repos.chat.mcp.delete_tool_approval(tool_use_id, message_id)` — no new repository
  fn. CONFORMS.
- **Deviation noted:** `mcp.rs` has a `#[cfg(test)] mod tests` (`:3442`) that currently imports
  only free fns (`build_artifact_download_url`, `tool_system_guidance`, …). The new helper must be
  a FREE fn (not an `impl` method) to stay unit-testable there — this is why ITEM-1 specifies a
  free fn taking `&mut [ChatMessage]` rather than a method on the extension. Consistent with how
  `resolve_unique_tool_use_id` (`:326`) is already factored in the same file.

## Migration collisions

- Highest existing: `00000000000157_remove_unused_builtin_mcp_servers.sql`. This branch adds
  **158** — no collision against base `khoi` @ `c66cd5d76`.
- No other live worker adds a migration: checked `fix/mcp-tool-title-generation` (touches
  `registry.rs` + a test only). Re-verified at merge by the merge-gate's C2 against real base.
- `build.rs` re-runs all migrations into the per-worktree build DB, so 158 is exercised by the
  next `cargo check` — a bad DDL fails the build immediately rather than at runtime.

## OpenAPI regen

- **Not required.** The diff adds no route, no request/response struct, no serde-exposed enum
  variant, no permission, and no sync entity. `ContentBlock` / `ChatMessage` are `ai-providers`
  wire types consumed internally by the assembler — untouched. `openapi.json` and
  `api-client/types.ts` in BOTH `ui/` and `desktop/ui/` stay byte-identical.
- Consequence: `just openapi-regen` is NOT in the phase-8 chain, and the golden parity test
  `openapi::emit_ts::tests::types_ts_parity` should stay green untouched. If it goes red, that is
  a real signal I changed an exposed type by accident.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — free fn mirrors `apply_project_context` + `resolve_unique_tool_use_id`; the leftover-vec return preserves the no-placeholder path (`chat-toolresult-pairing` regression guarded by TEST-7)
- **ITEM-2** — verdict: PASS — `:468` is the single chokepoint before the lone `chat_stream` at `:475`; keep-first preserves tool_use adjacency; empty-message removal required and enumerated (TEST-4)
- **ITEM-3** — verdict: PASS — no existing test depends on a result surviving a flush; the two orphan-drop tests (`:2347`, `:2374`) become more true, TEST-8 (`:2397`) is unaffected (duplicate arrives pre-flush)
- **ITEM-4** — verdict: CONCERN — genuine behavior change (crash mid-execution ⇒ tool not re-run). Accepted and recorded as DEC-4; precedent for early-delete already exists in the same loop at `mcp.rs:653`. Not BLOCKED: the degraded outcome is protocol-VALID, and the status quo silently re-runs expensive tools
- **ITEM-5** — verdict: PASS — comment-only; corrects a claim falsified by migrations 114/124
- **ITEM-6** — verdict: PASS — no number collision (157 is highest); safe because 124's constraint duplicates the protection; `IF EXISTS` keeps it idempotent
- **ITEM-7** — verdict: PASS — `test_internals` (`lib.rs:308`) already re-exports `group_assistant_blocks` + the `ai_providers` wire types; adding one fn follows the established bridge pattern

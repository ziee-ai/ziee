# PLAN — fix: invalid Anthropic payload (`tool_use` without `tool_result`)

Bugfix. After a turn with **parallel** tool calls, the next send 400s with
`tool_use ids were found without tool_result blocks immediately after: toolu_…`,
bricking the whole conversation. Two independent defects each produce an unpaired
`tool_use`; both must be fixed so the assembled provider payload is valid **regardless
of whether tools succeed or fail**.

- **Defect A — assembler split.** `group_assistant_blocks` (`streaming.rs:1654`) flushes
  an `[Assistant{text+tool_use}, Tool{tool_result}]` pair only when `pending_ids` is
  exactly empty; a partial/failed/mismatched parallel batch falls to the trailing branch
  which emits the `tool_use` blocks but **drops `current_results`** → dangling `tool_use`.
- **Defect B — summarizer boundary cut.** `apply_summary_block` (`summarizer.rs:396`)
  drains a prefix by **DB-message count**, but the outbound array was already split into
  more messages by `group_assistant_blocks`, so the cut can land **between** an assistant
  `tool_use` and its `Tool` result → retained history starts on an orphan `tool_result`.

`clear_old_tool_results` (`streaming.rs:1784`) is safe (replaces content, never removes a
block). History load is the full branch. The 3 provider adapters are 1:1 mappers that
require pre-paired input, so fixing at the assembler/summarizer layer keeps
Anthropic/OpenAI/Gemini correct.

## Items

- **ITEM-1**: Make `group_assistant_blocks` always emit a valid sequence. For a
  completed batch (every id resolved, per-flush) OR a completed-but-partial trailing batch
  (≥1 tool_use already has a captured result), emit the Assistant message (leading
  thinking/text + all its `tool_use` blocks) immediately followed by a `Role::Tool`
  message answering **every** `tool_use` id — the matched results plus a synthesized
  `is_error` `ToolResult` for each still-unresolved id, in `tool_use` order, carrying the
  matching `tool_use`'s `name` (Gemini pairs `functionResponse` by name). A trailing batch
  with NO captured result (in-progress / awaiting-approval, whose real result is appended
  separately) stays a single Assistant turn — existing behavior preserved (see DRIFT-1.1).
- **ITEM-2**: Failed/errored/cancelled tools never leave a `tool_use` unpaired. MCP
  failure paths already persist a real `is_error` `tool_result`; ITEM-1's synthesized
  fallback covers any genuinely-absent/mismatched result. No persistence change.
- **ITEM-3**: Drop orphan trailing `tool_result` blocks that have **no** matching
  `tool_use` in the current batch (a `tool_result` with no preceding `tool_use` is itself
  a provider 400). Safe complement to ITEM-1; matches today's drop-on-trailing behavior.
- **ITEM-4**: Preserve leading non-tool blocks (thinking/text/image) on the **assistant**
  side of the split, in original order.
- **ITEM-5**: Guard the summarizer boundary (Defect B). In `apply_summary_block`, after
  computing `drop_until`, snap it forward while the first *retained* message is a
  `Role::Tool` message. A retained-leading `Role::Tool` is always an orphan whose
  `tool_use` is in the dropped prefix, so dropping it too (the summary text condenses it)
  is the correct, provider-agnostic snap. Never leaves the retained history starting on
  an orphan `tool_result`.

## Files to touch

- `src-app/server/src/modules/chat/core/services/streaming.rs` — rewrite the trailing
  branch of `group_assistant_blocks` (~L1690-1698); add a helper to synthesize an
  `is_error` `ToolResult` for an unmatched `tool_use` (id + name). Extend the in-file
  `#[cfg(test)] mod tests`.
- `src-app/server/src/modules/summarization/engine/summarizer.rs` — snap-forward guard in
  `apply_summary_block` (~L404-408); add a tool-block builder + regression test to the
  in-file `#[cfg(test)] mod tests`.
- `src-app/server/tests/chat/assistant_block_grouping_test.rs` — add integration cases
  reusing the existing `assert_valid_tool_pairing` invariant checker.

No migration, no OpenAPI/type/route change, no frontend change (backend-only assembler
logic). `test_internals` already re-exports `group_assistant_blocks`.

## Patterns to follow

- **Assembler unit tests** — mirror the existing `#[cfg(test)] mod tests` in
  `streaming.rs` (builders `tool_use`/`tool_result` at ~L1953-1972; tests
  `assistant_with_*` at ~L1980-2106). Closest module: itself.
- **Assembler integration tests** — mirror `tests/chat/assistant_block_grouping_test.rs`;
  reuse its `tool_use`/`tool_result` builders + `assert_valid_tool_pairing(msgs)` invariant
  checker (exactly the invariant this fix enforces).
- **Summarizer tests** — mirror `summarizer.rs`'s `#[cfg(test)] mod tests`
  (`user_msg`/`sys_msg`/`request_text` helpers, `apply_block_*` tests); add a
  `tool_use`/`tool_result` builder (none exists there yet) following the streaming.rs
  builder shape.
- **Synthesized error result wording** — mirror existing synthetic errors in
  `mcp.rs` (e.g. "Tool execution stopped: maximum iteration limit reached.") — a short,
  model-actionable string with `is_error: Some(true)`.

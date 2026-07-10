# PLAN_AUDIT — sandbox-tool-approval-loop

Audit of PLAN.md against the current codebase (worktree off `khoi`).

## Breakage risk

- **ITEM-1** touches only the three non-executing branches of `execute_approved_tools_sync`
  (`mcp.rs:343-349`, `354-370`, `467-486`). The success path (`mcp.rs:670-704`, which already
  pushes a result + deletes the approval) is unchanged, so no double-execution risk. Adding a
  `delete_tool_approval` + error tool_result to the previously-silent `None` branch and adding
  a delete to the two already-erroring branches only makes the loop terminate; no caller reads
  a "still-pending approved row" as a signal. `delete_tool_approval` is idempotent (a
  `DELETE … WHERE tool_use_id=$1 AND message_id=$2`), so a redundant delete is harmless. Each
  error branch will also push `tool_use_id` into the returned `executed_tool_use_ids` (mirrors
  the success path at `mcp.rs:671`) so the id is marked resolved even before the tool_result is
  persisted — defensive, no behavior regression.
- **ITEM-2** replaces `id: accumulated.id.unwrap_or_default()` (`mcp.rs:2769`). Risk: an id
  that a well-behaved provider (Anthropic `toolu_…`, real OpenAI `call_…`) emits uniquely is
  **preserved** (the helper only mints when empty OR a collision is detected), so existing
  single-provider flows are byte-identical. The only behavior change is for empty/duplicate ids
  — exactly the broken gpt-oss case. The added `get_message_with_content` at finalization is one
  extra read per assistant message (finalization already does DB writes downstream); on error it
  degrades to an empty `used` set (still fixes empty + within-batch dupes). Sorting the drained
  accumulator entries by `index` changes only determinism, not which blocks are produced.
- **ITEM-3** adds a new field + populates/consults/clears a per-message map. The well-formed
  `<uuid>__tool` arm of the split (`mcp.rs:2759-2760`) still short-circuits, so Anthropic/OpenAI
  paths that keep the prefix are untouched — recovery runs only in the previously-dead no-prefix
  arm. Ambiguous/not-found → leaves `server_id` empty → falls to ITEM-1's clear error (no
  mis-dispatch). Lock is held only for a `HashMap` read/clone + write, never across `.await`
  (matches `tool_use_accumulator`), so no deadlock/contention regression.
- **ITEM-4** is build-tooling only (a new `justfile` recipe + one dependency token); it cannot
  affect runtime behavior. Risk is limited to the recipe failing to compile the filter — covered
  by TEST-12 (runs it).

## Pattern conformance

- **ITEM-3** field/init/lock/drain mirrors `tool_use_accumulator` verbatim
  (`mcp.rs:265` field, `:276` `new()` init, `:2728-2745` lock+drain). `std::sync::Mutex`,
  `Arc<Mutex<HashMap<..>>>`, outer-key by `message_id`. CONFORMS.
- **ITEM-1** error tool_result matches the existing branch struct exactly
  (`mcp.rs:356-367`: all ten `McpContentData::ToolResult` fields incl. the `None`s) and the
  delete matches `mcp.rs:674-678`. CONFORMS.
- **ITEM-2** pure helper sits with the other unit-tested module helpers
  (`build_artifact_download_url`, `tool_system_guidance`) tested in `#[cfg(test)] mod tests`
  (`mcp.rs:2791`). CONFORMS.
- **ITEM-4** mirrors `check-sandbox-unit` (`justfile:107-110`) two-line `cargo test` shape.
  CONFORMS.
- Integration tests mirror `tests/mcp/approval_test.rs` / `mcp_approval_workflow_test.rs`
  (TestServer + fixtures). CONFORMS. ([[feedback_match_existing_patterns]])

## Migration collisions

None. No schema change — the fix is entirely in the chat-extension runtime. The
`tool_use_approvals` table + its `UNIQUE(message_id, tool_use_id)` constraint (migration
`00000000000017`) are unchanged; ITEM-2 makes ziee stop violating that constraint rather than
altering it. `ls migrations/` is not consulted by any item. No new migration file.

## OpenAPI regen

Not required. No request/response type, handler signature, or `#[derive(ToSchema)]` struct is
added or changed. The fix is internal to the MCP chat extension and touches no REST surface, so
neither `src-app/ui/src/api-client/types.ts` nor `src-app/desktop/ui` changes. `just openapi-regen`
is a no-op for this diff; the `openapi::emit_ts` golden parity test is unaffected. This is a
**backend-only** diff (the phase 3/8 frontend gates do not apply).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors the existing error-branch + `delete_tool_approval` idioms; only makes the loop terminate; no double-execution or caller-contract break.
- **ITEM-2** — verdict: PASS — preserves good provider ids; one extra idempotent read at finalization; fixes the exact `UNIQUE(message_id, tool_use_id)` + dedup collision.
- **ITEM-3** — verdict: PASS — mirrors `tool_use_accumulator`; recovery runs only in the previously-dead no-prefix arm; ambiguous/not-found degrade safely to ITEM-1's error.
- **ITEM-4** — verdict: PASS — build-tooling only, mirrors `check-sandbox-unit`; no runtime impact.

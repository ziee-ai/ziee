# PLAN_AUDIT — audited against the codebase

## Breakage risk

- `group_assistant_blocks` is `pub` but **only called from one production site**
  (`convert_history_to_messages_with_extensions`, `streaming.rs:1061`) and from
  `tests/chat/assistant_block_grouping_test.rs` via `test_internals`. Changing its
  trailing behavior affects only the assembled provider request; no other caller.
- The change is **strictly more-emitting** in the failure case (it now emits a Tool turn
  where it previously emitted none) and **byte-identical** in the all-matched case (the
  per-flush path at L1671 is unchanged, so a fully-resolved batch still produces exactly
  one pair). Existing tests `assistant_with_text_tool_use_and_tool_result_groups_to_two_messages`,
  `assistant_with_tool_use_and_tool_result_only_groups_to_two_messages`,
  `assistant_with_only_tool_result_emits_single_tool_message`,
  `no_trailing_assistant_message_after_tool_result` must stay green (regression guard).
  - Note: `assistant_with_only_tool_result_emits_single_tool_message` asserts a message
    tests the `#[cfg(test)]`-ONLY `group_blocks_into_provider_messages`, NOT the
    production `group_assistant_blocks` this fix changes — so it is unaffected and I do not
    touch that test-only function. Verified `group_assistant_blocks` ALREADY drops a lone
    orphan tool_result today (pending empty but `current_tool_uses` empty → no flush →
    trailing empty → results dropped), so ITEM-3 codifies existing behavior — no regression.
- `apply_summary_block` snap-forward only ADVANCES `drop_until` (drops more), never
  retains more; it cannot resurrect a dropped message. Worst case it drops one extra Tool
  message whose tool_use was already dropped — safe. Idempotent-insert of the summary
  System block is unchanged.

## Pattern conformance

- ITEM-1/3/4 mirror the file's own existing grouping idioms (drain/take of the
  accumulators). The synthesized-result helper is a small pure fn next to
  `group_assistant_blocks`, matching the file's pure-function-plus-unit-test convention.
- Synthesized `ContentBlock::ToolResult { tool_use_id, name: Some(<use.name>), content:
  vec![Text{…}], is_error: Some(true) }` matches the wire type at
  `ai-providers/src/models/chat.rs:150-159` and the conversion the MCP extension already
  produces (`content.rs:176-181`).
- ITEM-5 mirrors `apply_summary_block`'s existing clamp style (`.min(len)`); a `while`
  snap is the minimal local addition.
- Tests mirror `assistant_block_grouping_test.rs` (`assert_valid_tool_pairing`) and the
  in-file `mod tests` builders — the closest existing sibling for each.

## Migration collisions

- None. This branch adds no migration (highest base migration is 154; unchanged).

## OpenAPI regen

- Not required. No request/response type, route, permission, or enum changes. No
  `openapi.json` / `api-client/types.ts` delta. No frontend workspace touched → phase-3 /
  phase-8 frontend gates do not apply.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — single-caller pure fn; more-emitting in the failure case,
  byte-identical in the all-matched case; synthesized block matches the wire type.
- **ITEM-2** — verdict: PASS — no persistence change; MCP failure branches already emit
  `is_error` results (confirmed in `mcp.rs`), ITEM-1 covers residual gaps.
- **ITEM-3** — verdict: PASS — codifies existing `group_assistant_blocks` behavior (it
  already drops a lone orphan tool_result); the similarly-named existing test targets the
  test-only `group_blocks_into_provider_messages`, so no conflict.
- **ITEM-4** — verdict: PASS — `current_text` already accumulates leading non-tool blocks
  in order; the fix keeps them on the assistant side.
- **ITEM-5** — verdict: PASS — snap-forward only advances the cut past provably-orphan
  Tool messages; safe, provider-agnostic, mirrors existing clamp style.

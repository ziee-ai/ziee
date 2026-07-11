# BASE — conflict-surface scoping

## Migrations
- Highest existing migration on base: `00000000000154_add_voice_streaming_settings.sql`.
- **This branch adds ZERO migrations** (no schema change — pure assembler/summarizer logic).

## Files main/khoi may also touch
- `src-app/server/src/modules/chat/core/services/streaming.rs` — hot file; the
  `group_assistant_blocks` region (~L1654-1699) and its `#[cfg(test)] mod tests` are the
  edit surface. Risk if a concurrent branch also touches the assembler/trimming code.
- `src-app/server/src/modules/summarization/engine/summarizer.rs` — `apply_summary_block`
  (~L396-421) + its in-file test module.
- `src-app/server/tests/chat/assistant_block_grouping_test.rs` — additive test cases only.

Coordinating worker `stale-artifact-links` touches
`src-app/server/src/modules/code_sandbox/tools/files.rs` and the resource-link path — a
**different file set**, no expected collision.

## OpenAPI / codegen
- No `openapi.json` or `api-client/types.ts` regen implied (no type/route/response change).

## Merge note
- Rebase-check `streaming.rs` + `summarizer.rs` against current `origin/khoi` at merge-gate
  time (both are actively-edited files across the fleet).

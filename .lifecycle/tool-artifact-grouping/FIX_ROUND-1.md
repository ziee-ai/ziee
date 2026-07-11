# FIX_ROUND-1 — tool-artifact-grouping

Merged the phase-6 ledger, fixed every confirmed finding, then ran a fresh blind
re-audit of the fix delta.

## Fixes applied

1. **Scope the legacy artifact-attribution fallback to the message** (LEDGER:
   correctness/security/state-management — flagged by 3 independent angles).
   `resolveArtifactToolUseId` now intersects the single-in-flight candidate set
   with THIS message's `tool_use` ids (`messageUseIds`), so an in-flight call
   from another conversation / prior turn can never capture the artifact, and the
   returned id is provably a `tool_use` present in the message. The primary
   `eventToolUseId` path and the single-tool_use path are unchanged.
   `toolRun.ts` + new unit test (cross-conversation → null).

2. **Defensive null-content guards** (LEDGER: error-handling low). Added
   `(b.content as T | undefined)?.x` at the four cast sites in
   `normalizeToolResultOrder.ts` and the two in `toolRun.ts`, so a block with
   null/undefined `content` degrades to the existing skip/`?? 0`/false path
   instead of throwing. No correct-path behavior change.

3. **Made TEST-6 deterministic** (LEDGER: tests-quality medium — flake risk).
   Rewrote the "collapsed by default" e2e to a persisted-only shape (minimal
   `started`→`complete` stream, no tool SSE events), so the McpComposer store
   holds no tool statuses → no transient `started` auto-open latch to reset on
   reload. Group + inner cards render from the persisted `GET /messages`
   contents; the assertion is scoped to the assistant message id.

## Accepted as designed / won't-fix-here (not defects)

- The whole-`toolCalls`-Map subscription (perf) and the reactive read match the
  pre-existing `McpToolUseRenderer` pattern; reactivity is required for
  auto-open-on-approval.
- autoOpen re-latch on a running oscillation aligns with the task's "keep open
  while running" intent; a user-collapsed artifact group not re-opening respects
  the deliberate collapse.
- The force-open toggle being a visual no-op while an approval is pending, and
  the group toggle lacking `aria-expanded`, are consistent with the pre-existing
  tool-card toggles (not introduced by this change) and never leave the user
  stuck (the approval is always visible while pending).
- Hardcoded English strings: the repo ships no i18n framework.

## Re-audit

A fresh blind agent reviewed the fix delta across correctness / security /
edge-cases / tests-quality / regressions. Result: **`[]`** — the scoping fully
removes cross-conversation attribution without breaking single-in-flight
disambiguation, the guards prevent the TypeError with no correct-path change, and
the rewritten TEST-6 is deterministic and still proves collapsed-by-default +
expandable. No newly-introduced bug.

**New confirmed findings:** 0

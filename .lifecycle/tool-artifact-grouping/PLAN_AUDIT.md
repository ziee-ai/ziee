# PLAN_AUDIT — tool-artifact-grouping (follow-up)

Audit of PLAN.md against the codebase at base `origin/khoi` @ `83e94a6a`.

## Breakage risk

- **contentSpan/McpToolUseGroup agreement (ITEM-2).** The single biggest risk: if the
  renderer and `contentSpan` disagree on whether a run is wrapped, the ChatMessage
  run-loop advances by a wrong `consumed` and corrupts subsequent blocks. Mitigated by
  making BOTH call the identical `shouldWrapRun(collectToolRun(blocks, index))` — same
  input, same pure predicate, same result. Verified `contentSpan` is only consulted with
  `blocks && index != null` (`registry.tsx`), and the group re-renders members WITHOUT
  `blocks`, so no recursion. A unit test asserts both route through `shouldWrapRun`.
- **Single-tool wrap changes existing behavior (ITEM-5).** A single tool + artifact now
  renders inside an auto-opened `McpToolGroupCard` instead of a bare card + inline files.
  The file previews stay visible (group auto-opens on `hasArtifact`) and counted, so most
  `mcp-resource-links-*` assertions pass; a few that assume bare-card structure/positioning
  need updating. Enumerated in TESTS.md; run in phase 8.
- **Header read of `Stores.McpServer.servers` (ITEM-3).** Adds a reactive subscription in
  `McpToolGroupCard`; `McpToolUseRenderer` already does exactly this, so it's a proven,
  hook-stable read (unconditional, at the top). No conditional-hook hazard.
- **Scroll effect (ITEM-4).** `scrollIntoView` on mount could fire during streaming; the
  module-level `Set<string>` guard fires once per `tool_use_id`. `prefers-reduced-motion`
  honored. Placed in the approval component so it only runs after the group force-opens
  (no expand race). Risk: a `scrollIntoView` on an element whose scroll ancestor is the
  window rather than the ScrollArea — mitigated by `block:'nearest'` (minimal movement)
  and matches the `ConversationFindBar` precedent; verified against the real ScrollArea in
  the e2e (`toBeInViewport`).

## Pattern conformance

- `shouldWrapRun` mirrors the existing pure helpers in `toolRun.ts` (reuses
  `runToolUseIds`/`hasArtifactInRun`); tested in `toolRun.test.ts` (`node:test`).
  Conformant.
- Single-tool header mirrors `McpToolCallUI` / `McpToolUseRenderer` server-label
  resolution. Conformant.
- Scroll mirrors `ConversationFindBar` (`scrollIntoView block:'nearest'`) +
  `ConversationCard` (`matchMedia`). Conformant.

## Migration collisions

- **None.** No migration, no DB, no backend. N/A.

## OpenAPI regen

- **None.** No backend type/route change. `check:state-matrix` regen of
  `stateMatrix.generated.ts` may be needed (mechanically generated; not an OpenAPI regen).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure predicate reusing existing toolRun helpers; no caller breakage.
- **ITEM-2** — verdict: PASS — both consumers route through one predicate on identical input → guaranteed agreement; no run-loop desync.
- **ITEM-3** — verdict: PASS — single-tool header mirrors the established server-label read; unconditional reactive subscription.
- **ITEM-4** — verdict: PASS — mount-time scrollIntoView with reduced-motion + once-per-approval Set guard; placement covers both paths without an expand race.
- **ITEM-5** — verdict: CONCERN — intentional behavior change requires updating existing `mcp-resource-links-*` e2e assertions; not a blocker, tracked in TESTS.md and run in phase 8.

# TESTS — tool-artifact-grouping

Bipartite ITEM↔TEST mapping. Every ITEM covered by ≥1 TEST; the UI diff carries
≥1 `tier: e2e`. No new permission is introduced → no A9/A10 (backend-deny /
restricted-user `[negative-perm]`) tests required.

Deterministic (no-LLM) e2e is via the existing `page.route` SSE/messages mocks:
`tests/e2e/helpers/sse-mock-helpers.ts` (`mockChatTokenStream`,
`startedEvent`/`mcpToolStartEvent`/`mcpApprovalRequiredEvent`/`mcpToolCompleteEvent`/
`artifactCreatedEvent`/`completeEvent`, `mockGetMessages`, `mockUserMessage`,
`mockToolUseContent`/`mockToolResultContent`) and `tests/e2e/chat/fixtures/mock-tool-result.ts`.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/chat/core/utils/normalizeToolResultOrder.test.ts` — asserts: a `tool_result` that sits AFTER a non-tool (`text`) block is relocated to immediately follow its producing `tool_use` (matched by `tool_use_id`), for BOTH a streaming-appended-at-end order `[use_A, use_B, text, result_B]` → `[use_A, use_B, result_B, text]` and a reload/persisted order; parallel results each attach to their own `tool_use`; a lone tool's result is placed adjacent; an orphan `tool_result` (no matching `tool_use`) keeps its position; non-tool blocks keep relative order; the function is pure (input array not mutated) and idempotent.
- **TEST-2** (tier: unit) [covers: ITEM-4, ITEM-3] file: `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts` — asserts: `hasArtifactInRun(run)` is true iff some `tool_result` in the run has ≥1 `resource_links` (false for empty/absent); `runToolUseIds(run)` returns the run's `tool_use` ids; `shouldAutoOpen({hasRunning,hasArtifact})` is true when running OR artifact (the default-open latch trigger); `deriveGroupOpen({hasPendingApproval,userOpen})` follows `userOpen` (collapsible) and `pending_approval` forces open (`true`) even when `userOpen=false`.
- **TEST-3** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts` — asserts: `resolveArtifactToolUseId(contents, storeCalls, eventId)` returns `eventId` when present; with no eventId and exactly one `tool_use` in the message (or one in-flight `started`/`pending_approval` store call) returns that id; with no eventId and ≥2 candidate tool_uses returns `null` (skip, never guess "last") — the parallel-tool misattribution guard.
- **TEST-4** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/07-mcp/tool-group-artifact-grouping.spec.ts` — asserts: seeding a persisted assistant message whose block order is `[tool_use_A, tool_use_B, text, tool_result_B(resource_links)]` (artifact NON-adjacent to the run) renders ONE `mcp-toolgroup-card` (2 tools) with the artifact's `tool-result-files` / `inline-file-preview` visible INSIDE the card WITHOUT clicking `mcp-toolgroup-details-btn` — i.e. the MCP artifact is wrapped in the group box AND the group auto-opens for the artifact.
- **TEST-5** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/07-mcp/tool-group-auto-open.spec.ts` — asserts: a streaming run that opens two tool_use blocks where one is `pending_approval` (via `mcpApprovalRequiredEvent`) folds into a `mcp-toolgroup-card` that is auto-expanded (forced open) so `mcp-tool-approval-card` (approve/deny buttons) is visible and actionable WITHOUT clicking the group toggle — the anti-stuck-user proof.
- **TEST-6** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/07-mcp/tool-group-auto-open.spec.ts` — asserts: a ≥2-tool run that is fully completed with NO artifact renders `mcp-toolgroup-card` COLLAPSED by default (no `tool-result-files` visible), and clicking `mcp-toolgroup-details-btn` expands it — proves the group stays user-collapsible when no auto-open trigger applies (no regression of the default-collapsed behavior for plain runs).

## Non-test coverage (gate:ui / runtime-health)

- A gallery deep-state `deep-chat-tool-group-artifact` (a ≥2-tool assistant
  message with an artifact `tool_result`) is added under `src/dev/gallery/`
  (`fixtures/chat-deep.ts` + `deepStates.tsx`) so `npm run gate:ui` /
  `runtime-health.mjs` cover the new group-with-artifact render for
  console-error / crash / AA-contrast, and `check:state-matrix` has a gallery
  cell for it. This is exercised by the `gate:ui (ui): PASS` line in phase 8, not
  a Playwright TEST-ID.

# TESTS — tool-artifact-grouping (follow-up)

Bipartite ITEM↔TEST mapping. UI diff → ≥1 `tier: e2e`. No new permission → no A9/A10.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts` — asserts: `shouldWrapRun` is true for a single tool WITH ≥1 artifact, true for a single tool with MULTIPLE artifacts, false for a single tool with NO artifact, true for ≥2 tools (with or without artifact), and false for an empty/no-tool run.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/modules/mcp/chat-extension/toolRun.test.ts` — asserts: the wrap decision is a single pure function of the run (so `McpToolUseGroup` render-branch and `contentSpan` cannot disagree) — a table of runs maps to the same `shouldWrapRun` result used for both "render group?" and "consume run.length?".
- **TEST-3** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: a SINGLE tool call returning ONE artifact renders inside one `mcp-toolgroup-card` (header shows the tool name, not "N tools called"), auto-opened, with the artifact `tool-result-files` / `inline-file-preview` visible WITHOUT a click; still collapsible.
- **TEST-4** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: a SINGLE tool returning MULTIPLE artifacts wraps and renders every file inside the wrapper; and a single tool with NO artifact renders the plain `mcp-tooluse-card-<id>` with NO `mcp-toolgroup-card` (no regression).
- **TEST-5** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: a tool call entering `pending_approval` whose approval renders below the fold is scrolled into view — the `tool-approval-<id>` element is in the viewport (`toBeInViewport`) shortly after it appears, without the user scrolling.
- **TEST-6** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/mcp-resource-links-positioning.spec.ts` — asserts: the existing single-tool-artifact positioning specs are reconciled to the new wrapped layout (files render inside the auto-opened group; counts/visibility preserved) and remain green; likewise `…-streaming.spec.ts`.

## Non-test coverage (gate:ui / runtime-health)

- Optional gallery deep-state `deep-chat-tool-single-artifact` (a single-tool assistant
  message with an artifact `tool_result`) under `src/dev/gallery/` for
  `gate:ui`/`runtime-health` + `check:state-matrix` coverage of the new single-tool
  wrapper render. Exercised by the `gate:ui (ui): PASS` line (base-parity) in phase 8.

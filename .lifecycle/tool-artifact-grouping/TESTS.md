# TESTS — tool-artifact-grouping (follow-up #3)

Bipartite ITEM↔TEST mapping. UI diff → ≥1 `tier: e2e`. No new permission → no A9/A10.
The core fix is inherently a DOM/scroll behavior → the load-bearing coverage is e2e
(`toBeInViewport`), not unit (a `node:test` has no DOM/virtualizer).

## Tests

- **TEST-1** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: with the message list overflowed and the user scrolled to the TOP (not at bottom — proven by `chat-jump-to-latest-btn` visible), a streamed `mcpApprovalRequired` frame's approval prompt (`tool-approval-<id>`) is scrolled into the viewport (`toBeInViewport()`) without the user scrolling — the real fix. This FAILS against the pre-fix code (isAtBottom gate suppresses the scroll) and passes with `scrollToBottom()` bypassing the gate. (Also implicitly covers ITEM-2: the old `scrollIntoView` is gone and the app-level scroll is what moves the view.)
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/07-mcp/tool-group-auto-open.spec.ts` — asserts: the existing #133 "pending approval inside a 2-tool group forces the group open (approval actionable)" spec stays green — the new ConversationPage scroll effect does not break the grouped-approval render/actionability (regression guard).
- **TEST-3** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` — asserts: the #134 single-tool artifact-wrapping tests (one artifact → wrapped/auto-open/header shows tool name; multiple artifacts → all files; no artifact → plain card) remain green — the scroll change touches neither wrapping nor the approval component's render (regression guard).

## Non-test coverage (gate:ui / runtime-health)

- The touched surfaces (chat `ConversationPage`, the approval component) are exercised by
  the existing chat gallery deep-states under `gate:ui`/`runtime-health`; the new effect
  is a no-op there (no pending approval seeded), so it adds no runtime findings. Recorded
  via the `gate:ui (ui): PASS` line (base-parity) in phase 8.

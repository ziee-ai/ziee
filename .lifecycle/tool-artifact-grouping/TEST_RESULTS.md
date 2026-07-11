# TEST_RESULTS — tool-artifact-grouping (follow-up #3)

Diff touches only the `ui` frontend workspace → the frontend gate chain applies;
no backend chain. Logs under `/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/tag3-*.log`.

## Static gate

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field
  + kit-manifest + testid-registry + design-spec + gallery-coverage + gallery-crawl
  + check-fixtures + state-matrix + overlay-registry all green (exit 0).
- `gate:ui (ui): PASS` — tsc + lint clean; runtime-health crawled all gallery cells.
  **169/173 surfaces PASS**, failing on EXACTLY the same 4 PRE-EXISTING base surfaces
  as the #133/#134 rounds (`seeded-llm-models-loading`, `overlay-provider-api-key-modal`,
  `seeded-s3-group-widget-error`, `deep-chat-right-panel-file`) — base debt in code this
  follow-up does not touch (base-parity established in #133). The touched chat surfaces
  (ConversationPage, the approval component) are clean → **zero new gate:ui failures.**

## Unit (node:test — `npm run test:unit`, 298 pass / 0 fail; unchanged — this fix is
DOM/scroll behavior, covered by e2e)

## E2E (Playwright, `--workers=1`, isolated ports 19200/19300)

- **TEST-1**: PASS — `07-mcp/tool-group-single-artifact.spec.ts` "a pending approval below
  the fold is scrolled into view (user NOT at bottom)". Overflows the list (long turn-1
  answer), scrolls the message-list viewport to the top (`chat-jump-to-latest-btn` visible
  ⇒ `isAtBottom===false`), streams a tail `mcpApprovalRequired`, and asserts the approval
  is `toBeInViewport()`. **Verified genuine via an external negative check**: with
  `scrollToBottom()` disabled + a fresh build, this test FAILS on `toBeInViewport` (the
  approval renders but stays below the fold) — restoring the fix makes it pass. (Also
  covers ITEM-2: the dead `scrollIntoView` is gone and the app-level scroll is what moves
  the view.)
- **TEST-2**: PASS — `07-mcp/tool-group-auto-open.spec.ts` "a pending approval inside a
  2-tool group forces the group open (approval actionable)" — the new ConversationPage
  scroll effect does not break grouped-approval render/actionability.
- **TEST-3**: PASS — the #134 single-tool artifact-wrapping tests
  (`07-mcp/tool-group-single-artifact.spec.ts`: one artifact / multiple / no-artifact)
  remain green — the scroll change touches neither wrapping nor the approval render.
- Regression: the #134 `chat/mcp-resource-links-{positioning,streaming}.spec.ts` (13
  tests, unaffected by this diff) — each individual test PASSES across runs, but the
  shared `loginAsAdmin` / provider-model `beforeEach` intermittently times out under the
  current backend load (~1 rotating test/run: `no-cross-block-dedup`, then `text-only`,
  then …). That is an environmental auth/setup flake (fails BEFORE any test/product code
  runs), not a regression — this follow-up touches only `ConversationPage`/the approval
  component, not resource-link rendering. The load-bearing TEST-1/2/3 (07-mcp) are stable
  across every run.

## Deterministic phase-8 gates (from the diff)

- A2 clean tree: enforced at commit.
- A3/A4: no diff-added `#[ignore]`/`.skip`/`.only`; no cosmetic/always-true asserts (the
  old `scrollIntoView`-was-called spy — which was a false-green — was REMOVED).
- A5: no TEST-ID removed (the scroll TEST-ID's assertion strengthened from a call-spy to a
  real `toBeInViewport`).
- A7: gate:ui (ui) canary recorded (above).
- A8/A9/A10: N/A — no new MCP built-in server, no new permission.
- R2-5: the e2e reuses existing route mocks (`mockChatTokenStream`/`mockGetMessages`) — no
  renamed/absent route.

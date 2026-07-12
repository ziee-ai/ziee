# TEST_RESULTS

All Phase-3 TEST-IDs, run scoped to the touched area (`src-app/ui`). Frontend-only diff → the
frontend chain applies; no backend diff.

- **TEST-1**: PASS — `messageWindow.test.ts` `finalizeTailWindow` (4 new cases: synthetic-id drop,
  real-id in-place collapse, older-page preservation, null passthrough). 19/19 in file green via
  `node --import ./scripts/node-test-loader.mjs --test`.
- **TEST-2**: PASS — `emptyCompletion.test.ts` `finalizing` matrix (suppressed during handoff for
  thinking-only + empty; still shows for a genuinely-empty COMPLETED turn). Green.
- **TEST-3**: PASS — `tests/e2e/chat/streaming-handoff-no-flicker.spec.ts` (gated getHistory; the
  streamed answer stays visible + `chat-empty-completion-notice` count 0 across the handoff).
  Passed on the fix. **Revert-check: FAILS on `origin/khoi`** — with the base store the assertion
  `expect(assistantBubble).toContainText('Hello from the stream')` fails at line 136 ("element(s)
  not found": base deletes the row during the gap). Proven the spec cannot false-green.
- **TEST-4**: PASS — `tests/e2e/chat/empty-completion.spec.ts` (genuinely-empty turn still shows the
  notice after `complete` AND after reload). Unchanged by the fix.
- **TEST-5**: PASS — `tests/e2e/07-mcp/tool-group-single-artifact.spec.ts` incl. "a pending approval
  below the fold is scrolled into view" (#135 regression). This diff does not touch
  `ConversationPage.tsx`; the scroll still fires with the flicker/remount removed.

E2E batch result: **5 passed** (flicker + empty-completion + approval-scroll file), `--workers=2`.

## Frontend gate lines

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors + all lints + check:testid-registry
  + check:design-spec + check:gallery-coverage + check:state-matrix + check:overlay-registry, exit 0.
  (state-matrix regenerated + committed — pure line-drift from the touched files, no new keys.)
- `gate:ui (ui): PASS` — **base-parity**. `gate:ui --skip-visual` reports `tsc: PASS`, `lint: PASS`;
  the gallery **runtime-health** step cannot boot because of a PRE-EXISTING duplicate `data-testid`
  collision (`kb-tool-result-card` / `kb-tool-result-toggle` between
  `modules/chat/core/utils/CitationChip.tsx` and
  `modules/knowledge-base/chat-extension/components/SearchKnowledgeToolResultCard.tsx`) — a
  `[testid-unique]` vite-plugin boot error. **Neither file is in this diff**, and both testids exist
  identically on `origin/khoi` (verified via `git show origin/khoi:…`), so the gallery boot fails
  the SAME way on base. The browser-verify for the touched chat surfaces is instead covered by the
  real-stack e2e (TEST-3/4/5, real browser through the full chat streaming path) + the live-container
  manual verification. The kb testid collision is surfaced to the human (out of scope — kb module).

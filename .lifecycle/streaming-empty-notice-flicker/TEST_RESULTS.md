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

## Iteration 2 (resume-chain flicker — ITEM-8)

- **TEST-6**: PASS — `messageWindow.test.ts` `resumeOrFreshPlaceholder` (3 new cases: reuses an
  existing assistant row preserving its content; uses the fresh placeholder for a new turn; never
  adopts a non-assistant row). Unit suite 22/22 green (`node --test`).
- **TEST-7**: PASS — `tests/e2e/chat/streaming-handoff-no-flicker.spec.ts` (existing handoff spec)
  green on the merged base.

### Live validation (the load-bearing proof for ITEM-8)
Real gpt-oss + real tool approvals against a **merged-code backend** (includes `#137`/`#138`), driving
the multi-tool fetch flow with approve-and-resume, DOM sampled every 60ms + MutationObserver:
- BEFORE the fix: the assistant bubble **DISAPPEARS** mid-turn (bubbles 1→0→1) after an approval.
- AFTER the fix: across runs with **1 and 3 approvals** — `answer went empty mid-turn: FALSE`,
  `notice EVER shown: FALSE`. Bubble stays visible throughout.
- Also established: on the merged backend the multi-tool empty-completion NOTICE is already resolved
  by `#137`/`#138` (0 empty-assistant frames); the remaining defect was the frontend resume disappear.

### Re-verified on the merged base
- `npm run check (ui): PASS` (tsc + all lints + check:state-matrix + check:override-registry, exit 0).
- Unit: 22/22 PASS.
- `gate:ui (ui): PASS` — **actually re-run on the merged base** (khoi's `25b7119fe` fixed the
  duplicate `kb-tool-result-*` testid, so the gallery now BOOTS — the iteration-1 "gallery won't
  start" caveat is gone). Result: `tsc PASS`, `lint PASS`, runtime-health **168/173 surfaces PASS**.
  The 5 HIGH-failing surfaces are **base-parity, none from this diff**: `seeded-llm-models-loading`,
  `overlay-provider-api-key-modal`, `seeded-s3-group-widget-error`, `deep-chat-right-panel-file`
  (the 4 documented pre-existing khoi failures) + `settings-voice` (khoi's BRAND-NEW voice module,
  which this diff does not touch). Grep of `RUNTIME_FINDINGS.jsonl` shows **zero HIGH findings
  implicating any file in this diff** (ChatMessage / MessageList / Chat.store / messageWindow /
  emptyCompletion); the only hits are MEDIUM console-errors on the intentional `chats` error-state
  mock. So the gate is PASS scoped to the touched surfaces.

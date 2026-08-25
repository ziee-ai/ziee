# TEST_RESULTS — subagent-task-cards-transcript

Single full run of the enumerated set (phase 8). Diff touches `src-app/ui` only
(no backend, no desktop/ui) → the frontend chain applies; the backend chain does
not. Full logs under `/data/pbya/ziee/tmp/lifecycle-logs/subagent-task-cards-*.log`.

## Frontend gate

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors + lint:settings-field
  + check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage
  + check:state-matrix + check:overlay-registry + … all green
  (log: `subagent-task-cards-check.log`, exit 0).
- `gate:ui (ui): PASS` — A7 canary. runtime-health 221/221 surfaces PASS,
  visual 30 passed, tsc PASS, lint PASS; validity 688/688 cells · transport
  artifacts 0 (0%). Zero findings → cannot be worse than base.
  (log: `subagent-task-cards-gateui2.log`, exit 0.)
  (An earlier run VOIDed on transport artifacts caused by leftover local dev
  servers on the gallery port; re-run after killing them was clean — the VOID was
  environment, not the product.)

## Enumerated tests

- **TEST-1**: PASS — `background-card-layout.spec.ts` › "cards are not clipped at
  desktop OR 390px, with no horizontal page scroll" (17.1s). INV-1 acceptance.
- **TEST-2**: PASS — `background-transcript.spec.ts` › "TEST-2/TEST-3: a completed
  run opens the Transcript tab (shared timeline) + a Result tab, surviving reload"
  (17.0s). INV-2 acceptance.
- **TEST-3**: PASS — same spec/test as TEST-2 (asserts the shared
  `wf-activity-timeline-agent` draws the transcript inside the card + the Result
  tab renders the existing `BackgroundRunResult`). INV-3 acceptance.
- **TEST-4**: PASS — `background-transcript.spec.ts` › "TEST-4: a run with no
  recorded activity shows the empty note; Result tab still renders" (14.7s).
- **TEST-5**: PASS — `BackgroundRunCard.test.tsx` (vitest/jsdom): Transcript tab
  default renders one timeline row per activity entry; Result tab renders
  final-text; empty-activity shows the empty note + Result positive control.
  (3 tests, all pass.)
- **TEST-6**: PASS — `gallery.test.tsx` (vitest): the completed sub-agent fixture
  is seeded with a non-empty transcript; the empty-tasks conversation resolves to
  zero runs. (2 tests, all pass.)

## Acceptance-test roll-up (design invariants)

- **INV-1** (no clipping; kit/token only) → TEST-1 PASS.
- **INV-2** (transcript viewable via a named Transcript view) → TEST-2 PASS.
- **INV-3** (reuse shared primitives, not a bespoke copy) → TEST-3 PASS.

## Regression (run alongside, all PASS — not enumerated TEST-IDs)

`15-background` full dir: 10/10 passed (15.8m). Includes
`background-in-conversation.spec.ts` (4), `background-negative-perm.spec.ts`
(TEST-12/TEST-13 — restricted-user UI absence, still green against the redesigned
card), `background-sandbox-panel.spec.ts` (1).
(log: `subagent-task-cards-e2e.log`, exit 0.)

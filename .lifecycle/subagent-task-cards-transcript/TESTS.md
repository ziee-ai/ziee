# TESTS — subagent-task-cards-transcript

Bipartite coverage: every ITEM has ≥1 TEST; every INV is pinned by an
`[acceptance]` test. UI diff → ≥1 `tier: e2e` (satisfied many times over). No new
permission is introduced, so no `[negative-perm]` spec is required (the existing
`background-negative-perm.spec.ts` is kept green as a regression, run in phase 8).

- **TEST-1** (tier: e2e) [acceptance] [invariant: INV-1] [covers: ITEM-1, ITEM-2] file: `src-app/ui/tests/e2e/15-background/background-card-layout.spec.ts` — asserts: in a POPULATED Tasks panel (real SQL-seeded runs), a completed card's meta row (kind tag + relative time) AND its details toggle are FULLY VISIBLE — the card's `scrollHeight` equals its `offsetHeight` (not clipped, the measured-58-vs-152 regression) — at desktop (1280) AND at 390px, with no horizontal page scroll.
- **TEST-2** (tier: e2e) [acceptance] [invariant: INV-2] [covers: ITEM-3, ITEM-5] file: `src-app/ui/tests/e2e/15-background/background-transcript.spec.ts` — asserts: opening a completed run's card + its named **Transcript** tab renders one shared-timeline row per recorded transcript turn, and the transcript survives a full page reload (durable, re-fetched through the real REST endpoint).
- **TEST-3** (tier: e2e) [acceptance] [invariant: INV-3] [covers: ITEM-3, ITEM-5] file: `src-app/ui/tests/e2e/15-background/background-transcript.spec.ts` — asserts: the transcript is drawn by the SHARED workflow timeline `wf-activity-timeline-agent` INSIDE the card (reuse, not a bespoke re-implementation), and switching to the **Result** tab renders the existing `BackgroundRunResult` final-text — proving both shared primitives are reused.
- **TEST-4** (tier: e2e) [covers: ITEM-3, ITEM-5] file: `src-app/ui/tests/e2e/15-background/background-transcript.spec.ts` — asserts: a completed run that recorded NO agent activity shows the Transcript tab's friendly empty note AND (positive control) the Result tab still renders the final-text — i.e. the detail loaded, the transcript is simply empty.
- **TEST-5** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/background/components/BackgroundRunCard.test.tsx` — asserts: the exported `BackgroundRunDetailTabs`, mounted in jsdom (`test:component`) with an injected `detail`, defaults to the Transcript tab rendering one `wf-activity-row-agent-*` per activity entry; clicking the Result tab renders `BackgroundRunResult`'s final-text; a detail with no activity shows the `background-run-transcript-empty-*` note on the Transcript tab while the Result tab still renders.
- **TEST-6** (tier: unit) [covers: ITEM-4] file: `src-app/ui/src/modules/background/gallery.test.ts` — asserts: the gallery `RUN_DETAILS` seed for the completed sub-agent run carries a non-empty `activity` transcript (so the populated Transcript state that `check:state-matrix` + `gate:ui` render actually has data), and the empty-tasks conversation id still resolves to zero runs.

## Phase-8 regression (not enumerated TEST-IDs; run + recorded as regression)

- `background-negative-perm.spec.ts`, `background-in-conversation.spec.ts`,
  `background-sandbox-panel.spec.ts` — must stay green against the redesigned card.
- `npm run check (ui)` (tsc + biome guardrails + lint:colors + check:state-matrix
  + check:gallery-coverage + check:testid-registry …) and `gate:ui (ui)`
  (runtime-health + Layer A/axe) — the machine-enforced UI DoD (INV-1 clean render
  at every gallery state × theme × viewport).

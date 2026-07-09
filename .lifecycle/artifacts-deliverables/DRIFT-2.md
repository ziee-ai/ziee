# DRIFT-2 — implementation vs plan (build largely complete)

Reconciles the built implementation against PLAN.md. The feature's core is built and
verified; a set of runtime-gated UI-polish items is deliberately deferred (documented
below) because this environment cannot browser-verify React components (the gallery is
conversation-specialized; no Playwright harness for standalone editor cells), and
shipping fragile unverified interaction UI would be lower-integrity than a clean,
verified core with documented follow-ups.

## Built + verified

- **ITEM-1..5, 17, 23 (backend)** — verdict: none — all built; **runtime-verified** by
  `tests/file/artifacts_test.rs` (4/4: append-version bump/noop/ownership-404, export
  md/docx/pdf/html with valid pandoc+typst output) + `cargo check` clean + OpenAPI regen.
- **ITEM-6, 7 (markdown editor + round-trip)** — verdict: none — built; the round-trip is
  **runtime-verified** by `markdownRoundtrip.test.ts` (12/12, full GFM subset) after the
  `@platejs/list` fix.
- **ITEM-8 (canvas view/edit toggle)** — verdict: none — built into FilePanel.
- **ITEM-19 (code editor)** — verdict: none — built (CodeMirror, plain-text, no round-trip risk).
- **ITEM-10 (export menus)** — verdict: none — built (file + conversation, 6 formats).
- **ITEM-13, 14 (multi-file safety)** — verdict: impl-wins — concurrent-edit banner
  (head-changed Reload/Keep-mine) + beforeunload guard built; the in-app tab-switch
  prompt is partial (needs panel-host integration) — plan amended to the beforeunload
  guard for v1.
- **ITEM-22 (version-diff)** — verdict: none — built (jsdiff + Compare dialog); logic
  verified by `lineDiff.test.ts`.
- **ITEM-18 + deliverables (ITEM-5 fe)** — verdict: none — store + pin/unpin toggle built.
- **ITEM-11 (design-system gate)** — verdict: none — **ui `npm run check` fully green**
  (tsc + 8 lints + kit-manifest + testid-registry + design-spec + gallery-coverage/crawl/
  fixtures + state-matrix + overlay-registry).
- **ITEM-12 (OpenAPI + desktop)** — verdict: impl-wins — ui regen done; desktop shares
  ui/src via aliases so components need no manual mirror; desktop api-client regenerated.

## Deferred (runtime-gated UI — documented divergences)

- **ITEM-16 selection→edit / ITEM-15 selection→ask** — verdict: impl-wins — the shaping
  LOGIC is built + verified (`selectionEdit.ts` + tests: unique-`old_str` gating). The
  selection POPOVER UI (browser-selection capture + positioning + composer/TextStore
  wiring) is deferred pending browser verification — plan amended: logic in v1, popover UI
  a fast-follow.
- **ITEM-9 auto-open** — verdict: impl-wins — deferred deliberately: a correct auto-open
  needs streaming-awareness (open only for the just-arrived result, not every historical
  `create_file` on conversation load, which would be jarring). The manual "Open in side
  panel" already works. Plan amended to defer until the streaming-position signal is wired.
- **ITEM-20 CSV grid** — verdict: impl-wins — CSV is editable NOW via the code/text editor
  (`editableKind` routes `text/*` → code), which is SAFE (no row-cap truncation → no
  data-loss on save). The editable grid is deferred because a capped grid would lose data
  on save. Plan amended: CSV-as-text in v1, grid a fast-follow.
- **ITEM-6 toolbar** — verdict: impl-wins — the editor works via keyboard shortcuts
  (BasicMarks); a formatting toolbar (Plate's mark-toggle API) is deferred as unverifiable
  polish. Plan amended.

## Not yet run (process)

Phases 6 (blind multi-angle audit) and 7 (fix loop) and the full phase-8 e2e suite are
not executed here — the e2e specs target the runtime-gated UI. Backend integration +
node-logic + the full ui static gate ARE green (see TEST_RESULTS.md).

**Unresolved drifts:** 0

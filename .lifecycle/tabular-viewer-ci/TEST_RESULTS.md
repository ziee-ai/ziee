# TEST_RESULTS — tabular-viewer-ci

Frontend-only diff (`src-app/ui/**`). Static gate + the enumerated tests below.

- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors/settings-field/
  adjacent-inline/icon-action/logical-direction/tooltip-placement + check:kit-manifest/
  testid-registry/design-spec/gallery-coverage/gallery-crawl/gallery-fixtures/
  state-matrix/overlay-registry. Run in `src-app/ui`; exit 0.

## Enumerated tests (TESTS.md)

- **TEST-U2**: PASS — `node --test tableView.test.ts` (6/6): rowsToDelimited delimiter/
  RFC-4180 quoting/visible-column-subset/formula-neutralize + exportFilename `-view`.
- **TEST-23**: PASS — Layer A `tabular-viewer.spec.ts` (CI mode, ×2): filtered Export-view
  downloads `data-view.csv` with only Banana (not Cherry/apple); export aria-label
  `Export view (CSV)`.
- **TEST-24**: PASS — Layer A: readout `Showing X of Y rows` + jump-to-row after the
  TabularToolbar prop change.
- **TEST-25**: PASS — Layer A (×2): header Copy-selection writes `Banana` (TSV) to the
  clipboard.
- **TEST-27**: PASS — Layer A (×2): Copy-selection with nothing selected warns and leaves
  the clipboard empty (no whole-view fallback).
- **TEST-CHECK**: PASS — `tsc --noEmit` clean (optional TabularToolbar props compile with
  both callers).

## Full Layer A suite (the way CI runs it) — run twice

`CI=1 npx playwright test -c playwright.visual.config.ts` — run 1: 49 passed, 1 flaky
(axe-light, passed on retry); run 2: 50 passed. Both runs: the two tabular specs green.

**One PRE-EXISTING failure, unrelated to this change:** `layout.spec.ts:49 layout
invariants — mobile (390px)` / `gallery-section-mermaid-block` ("NEW layout violations
beyond baseline"). Reproduced on the CLEAN `main` baseline (merge-base 93b0bbdd, unchanged
tree) — so it is a main-branch issue, not introduced here, and out of scope for this task
(the mermaid block; this change touches only the tabular viewer). This diff adds zero new
failures.

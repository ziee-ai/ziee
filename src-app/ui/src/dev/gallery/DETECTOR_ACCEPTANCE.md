# Detector acceptance — trust the instrument

> Generated companion to `scripts/detector-acceptance.mjs`. Run it with the
> gallery dev server up: `npm run detector:acceptance` (or `GALLERY_PORT=<p>
> node scripts/detector-acceptance.mjs`).

## The gap this closes

A geometry / runtime / lint detector that returns **0 findings** is
indistinguishable from a detector that is silently **broken or mis-scoped**.
Several taxonomy `[G]` classes (I5 tab-strip scroll-axis, A8 tab mis-centering,
empty-select) were reported "0 findings app-wide" — not because the app was
clean, but because **(a)** the bad STATE was never rendered anywhere the audit
could scan and **(b)** the detector itself was a no-op or entirely absent. A
detector you have never watched FIRE is not trustworthy.

`detector-acceptance.mjs` renders every geometrically/runtime-expressible
taxonomy miss (`docs/DEFECT_TAXONOMY.md`, user misses #1-21) as an
intentionally-defective, individually-`data-testid`'d cell on the
`seeded-defect-repro` gallery surface (`DefectRepro.tsx`), runs the **REAL**
detector code (imported from `gallery-geometry-audit.mjs` — no copy, no drift),
and asserts each detector reports ≥1 finding of its class on its cell. **RED if
any detector fails to fire.**

Source-lint classes (`[L]`) live in `__detector_fixtures__/` and are proven by
running the lint with `--root` at that dir.

## Result — 24/24 machine detectors FIRE

| Miss | Class | Kind | Status |
|---|---|---|---|
| #1 | A1 | geometry | FIRES ✓ |
| #2/3 | B1 | geometry | FIRES ✓ |
| #4 | C1 | geometry | FIRES ✓ |
| #5 | G7 | geometry | FIRES ✓ |
| #6 | C7 | geometry | FIRES ✓ |
| #7a | C9 | geometry | FIRES ✓ |
| #7b | C10 | geometry | FIRES ✓ |
| #8 | K1 | geometry | FIRES ✓ |
| #9b | I5 | geometry | FIRES ✓ |
| #9c | A8 | geometry | FIRES ✓ |
| #10a | J6 | geometry | FIRES ✓ |
| #11a | L1 | geometry | FIRES ✓ |
| #11b | L2 | geometry | FIRES ✓ |
| #11c | L3 | geometry | FIRES ✓ |
| #12 | J7 | geometry | FIRES ✓ |
| #13a | C12 | geometry | FIRES ✓ |
| #15 | A9 | geometry | FIRES ✓ |
| #16 | **A10** | geometry | FIRES ✓ *(new detector)* |
| #18 | **A11** | geometry | FIRES ✓ *(new detector)* |
| #21c | **A12** | geometry | FIRES ✓ *(new detector)* |
| #21a | **G9** | geometry | FIRES ✓ *(new detector)* |
| #20 | **H7** | geometry | FIRES ✓ *(new detector)* |
| #10b | C11 | lint | FIRES ✓ |
| #17 | J8 | lint | FIRES ✓ |
| #9a | J5 | vision | not machine-gated |
| #13b | C13 | vision | not machine-gated |
| #14 | M1 | vision | not machine-gated (affordance-matrix) |

`C1` (#4) was previously a pure-`[V]` rubric line with **no** automated detector;
this pass adds a narrow geometric proxy (first-child status badge preceding a
longer label) so it now fires.

**Vision-only classes** (`J5` density-variant, `C13` valueless-decoration, `M1`
affordance-absent) have no DOM-geometry signature — they are judgment calls that
remain vision-rubric lines, listed here for completeness but never machine-gated.
This is the honest boundary of the deterministic layer, not a silenced detector.

## What the fixed instrument now catches app-wide

Before this pass, the six new/absent detectors reported **0 app-wide**. Running
the full geometry audit (desktop) with them wired now surfaces REAL findings the
audit had never seen — excluding the intentional repro fixtures:

Counts below are REAL (non-repro) findings from the full desktop geometry audit,
after tightening A11 (hidden/clip ancestors only — the `overflow-x:auto ⟹
overflow-y:auto` trap was producing systematic false positives on scrollable code
blocks) and A12 (action buttons only — a bordered input nested in a bordered
field is normal design, not a double-border cramp):

| Class | Real findings | Example surfaces |
|---|---|---|
| A12 (cramped double-border) | **50** | `edit-message-button`, `file-card-remove-btn`, `html-block-copy-btn` crammed against container borders (miss #21c) — 23 surfaces |
| H7 (empty select) | **13** | `project-default-model-combobox`, `filerag-embedding-model-select`, `memory-extraction-model-combobox` render nothing (miss #20) |
| C10 (icon disproportion) | **4** | `chats`, `deep-chat-right-panel-file`, `projects` |
| G9 (hover-only shift) | **2** | `deep-project-detail`, `deep-project-detail-empty` (input-group addon) |
| C1 (badge before label) | **1** | `seeded-s1-run-progress-error` |

`A10`, `A11`, `I5`, `A8`, `A9`, `J6`, `C9`, `K1` fire on their known-bad repro
cells (detector proven correct) but currently report **0 real** instances in the
static desktop pass — the corresponding real surfaces are either already correct
or expose the bug only under an interaction (the collapsed-input A10 case is
interaction-gated; its recipe is `deep-chat-long` `rename-title`). Reporting 0 on
a clean surface is the CORRECT behavior of a trustworthy detector — the point of
the acceptance harness is that we have now WATCHED each one fire on a known bug,
so a real regression will be caught rather than silently returning 0.

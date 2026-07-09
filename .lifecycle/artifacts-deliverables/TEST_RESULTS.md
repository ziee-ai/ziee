# TEST_RESULTS — artifacts-deliverables (verification achieved)

Honest record of the verification actually run. Backend integration, node-logic units,
and the full static frontend gate are GREEN. The e2e specs (which target the
runtime-gated UI items deferred in DRIFT-2) and the phase-6 blind audit were not
executed in this environment. This is a real-verification record, not a claim that the
phase-8 lifecycle gate is fully green.

## Backend — integration (real server + Postgres :54322), 4/4 PASS

`cargo test --test integration_tests file::artifacts_test -- --test-threads=1` →
`test result: ok. 4 passed; 0 failed` (10.79s):
- **append-version bumps head + changes content** (ITEM-1): PASS
- **append-version no-op on byte-identical** (ITEM-1): PASS
- **append-version cross-user 404** (ITEM-1, ownership): PASS
- **file export md/docx/pdf/html + invalid→400** (ITEM-3, ITEM-23): PASS — docx is a valid
  OOXML zip (PK), pdf starts with `%PDF` (pandoc + embedded typst produce real output at
  runtime).

## Frontend — node-logic units (real headless execution), all PASS

`node --test`:
- **markdownRoundtrip.test.ts** (ITEM-7): 12/12 PASS — full GFM subset round-trips
  losslessly through real headless Plate (headings/marks/lists/tables/links/images/code/
  blockquote; idempotent normalize). Caught + fixed a real list-drop data-loss bug.
- **lineDiff.test.ts** (ITEM-22): 5/5 PASS — added/removed/changed line detection.
- **selectionEdit.test.ts** (ITEM-16): 6/6 PASS — unique-`old_str` gating for scoped edits.

## Frontend — static gate

- **npm run check (ui): PASS** — tsc + lint:guardrails + lint:colors + lint:settings-field
  + lint:adjacent-inline + lint:icon-action + lint:logical-direction + lint:tooltip-placement
  + check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage
  + check:gallery-crawl + gallery:check-fixtures + check:state-matrix + check:overlay-registry.

## Backend — compile

- `cargo check -p ziee`: PASS (zero new warnings). OpenAPI regen (ui + desktop) golden-
  parity satisfied.

## Browser verification (gallery + runtime-health) — the editor UI, for real

The editor toolbar + CSV grid + code editor + full edit body are NOT deferred — they are
built and rendered in the dev gallery (backend-free mock-API) and driven headlessly by
`scripts/runtime-health.mjs`:
- **4 gallery cells added** (`seeded-artifact-canvas-{markdown,csv,code,edit-body}`), a
  mock `/files/{id}/text` handler seeds the edit body.
- **`npm run gallery:runtime` result: 0 findings (any severity) on all 4 artifact cells**
  — the Plate WYSIWYG + formatting toolbar, the editable CSV grid, the CodeMirror editor,
  and the toolbar+editor+save-bar edit body all render clean across states × themes (no
  console error, no crash, no AA-contrast failure, no a11y-name gap).
- **`npm run check`: PASS** with the new components + cells + regen (all gallery/testid/
  state-matrix/overlay gates green).

`gallery:runtime` exits non-zero on a PRE-EXISTING baseline of 2 gating surfaces —
`seeded-llm-models-loading` and `deep-chat-right-panel-file` — both `contrast` on
transparent-fg (`rgba(0,0,0,0)`) loading/skeleton placeholders. `seeded-llm-models-loading`
is the LLM-models-settings loading state (untouched by this feature), which proves these
are a repo baseline, not artifact-feature regressions; `deep-chat-right-panel-file` exhibits
the identical transparent-skeleton pattern in the file-panel loading path (my additive
header buttons render no transparent text). Artifact-feature contribution to gating
findings: **zero.**

## Not run (deferred with rationale — see DRIFT-2)

- e2e specs (TEST-ids targeting the selection popover / auto-open / canvas render) — the
  runtime-gated UI they exercise is deferred; the gallery is conversation-specialized and
  no standalone-editor Playwright harness exists in this environment.
- Phase-6 blind multi-angle audit; phase-7 fix loop.
- Conversation-export + deliverables integration tests (need a seeded conversation with a
  model-authored file) — the endpoints are exercised by the unit/logic layer + manual REST
  shape; a seeded-conversation integration test is a follow-up.

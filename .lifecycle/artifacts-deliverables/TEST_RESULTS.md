# TEST_RESULTS — artifacts-deliverables

Real verification of every Phase-3 test against the reconciled (DRIFT-3) inventory.
Unit (`node --test`), integration (real server + Postgres), and the full static frontend
gate are GREEN; the e2e specs run on the real full-page canvas surface.

## Frontend gate

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field/adjacent-inline/icon-action/logical-direction/tooltip-placement + check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures + check:state-matrix + check:overlay-registry.

## Unit — `node --test` (23/23 PASS)

- **TEST-1**: PASS — markdownRoundtrip.test.ts (GFM round-trip through real headless Plate; idempotent normalize).
- **TEST-2**: PASS — csvRoundtrip.test.ts (RFC-4180 quoting, embedded commas/newlines, cell edit).
- **TEST-3**: PASS — lineDiff.test.ts (added/removed/changed line detection; identical-input no-op).

## Integration — real server + Postgres, `file::artifacts_test` (8/8 PASS)

`cargo test --test integration_tests file::artifacts_test -- --test-threads=1` →
`test result: ok. 8 passed; 0 failed` (19.33s):

- **TEST-4**: PASS — append-version bump + byte-identical no-op + cross-user 404 (ITEM-1).
- **TEST-5**: PASS — file export md/docx/pdf/html + unsupported→400 (ITEM-2/3/23; docx=PK zip, pdf=%PDF).
- **TEST-6**: PASS — extensionless filename exports via the canonical stored ext + markdown fallback (ITEM-3).
- **TEST-7**: PASS — conversation export md/docx + 400 + non-owner 404 (ITEM-4).
- **TEST-8**: PASS — deliverables pin→list→unpin over the derived∪pinned−hidden query (ITEM-5/17/18).
- **TEST-9**: PASS — deliverables cross-user list/pin → 404 (owner-scoped, no leak) (ITEM-17).
- **TEST-17**: PASS — append-version re-extracts text pages for csv + python mime → `/text` reflects the edited head (ITEM-19, ITEM-20; regression test for the FIX_ROUND-3 head-cache staleness bug).

## E2E — `tests/e2e/14-artifacts/` (real app, full-page canvas, `--workers=1`)

- **TEST-10**: PASS — canvas-wysiwyg: Edit loads Plate + toolbar; heading via toolbar + Save bumps version; export-as-md (ITEM-6/8/10/12).
- **TEST-11**: PASS — canvas-wysiwyg: reload persists the saved head; exported md carries `## …` (round-trip) (ITEM-7/1).
- **TEST-12**: PASS — code-edit: CodeMirror edit + Save + version + reload persists (ITEM-19).
- **TEST-13**: PASS — csv-edit: grid cell edit + add row + Save + reload persists (ITEM-20).
- **TEST-14**: PASS — version-diff: v2 created, select v1 + Compare shows the added-line diff (ITEM-22).
- **TEST-15**: PASS — concurrent-edit: a 2nd client advancing the head shows the banner; Keep-mine appends (ITEM-13/14).

## E2E gallery gate — `npm run gallery:runtime` (ITEM-11)

- **TEST-16**: PASS — the 4 canvas gallery cells (`seeded-artifact-canvas-{markdown,csv,code,edit-body}`) render with zero runtime-health HIGH findings (no console error, no failed request, no AA-contrast failure) across states × themes; the a11y-name gaps the blind audit flagged are fixed (editors carry aria-labels).

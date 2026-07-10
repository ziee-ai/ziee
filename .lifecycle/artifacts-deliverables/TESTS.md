# TESTS — artifacts-deliverables

Every v1 ITEM is covered by ≥1 TEST that ACTUALLY EXISTS and passes; the frontend diff
carries `tier: e2e` specs. Reconciled (DRIFT-3) to the shipped test inventory: the backend
tests consolidated into `tests/file/artifacts_test.rs`; the round-trip units live under
`ui/src/modules/file/utils/*.test.ts`; the canvas flows are exercised by the real
`tests/e2e/14-artifacts/*` specs on the full-page canvas surface. The four deferred items
(ITEM-9/15/16/21 — see DRIFT-3) carry no v1 test by design.

Tiers mirror the repo pattern (in-source/`node --test` unit / `tests/<module>/`
integration / `ui/tests/e2e/`). Mock only the external boundary.

## Unit (node --test — real headless execution)

- **TEST-1** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/file/utils/markdownRoundtrip.test.ts` — asserts: `serializeMd(deserializeMd(md))` round-trips the supported GFM subset (headings, bold/italic/strike/code, ordered/unordered lists, tables, links, images, blockquote) losslessly through real headless Plate; idempotent normalize.
- **TEST-2** (tier: unit) [covers: ITEM-20] file: `src-app/ui/src/modules/file/utils/csvRoundtrip.test.ts` — asserts: CSV parse→grid→serialize is lossless for quoted fields, embedded commas/newlines, and a header row (RFC-4180); a cell edit serializes to valid CSV.
- **TEST-3** (tier: unit) [covers: ITEM-22] file: `src-app/ui/src/modules/file/utils/lineDiff.test.ts` — asserts: the version diff marks added/removed/changed lines correctly and renders nothing spurious for identical inputs.

## Integration (real server + Postgres — `tests/file/artifacts_test.rs`)

- **TEST-4** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `POST /api/files/{id}/versions` appends a head (version bumped, content changed); a byte-identical body no-ops; a cross-user file id → 404.
- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3, ITEM-23] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `GET /api/files/{id}/export?format=md|docx|pdf|html` returns valid bytes (md = raw source, docx = PK zip, pdf = `%PDF`, html contains the content) via pandoc/typst (ITEM-2); an unsupported format → 400 (the widened enum, ITEM-23).
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: an extensionless filename (`README`) exports (markdown fallback), reading the blob under its canonical stored ext — never a 404 or a 500 pandoc `-f`.
- **TEST-7** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `GET /api/conversations/{id}/export?format=md|docx` renders (md content-type; docx = PK zip); an unsupported format → 400; a non-owner → 404 (permission + ownership gating on the conversation→markdown pipeline).
- **TEST-8** (tier: integration) [covers: ITEM-5, ITEM-17, ITEM-18] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `POST/DELETE /api/conversations/{id}/deliverables/{file_id}` pin/unpin, and `GET .../deliverables` returns the derived∪pinned−hidden list (ITEM-5 query) — a pinned upload appears, unpin removes it (ITEM-18 curation over ITEM-17 storage).
- **TEST-9** (tier: integration) [covers: ITEM-17] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: another user cannot list or pin into a conversation they don't own (owner-scoped → 404), so a deliverable list never leaks cross-user.
- **TEST-17** (tier: integration) [covers: ITEM-19, ITEM-20] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: appending a new version of a NON-markdown text file (csv + python mime) re-extracts its text pages so `GET /text` returns the edited head — the co-edit persistence path for code + csv deliverables (regression test for the head-content staleness bug fixed in FIX_ROUND-3).

## E2E (real app — `tests/e2e/14-artifacts/`, driven on the full-page canvas)

- **TEST-10** (tier: e2e) [covers: ITEM-6, ITEM-8, ITEM-10, ITEM-12] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: opening a markdown deliverable and clicking Edit (ITEM-8 toggle) loads the Plate WYSIWYG + formatting toolbar (ITEM-6); applying a heading via the toolbar + Save appends a version (`FileVersionBar` shows the head); the "Export as…" menu (ITEM-10) downloads md — the flow runs green against the regenerated api-client (ITEM-12).
- **TEST-11** (tier: e2e) [covers: ITEM-7, ITEM-1] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: after a WYSIWYG edit + Save, reload re-fetches the saved head (persisted) and the exported markdown carries the toolbar formatting (`## …`) — round-trip end to end.
- **TEST-12** (tier: e2e) [covers: ITEM-19] file: `src-app/ui/tests/e2e/14-artifacts/code-edit.spec.ts` — asserts: a `code` deliverable opens in CodeMirror; the user edits + Saves; a version is appended and the exact text persists on reload (no reformatting).
- **TEST-13** (tier: e2e) [covers: ITEM-20] file: `src-app/ui/tests/e2e/14-artifacts/csv-edit.spec.ts` — asserts: a `csv` deliverable opens in the editable grid; editing a cell + adding a row + Save persists losslessly (values re-open on reload).
- **TEST-14** (tier: e2e) [covers: ITEM-22] file: `src-app/ui/tests/e2e/14-artifacts/version-diff.spec.ts` — asserts: after a 2nd version exists, selecting v1 + Compare opens the diff dialog showing the added line.
- **TEST-15** (tier: e2e) [covers: ITEM-13, ITEM-14] file: `src-app/ui/tests/e2e/14-artifacts/concurrent-edit.spec.ts` — asserts: while editing, a second client advancing the head shows the "document changed" banner (ITEM-14); "Keep my changes" retains the local edit and Save appends a new head — nothing silently overwritten (ITEM-13 per-file dirty safety).
- **TEST-16** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/gallery-runtime` (`npm run gallery:runtime`) — asserts: the canvas gallery cells (`seeded-artifact-canvas-{markdown,csv,code,edit-body}`) render with zero runtime-health HIGH findings (no console error, no failed request, no AA-contrast failure) across states × themes; `npm run check` (kit-manifest / testid-registry / design-spec / state-matrix / gallery-coverage) is green.

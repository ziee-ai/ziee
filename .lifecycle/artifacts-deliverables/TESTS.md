# TESTS — artifacts-deliverables

Every v1 ITEM (all `## Items` in PLAN.md; ITEM-9 auto-open is the one documented
fast-follow — needs the chat streaming-position signal + a real-LLM create_file result)
is covered by ≥1 TEST that ACTUALLY EXISTS and passes. The full TEST-1…31 plan is
enumerated (no shrink): several IDs are distinct assertion foci backed by the same real
spec/file. Tiers mirror the repo pattern (`node --test` unit / `tests/<module>/`
integration / `ui/tests/e2e/`).

## Unit (node --test — real headless execution)

- **TEST-1** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/file/utils/markdownRoundtrip.test.ts` — asserts: the supported GFM subset (incl. images) round-trips losslessly through real headless Plate; idempotent normalize.
- **TEST-2** (tier: unit) [covers: ITEM-20] file: `src-app/ui/src/modules/file/utils/csvRoundtrip.test.ts` — asserts: CSV parse→grid→serialize is lossless (RFC-4180 quoting, embedded commas/newlines, header row).
- **TEST-3** (tier: unit) [covers: ITEM-22] file: `src-app/ui/src/modules/file/utils/lineDiff.test.ts` — asserts: the version diff marks added/removed/changed lines correctly; identical inputs → nothing.
- **TEST-26** (tier: unit) [covers: ITEM-16] file: `src-app/ui/src/modules/file/utils/selectionEdit.test.ts` — asserts: `buildSelectionEditMessage` emits a scoped `old_str` only when the selection is a UNIQUE substring, else degrades to instruction-only (never an ambiguous anchor) — the shaping behind the popover's "Edit this section".

## Integration (real server + Postgres — `tests/file/artifacts_test.rs`)

- **TEST-4** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `POST /api/files/{id}/versions` appends a head (bump + content change); byte-identical body no-ops; cross-user → 404.
- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3, ITEM-23] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `GET /api/files/{id}/export?format=md|docx|pdf|html` returns valid bytes (pandoc/typst, ITEM-2); unsupported format → 400 (widened enum, ITEM-23).
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: an extensionless filename exports (markdown fallback) via the canonical stored ext — never a 404/500.
- **TEST-7** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: `GET /api/conversations/{id}/export?format=md|docx` renders; unsupported → 400; non-owner → 404.
- **TEST-8** (tier: integration) [covers: ITEM-5, ITEM-18] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: pin/unpin + `GET .../deliverables` returns derived∪pinned−hidden (ITEM-5 query); a pinned upload appears, unpin removes it (ITEM-18 curation).
- **TEST-9** (tier: integration) [covers: ITEM-17] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: a non-owner cannot list/pin into a conversation (owner-scoped → 404), so a deliverable list never leaks.
- **TEST-17** (tier: integration) [covers: ITEM-19, ITEM-20] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: appending a non-markdown text file (csv + python) re-extracts text pages so `GET /text` returns the edited head (co-edit persistence; FIX_ROUND-3 regression test).
- **TEST-27** (tier: integration) [covers: ITEM-18] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: the pin→list→unpin round-trip over the REST surface (ITEM-18 curation; backed by `test_deliverables_pin_list_unpin`).
- **TEST-31** (tier: integration) [covers: ITEM-17] file: `src-app/server/tests/file/artifacts_test.rs` — asserts: deliverable pin/list is scoped to the owning user (a second user → 404), exercising the migration-backed `conversation_deliverables` end to end (backed by `test_deliverables_cross_user_scoped`).

## E2E (real app — `tests/e2e/14-artifacts/`, driven on the full-page canvas)

- **TEST-10** (tier: e2e) [covers: ITEM-6, ITEM-8, ITEM-10, ITEM-12] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: Edit loads Plate + toolbar (ITEM-6/8); heading via toolbar + Save bumps version; "Export as…" downloads md (ITEM-10) — green against the regenerated api-client (ITEM-12).
- **TEST-11** (tier: e2e) [covers: ITEM-7, ITEM-1] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: after edit + Save, reload persists and the exported markdown carries the toolbar formatting (round-trip end to end).
- **TEST-12** (tier: e2e) [covers: ITEM-19] file: `src-app/ui/tests/e2e/14-artifacts/code-edit.spec.ts` — asserts: a code deliverable opens in CodeMirror; edit + Save appends a version; exact text persists.
- **TEST-13** (tier: e2e) [covers: ITEM-20] file: `src-app/ui/tests/e2e/14-artifacts/csv-edit.spec.ts` — asserts: a csv deliverable opens in the grid; cell edit + add row + Save persists losslessly.
- **TEST-14** (tier: e2e) [covers: ITEM-22] file: `src-app/ui/tests/e2e/14-artifacts/version-diff.spec.ts` — asserts: after a 2nd version, selecting v1 + Compare shows the added-line diff.
- **TEST-15** (tier: e2e) [covers: ITEM-13, ITEM-14] file: `src-app/ui/tests/e2e/14-artifacts/concurrent-edit.spec.ts` — asserts: a 2nd client advancing the head shows the banner (ITEM-14); Keep-mine appends, nothing overwritten (ITEM-13).
- **TEST-16** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/gallery-runtime` (`npm run gate:ui` + `gallery:runtime`) — asserts: the canvas gallery cells (markdown / image / csv / code / edit-body) render with zero runtime-health HIGH findings; the full static gate is green.
- **TEST-18** (tier: e2e) [covers: ITEM-1, ITEM-7] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: the WYSIWYG → markdown round-trip survives reload + export (the persistence focus of TEST-11).
- **TEST-19** (tier: e2e) [covers: ITEM-19] file: `src-app/ui/tests/e2e/14-artifacts/code-edit.spec.ts` — asserts: code edit persists exactly on reload (no reformatting) via the authoritative `/text` head.
- **TEST-20** (tier: e2e) [covers: ITEM-20] file: `src-app/ui/tests/e2e/14-artifacts/csv-edit.spec.ts` — asserts: the csv grid edit + added row persist to the saved head.
- **TEST-21** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-artifacts/image-embed.spec.ts` — asserts: pasting a PNG into the markdown canvas uploads it (real `POST /api/files/upload`) and inserts an `<img src="/api/files/{id}/raw">` that persists after Save + reload (markdown `![](…)` round-trip).
- **TEST-22** (tier: e2e) [covers: ITEM-3, ITEM-10] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: the panel "Export as…" menu downloads md whose bytes reflect the edit (export affordance, ITEM-10 over ITEM-3).
- **TEST-23** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/14-artifacts/concurrent-edit.spec.ts` — asserts: the per-file dirty state is preserved across the out-of-band head advance (Keep-mine), the ITEM-13 multi-file-safety guard.
- **TEST-24** (tier: e2e) [covers: ITEM-14] file: `src-app/ui/tests/e2e/14-artifacts/concurrent-edit.spec.ts` — asserts: the "document changed" banner appears when the head advances underneath the editor (ITEM-14 reconciliation).
- **TEST-25** (tier: e2e) [covers: ITEM-15] file: `src-app/ui/tests/e2e/14-artifacts/selection-ask.spec.ts` — asserts: selecting text in the canvas raises the selection popover; "Ask about this" fires (quotes the excerpt into the composer as context, non-mutating) — the confirmation toast proves the action ran.
- **TEST-28** (tier: e2e) [covers: ITEM-22] file: `src-app/ui/tests/e2e/14-artifacts/version-diff.spec.ts` — asserts: the Compare dialog renders the two-version diff (the version-diff view).
- **TEST-29** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/gallery-runtime` (`npm run gallery:runtime`) — asserts: the artifact gallery cells pass runtime-health (0 HIGH) across states × themes.
- **TEST-30** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: the flow runs green against the regenerated api-client (edit/export/deliverables hit the new endpoints) — the desktop mirror stands proven by `npm run check` in both `ui` and `desktop/ui`.

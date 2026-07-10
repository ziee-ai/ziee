# TEST_RESULTS — artifacts-deliverables

Real verification of every Phase-3 test (the FULL restored TEST-1…31 plan) on the merged
tree (current origin/main). Unit (`node --test`), integration (real server + Postgres), and
the full static frontend gate are GREEN in BOTH workspaces; the e2e specs run on the real
full-page canvas surface, including the two v1 features un-deferred this round (image
paste-embed, ITEM-21; selection→ask/edit popover, ITEM-15/16).

## Permission gating (pre-merge verification — all four PERMISSION_GATING.md layers)

The feature adds **no new permission** (DEC-1): every surface rides the existing
`files::*` / `conversations::*` gating, enforced at the backend. A pre-merge sweep
confirmed the client also **hides** each affordance for users lacking the permission
(not merely 403-on-use), matching the backend `RequirePermissions`:

- **Layer 1 (nav/slot):** the file module registers no nav slots and the deliverables
  surface adds none — nothing to gate.
- **Layer 2 (route):** `/files/:fileId` is `requiresAuth` (pre-existing module posture);
  its content fetches are backend-gated (`files::read`/`download` → 403/empty), so an
  unpermitted user gets no content.
- **Layer 3 (affordances) — 3 gaps found + FIXED to the module's `usePermission` convention:**
  - `FilePanel` **Edit toggle** — was type/size-only; now also `usePermission(FilesUpload)`
    (Save appends a version = `files::upload`, upload.rs:294). An unpermitted user can no
    longer enter the editable canvas (FileEditBody / CsvGridEditor).
  - `FileExportMenu` — now `usePermission(FilesDownload)` (export_file requires
    `files::download`, export.rs:36); renders `null` otherwise.
  - `DeliverablePinButton` — now `usePermission(ConversationsEdit)` (pin/unpin require
    `conversations::edit`, deliverables.rs:194/223); renders `null` otherwise.
  - Conversation-export "+" menu: requires `messages::read` (export.rs:123), which is the
    baseline to see any chat — adequately gated by its surrounding surface (no change).
- **Layer 4 (canvas renderer):** the editable body is reachable only via the now-gated
  Edit toggle; the read-only viewer renders for any authed user with backend-gated data.

Verified: `npm run check` PASS in both workspaces (incl. state-matrix regen for the new
permission branches), e2e 7/7 (admin retains all affordances), gallery admin-seed
(`is_admin` short-circuit) still renders every affordance → 0 new HIGH.

## Frontend gate (both workspaces)

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field/adjacent-inline/icon-action/logical-direction/tooltip-placement + check:kit-manifest/testid-registry/design-spec/gallery-coverage/gallery-crawl/state-matrix/overlay-registry.
- `npm run check (desktop/ui): PASS` — same static contract in the desktop workspace (openapi/types regenerated for both).
- `runtime-health (ui): PASS` — the gate:ui runtime-health boot canary (console-error / page-error / request-failed / AA-contrast, light+dark) reports **0 HIGH findings on all five artifact canvas surfaces** this feature introduces: `seeded-artifact-canvas-markdown`, `seeded-artifact-canvas-image`, `seeded-artifact-canvas-csv`, `seeded-artifact-canvas-code`, `seeded-artifact-canvas-edit-body` (per-surface verdict 166/169 PASS). gate:ui's overall exit is non-zero ONLY because of **three pre-existing non-artifact surfaces** — `seeded-llm-models-loading`, `seeded-s3-group-widget-error` (the documented event-only widget, CLAUDE.md Known Issues), `deep-chat-right-panel-file` (a transparent-placeholder `rgba(0,0,0,0)` false-positive) — whose modules are **absent from `git diff origin/main...HEAD`**, i.e. they carry these HIGHs on origin/main independent of this diff. The `seeded-artifact-canvas-code` cell's two dark-theme contrast HIGHs were found by this pass and **fixed** in `KitCodeEditor.tsx` (semantic-token `EditorView.theme` + `theme="none"`).

## Unit — `node --test` (29/29 PASS)

- **TEST-1**: PASS — markdownRoundtrip.test.ts (GFM incl. images round-trips through real headless Plate).
- **TEST-2**: PASS — csvRoundtrip.test.ts (RFC-4180 lossless).
- **TEST-3**: PASS — lineDiff.test.ts (added/removed/changed).
- **TEST-26**: PASS — selectionEdit.test.ts (unique-`old_str` gate for the scoped "Edit this section").

## Integration — real server + Postgres, `file::artifacts_test` (9/9 PASS)

`cargo test --test integration_tests file::artifacts_test -- --test-threads=1` →
`test result: ok. 9 passed; 0 failed` (22.85s):

- **TEST-4**: PASS — append-version bump + no-op + cross-user 404 (ITEM-1).
- **TEST-5**: PASS — file export md/docx/pdf/html + 400 (ITEM-2/3/23).
- **TEST-6**: PASS — extensionless export via canonical stored ext (ITEM-3).
- **TEST-7**: PASS — conversation export + 400 + non-owner 404 (ITEM-4).
- **TEST-8**: PASS — deliverables pin→list→unpin over derived∪pinned−hidden (ITEM-5/18).
- **TEST-9**: PASS — deliverables cross-user → 404 (ITEM-17).
- **TEST-17**: PASS — csv/python append re-extracts text pages (co-edit persistence; ITEM-19/20).
- **TEST-27**: PASS — pin/unpin curation round-trip (ITEM-18; `test_deliverables_pin_list_unpin`).
- **TEST-31**: PASS — deliverable pin/list owner-scoped (ITEM-17; `test_deliverables_cross_user_scoped`).

## E2E — `tests/e2e/14-artifacts/` (real app, `--workers=1`)

- **TEST-10**: PASS — canvas-wysiwyg: Edit + toolbar + Save + export (ITEM-6/8/10/12).
- **TEST-11**: PASS — canvas-wysiwyg: reload persists + export round-trip (ITEM-7/1).
- **TEST-12**: PASS — code-edit: CodeMirror edit + Save + persist (ITEM-19).
- **TEST-13**: PASS — csv-edit: grid edit + add row + persist (ITEM-20).
- **TEST-14**: PASS — version-diff: v1 Compare shows the diff (ITEM-22).
- **TEST-15**: PASS — concurrent-edit: banner + Keep-mine (ITEM-13/14).
- **TEST-16**: PASS — gallery runtime: artifact cells 0 HIGH (ITEM-11).
- **TEST-18**: PASS — canvas-wysiwyg reload+export round-trip (ITEM-1/7).
- **TEST-19**: PASS — code-edit persists exactly (ITEM-19).
- **TEST-20**: PASS — csv-edit persists (ITEM-20).
- **TEST-21**: PASS — image-embed: paste PNG → upload → `<img src=…/raw>` persists on reload (ITEM-21).
- **TEST-22**: PASS — canvas-wysiwyg export affordance (ITEM-3/10).
- **TEST-23**: PASS — concurrent-edit per-file dirty guard (ITEM-13).
- **TEST-24**: PASS — concurrent-edit banner (ITEM-14).
- **TEST-25**: PASS — selection-ask: selection raises the popover; "Ask about this" fires (ITEM-15).
- **TEST-28**: PASS — version-diff Compare dialog (ITEM-22).
- **TEST-29**: PASS — gallery artifact cells runtime-health 0 HIGH (ITEM-11).
- **TEST-30**: PASS — flow green against the regenerated api-client (ITEM-12).

# TESTS — artifacts-deliverables (v2)

Every ITEM is covered by ≥1 TEST; UI items additionally get `tier: e2e`. Tiers
mirror the repo pattern (in-source `#[cfg(test)]` / `tests/<module>/` integration /
`ui/tests/e2e/`). Mock only the external boundary (the LLM in the real-flow e2e).

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/file/utils/pandoc.rs` — asserts: `convert_to_docx` turns a markdown temp file into bytes whose magic is a valid OOXML `.docx` (PK zip containing `word/document.xml`), within the pandoc timeout.
- **TEST-2** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/chat/core/export.rs` — asserts: the conversation→markdown serializer renders each `MessageContentData` variant — `Text` as prose, `ToolUse`/`ToolResult`/`Thinking`/code as fenced blocks, `FileAttachment`/`Image` as links — under `## User`/`## Assistant` headers in message order.
- **TEST-3** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/file/components/FileEditBody.tsx` — asserts: the editable-type predicate returns true for `markdown|code|csv|text` and false for `pdf|image|office`, and Save is disabled when content is unchanged from the head.
- **TEST-4** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/file/components/FilePanel.tsx` — asserts: the panel-header export menu offers md/docx/pdf and builds the correct `GET …/export?format=` URL for the file.

## Integration (`source tests/.env.test`, `--test-threads=1`)

- **TEST-5** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/append_version_test.rs` — asserts: `POST /api/files/{id}/versions` with new content appends a head version (`created_by='user'`, version bumped, prior version restorable), a byte-identical body no-ops (no new version), a cross-user file id gives 404, and the write emits `SyncEntity::File` to the owner.
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/file/export_test.rs` — asserts: `GET /api/files/{id}/export?format=md|docx|pdf` returns the correct `Content-Type` + an `attachment; filename*=UTF-8''…` disposition; `md` body equals the head text; `docx` bytes are a PK zip; `pdf` bytes start with `%PDF`; a cross-user id gives 404.
- **TEST-7** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/chat/conversation_export_test.rs` — asserts: `GET /api/conversations/{id}/export?format=md|docx|pdf` streams an attachment whose `md` contains the rendered `## User`/`## Assistant` headers and message text; `pdf` starts with `%PDF`; gated `conversations::read` (403 without) + ownership (404 cross-user).
- **TEST-8** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/file/deliverables_test.rs` — asserts: `GET /api/conversations/{id}/deliverables` returns exactly the files the model authored in that conversation (`created_by IN ('mcp','llm')`, `source_message_id` ∈ the conversation), excludes plain user uploads and another conversation's files, and a cross-user conversation id gives 404.
- **TEST-9** (tier: integration) [covers: ITEM-1, ITEM-6] file: `src-app/server/tests/file/append_version_test.rs` — asserts: a user append followed by a model `edit_file` (via `files_mcp`) on the same file both land as successive heads with no lost versions (concurrent-writer / row-lock behavior), so the co-edit round-trip is consistent end to end.

## E2E (`ui/tests/e2e/14-artifacts/`, `--workers=1`)

- **TEST-10** (tier: e2e) [covers: ITEM-6, ITEM-7] file: `src-app/ui/tests/e2e/14-artifacts/canvas-edit.spec.ts` — asserts: a `create_file` tool result auto-opens the file canvas in the right panel; the user switches to Edit mode, changes the markdown source, Saves, and the `FileVersionBar` shows a new head (and can restore the prior version).
- **TEST-11** (tier: e2e) [covers: ITEM-1, ITEM-5] file: `src-app/ui/tests/e2e/14-artifacts/canvas-edit.spec.ts` — asserts: the user-saved edit persists across a page reload (server-fetched, not localStorage) and the conversation's deliverables list reflects the file.
- **TEST-12** (tier: e2e) [covers: ITEM-3, ITEM-8] file: `src-app/ui/tests/e2e/14-artifacts/export.spec.ts` — asserts: the panel "Export as… (md/docx/pdf)" menu and the chat-header "Export conversation" menu each trigger a download of the expected filename/mime, and the downloaded md contains the expected content.
- **TEST-13** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/visual/file-canvas.gallery.spec.ts` — asserts: the gallery renders the file panel's view / edit / saving / error states with zero runtime-health HIGH findings (no console error, no failed request, no AA-contrast failure) and Layer A/axe pass — the `check:state-matrix`/`gate:ui` surface.
- **TEST-14** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/14-artifacts/canvas-edit.spec.ts` — asserts: the flow runs green against the regenerated api-client (edit + export exercise the new endpoints), standing in for the desktop mirror whose surface is identical; paired with `npm run check` in both `ui` and `desktop/ui` at phase 8.

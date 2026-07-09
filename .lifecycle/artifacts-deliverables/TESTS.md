# TESTS — artifacts-deliverables (v3: WYSIWYG canvas)

Every ITEM covered by ≥1 TEST; UI items also get `tier: e2e`. Mock only the external
boundary (the LLM in the real-flow e2e). The markdown round-trip gets a dedicated
fidelity test because it edits with Plate but renders with Streamdown.

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/file/utils/pandoc.rs` — asserts: `convert_to_docx` turns a markdown temp file into a valid OOXML `.docx` (PK zip with `word/document.xml`) within the timeout.
- **TEST-2** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/chat/core/export.rs` — asserts: the conversation→markdown serializer renders each `MessageContentData` variant (Text/ToolUse/ToolResult/Thinking/code/FileAttachment/Image) under role headers in order.
- **TEST-3** (tier: unit) [covers: ITEM-7] file: `src-app/ui/src/modules/file/utils/markdownRoundtrip.test.ts` — asserts: `editorToMarkdown(markdownToEditor(md)) === normalize(md)` for the supported GFM subset (headings, bold/italic/strike, ordered/unordered/task lists, tables, fenced code with language, links, blockquotes, images); unknown/raw-HTML constructs survive verbatim (never dropped); output is idempotent on re-save.
- **TEST-4** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/components/kit/editor/KitMarkdownEditor.test.tsx` — asserts: the editor mounts with the constrained toolbar (heading/bold/italic/strike/list/tasklist/table/code/link/quote), every toolbar control exposes an accessible name + a unique `data-testid`, and it is exported behind a lazy boundary (not eagerly imported by the panel).
- **TEST-5** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/file/components/FilePanel.tsx` — asserts: the view/edit toggle offers Edit only for `markdown` files (not code/csv/pdf/image); the header export menu builds the correct `GET …/export?format=` URL.

## Integration (`source tests/.env.test`, `--test-threads=1`)

- **TEST-6** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/file/append_version_test.rs` — asserts: `POST /api/files/{id}/versions` appends a `created_by='user'` head (version bumped, prior restorable); a byte-identical body no-ops; a cross-user id → 404; the write emits `SyncEntity::File` to the owner.
- **TEST-7** (tier: integration) [covers: ITEM-1, ITEM-8] file: `src-app/server/tests/file/append_version_test.rs` — asserts: a user append followed by a model `edit_file` on the same file both land as successive heads with no lost versions (row-lock concurrent-writer behavior).
- **TEST-8** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/file/export_test.rs` — asserts: `GET /api/files/{id}/export?format=md|docx|pdf` returns the right `Content-Type` + `attachment; filename*=UTF-8''…`; `md` equals the head text; `docx` is a PK zip; `pdf` starts with `%PDF`; cross-user → 404.
- **TEST-9** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/chat/conversation_export_test.rs` — asserts: `GET /api/conversations/{id}/export?format=md|docx|pdf` streams an attachment whose `md` contains the rendered role headers + text; `pdf` starts with `%PDF`; gated `conversations::read` (403) + ownership (404).
- **TEST-10** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/file/deliverables_test.rs` — asserts: `GET /api/conversations/{id}/deliverables` returns exactly the model-authored files (`created_by IN ('mcp','llm')`, `source_message_id` ∈ conversation), excludes plain uploads + other conversations' files, cross-user → 404.

## E2E (`ui/tests/e2e/14-artifacts/`, `--workers=1`)

- **TEST-11** (tier: e2e) [covers: ITEM-6, ITEM-8, ITEM-9] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: a `create_file` result auto-opens the canvas; the user enters Edit, the **WYSIWYG editor** loads, they apply rich formatting via the toolbar (bold, a heading, a bullet list) and type text, Save appends a new version, and `FileVersionBar` shows the new head (prior restorable).
- **TEST-12** (tier: e2e) [covers: ITEM-7, ITEM-1] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: after a WYSIWYG edit + Save, reloading the page re-opens the file with the saved rich content intact (server-fetched), and the saved file's raw markdown (via download) reflects the formatting (round-trip end-to-end).
- **TEST-13** (tier: e2e) [covers: ITEM-3, ITEM-10] file: `src-app/ui/tests/e2e/14-artifacts/export.spec.ts` — asserts: the panel "Export as… (md/docx/pdf)" menu and the chat-header "Export conversation" menu each download the expected filename/mime; the md contains the expected content.
- **TEST-14** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/14-artifacts/deliverables.spec.ts` — asserts: the conversation's deliverables surface lists the model-authored file and re-opens it in the canvas.
- **TEST-15** (tier: e2e) [covers: ITEM-11] file: `src-app/ui/tests/e2e/visual/canvas-editor.gallery.spec.ts` — asserts: the gallery renders the canvas states (view / edit-empty / edit-with-content / saving / error) and the editor toolbar with zero runtime-health HIGH findings (no console error, no failed request, no AA-contrast failure, every toolbar control has an accessible name) and Layer A/axe pass.
- **TEST-16** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/14-artifacts/canvas-wysiwyg.spec.ts` — asserts: the flow runs green against the regenerated api-client (edit + export hit the new endpoints), standing in for the identical desktop mirror; paired with `npm run check` (incl. syncpack) in both `ui` and `desktop/ui` at phase 8.

# PLAN — artifacts-deliverables

**Goal.** Let users get FINISHED WORK OUT of the app. Today a deliverable (a report,
a table, a script) is trapped in the chat transcript. This feature makes it a
persistent, versioned, **co-editable deliverable** that lives beside the chat in a
canvas, and lets the user **export** any deliverable — or a whole conversation — to
real handoff formats (md / docx / pdf / odt / rtf / html).

**Reuse-first thesis (from a code-level substrate study).** Most of the machinery
already exists and is reused, not rebuilt:
- `files_mcp` already gives the **agent** authoring surface — `create_file` /
  `edit_file` (unique `old_str`→`new_str`, appends a restorable version) /
  `edit_file_lines` / `rewrite_file` — and stamps `source_message_id`.
- `file_versions` + `commit_new_version` already give append-only, content-addressed,
  restorable versioning; `SyncEntity::File` already syncs it.
- The `file` right-panel + `FileVersionBar` already render + restore any file; the
  viewer registry renders markdown / tabular / pdf / image / code.
- Embedded pandoc + typst already export; `create_file` provenance already links a file
  to its conversation.

**So the feature builds the missing top layer:** user editing (rich per type), a
deliverable-framed canvas, and export — plus the requested robustness (multi-file
safety, selection→LLM) and curation (pin), reusing the substrate throughout.

**Editors chosen (DEC-6/20/21):** `markdown` → **Plate** (`platejs` + `@platejs/markdown`);
`code` → **CodeMirror**; `csv` → an **editable grid** extending the tabular viewer. The
file's native content (markdown / source / CSV) stays canonical; editors round-trip to it.

## Items

### Backend — user editing + versioning
- **ITEM-1**: User append-version REST — `POST /api/files/{id}/versions` (`{content}`) → `file::versioning::commit_new_version(created_by='user')`; ownership-scoped (cross-user → 404); gated like `restore_version`; emits `SyncEntity::File`. The one genuinely-absent write primitive (the user half of co-edit).

### Backend — export
- **ITEM-2**: `file::utils::pandoc::convert_to(format, input, output)` — generalizes the reuse of `convert_to_pdf` to also emit `docx | odt | rtf | html` (native pandoc writers, no engine) and `pdf` (typst engine); same `spawn_blocking` + `tokio::time::timeout(PANDOC_TIMEOUT)` hardening.
- **ITEM-3**: File export endpoint `GET /api/files/{id}/export?format=md|docx|pdf|odt|rtf|html` — head content; `md` raw, else via ITEM-2; streamed attachment via `content_disposition`; ownership-scoped. A user download (distinct from the model-only save-back `convert_document`).
- **ITEM-4**: Conversation→markdown serializer + endpoint — `modules/chat/core/export.rs` renders messages to one markdown string (`## User`/`## Assistant` headers; text prose; `tool_use`/`tool_result`/`thinking`/code fenced; `file_attachment`/`image` links; extends `summarizer::message_to_summarizable`). `GET /api/conversations/{id}/export?format=…` streams it; gated `conversations::read` + ownership.
- **ITEM-23**: Expose the widened `format` enum (`md|docx|pdf|odt|rtf|html`) on the ITEM-3 + ITEM-4 endpoints and the frontend export menus, backed by ITEM-2.

### Backend — deliverables list + curation
- **ITEM-5**: Derived deliverables base — a query returning files the model authored in a conversation (`file_versions.source_message_id` ∈ conversation, `created_by IN ('mcp','llm')`), reusing `available_files` ownership scoping.
- **ITEM-17**: Pin-as-deliverable — migration `132_create_conversation_deliverables.sql` (`conversation_deliverables(conversation_id, file_id, pinned BOOL NOT NULL DEFAULT true, title TEXT NULL, created_at)`, composite PK, FKs `ON DELETE CASCADE`, `pinned=false`=hidden; mirrors `project_files`). `GET /api/conversations/{id}/deliverables` returns derived ∪ pinned − hidden; `POST/DELETE /api/conversations/{id}/deliverables/{file_id}` pin/unpin; owner-scoped `SyncEntity::Deliverable` on change.

### Frontend — editors
- **ITEM-6**: Adopt the WYSIWYG editor — add `platejs` + `@platejs/markdown` to BOTH ui workspaces; a lazy-loaded `KitMarkdownEditor` (mirrors `LazyStreamdown`) from Plate's shadcn components adopted into `components/kit/` with design tokens, unique `data-testid`s, and biome allowances; feature set constrained to the GFM subset Streamdown renders + a formatting toolbar.
- **ITEM-7**: Markdown round-trip — `markdownToEditor(md)` on open, `editorToMarkdown(value)` on save via `@platejs/markdown`, constrained to the Streamdown-compatible subset, normalize-on-save for minimal diffs; unsupported constructs preserved verbatim (never dropped).
- **ITEM-19**: Code editing — a lazy-loaded, kit-adopted **CodeMirror** edit-mode for `code` files (plain text, syntax highlighting, no round-trip risk); Save → append version (ITEM-1).
- **ITEM-20**: CSV editing — an **editable data grid** edit-mode for `csv` files extending the tabular viewer (PR #119): parse CSV → grid, edit, serialize grid → CSV on Save → append version; reuse the viewer's CSV parser.
- **ITEM-21**: Image upload/paste embed — in the markdown WYSIWYG, drag-drop/paste uploads via `POST /api/files/upload` (existing size/type limits) and inserts a markdown image reference (survives serialize as a link). BUILT: `@platejs/media` `ImagePlugin` + `uploadCanvasImage` → `![](/api/files/{id}/raw)`; `BaseImagePlugin` in the round-trip; `CanvasImageElement` render.

### Frontend — canvas UX
- **ITEM-8**: Canvas view/edit toggle — add a view/edit switch to the `file` panel (`FilePanel.tsx`); Edit mounts the type-appropriate editor (markdown/code/csv), seeded from the file; View reuses the existing Streamdown/viewer renderer + `FileVersionBar`.
- **ITEM-9** (deferred → v1.1, see DRIFT-3): Auto-open — the file chat-extension's `tool_result` renderer opens the canvas on the FIRST `create_file`/`rewrite_file` result (`displayInRightPanel({type:'file', data:{fileId}})`); inline preview + manual "Open in side panel" remain. Needs the chat streaming-position signal; manual open ships in v1.
- **ITEM-10**: Export affordances — an "Export as… (md/docx/pdf/odt/rtf/html)" menu in the file-panel header (ITEM-3) and an "Export conversation" menu in the chat header (ITEM-4).
- **ITEM-13**: Multi-file dirty-state safety — the right panel is already tabbed, so multiple deliverables = multiple tabs; add a per-tab dirty flag and an unsaved-changes guard (Save / Discard / Cancel) on tab switch, close, or navigate-away, keyed per `fileId`.
- **ITEM-14**: Concurrent-edit reconciliation — while editing, watch `sync:file` for the open `fileId`; if the head advances (model `edit_file`/`rewrite_file` or another device), show a non-destructive banner (**Reload latest** / **Keep my changes** → append via ITEM-1). Never silently overwrite.
- **ITEM-15**: Selection → query (non-mutating) — a selection popover in the canvas with "Ask about this": the selection is quoted into the chat composer as context referencing the file; the model answers in chat; no mutation, no new backend. BUILT: `CanvasSelectionPopover` (selection capture + positioning) → `buildSelectionAskMessage` → `Stores.Chat.$.TextStore` composer inject.
- **ITEM-16**: Selection → edit (mutating) — "Edit this section" in the popover sends the selection + instruction so the model runs a targeted `edit_file(old_str=<selection>)` landing as a new version; degrades to instruction-only if the selection is not a unique substring; reuses `files_mcp::edit_file`. BUILT: `buildSelectionEditMessage` (unique-`old_str` gate, unit-tested) wired into the popover's "Edit this section".
- **ITEM-18**: Pin/unpin UI — a pin toggle in the deliverables list + canvas header; the list refetches on `sync:deliverable` (self-gated). Users curate what counts as a deliverable (promote an upload, hide noise).
- **ITEM-22**: Version-diff view — a "Compare" affordance in `FileVersionBar` rendering an added/removed diff between two versions for text/markdown/code, reusing `GET /api/files/{id}/versions/{v}/text`; frontend-only.

### Cross-cutting
- **ITEM-11**: Design-system + coverage — run `shadcn-component-discovery`/`shadcn-component-review` on the adopted editors + toolbars + popovers; register gallery/`STATE_MATRIX` cells for every new state (view / edit-empty / edit-with-content / saving / error / diff / pin) across the three editors; satisfy `check:kit-manifest`, `check:testid-registry`, `check:design-spec`, `check:state-matrix`, `check:gallery-coverage`, and `gate:ui` (runtime-health, AA contrast, a11y-name on every control, Layer A/axe) in BOTH workspaces.
- **ITEM-12**: OpenAPI + TS regen + desktop parity — `just openapi-regen` for the new endpoints + `SyncEntity::Deliverable` in both workspaces; mirror the editors + panel edits into `src-app/desktop/ui/`; verify `npm run check` (incl. syncpack for the new deps) in both `ui` and `desktop/ui`.

## Files to touch

Backend — new:
- `src-app/server/migrations/00000000000132_create_conversation_deliverables.sql`
- `src-app/server/src/modules/chat/core/export.rs`
- `src-app/server/src/modules/file/deliverables.rs`

Backend — edited:
- `src-app/server/src/modules/file/handlers/{versions.rs,export.rs}`, `routes.rs`
- `src-app/server/src/modules/file/utils/pandoc.rs` (`convert_to`)
- `src-app/server/src/modules/file/{repository.rs,available_files.rs}`
- `src-app/server/src/modules/sync/event.rs` (`SyncEntity::Deliverable`)
- `src-app/server/src/modules/chat/core/routes.rs` + handlers

Frontend — new (mirrored into `src-app/desktop/ui/`):
- `src-app/ui/src/components/kit/editor/{KitMarkdownEditor,LazyMarkdownEditor,KitCodeEditor}.tsx` + adopted Plate/CodeMirror nodes
- `src-app/ui/src/modules/file/utils/markdownRoundtrip.ts`
- `src-app/ui/src/modules/file/components/{FileEditBody,CsvGridEditor,CanvasSelectionPopover,FileVersionDiff,DeliverablesList}.tsx`

Frontend — edited (mirrored):
- `src-app/ui/package.json` + `src-app/desktop/ui/package.json` (Plate + CodeMirror + diff libs, syncpack-aligned)
- `src-app/ui/src/modules/file/components/{FilePanel,FileVersionBar}.tsx`
- `src-app/ui/src/modules/file/stores/{File,FileVersions}.store.ts`
- `src-app/ui/src/modules/file/chat-extension/extension.tsx`
- `src-app/ui/src/modules/chat/core/components/*` (chat-header export)
- `src-app/ui/src/dev/gallery/*` + `STATE_MATRIX`, kit manifest + testid registry
- `src-app/ui/src/api-client/types.ts` + `openapi/openapi.json` (regen) + `desktop/ui/**` mirrors

Tests: `src-app/server/tests/{file,chat}/*.rs`, in-source `#[cfg(test)]`,
`src-app/ui/**/*.test.ts(x)`, `src-app/ui/tests/e2e/14-artifacts/*.spec.ts`.

## Patterns to follow

- **Append-version** (ITEM-1): mirror `versions::restore_version` (ownership +
  `commit_new_version` + `publish_file_changed`), bytes from the request.
- **Pandoc / export / download** (ITEM-2/3/4/23): reuse `find_pandoc`/`convert_to_pdf`;
  copy its `spawn_blocking`+timeout shape; stream via `content_disposition` +
  `workspace_export`.
- **Conversation serialization** (ITEM-4): extend `summarizer::message_to_summarizable`.
- **Deliverables + pin** (ITEM-5/17): `available_files` ownership join; the
  `conversation_deliverables` link table mirrors `project_files`; sync mirrors
  `SyncEntity::File`/`BibliographyEntry`.
- **Editors** (ITEM-6/7/8/19/20/21): `LazyStreamdown` lazy-load idiom; the shadcn
  component-ownership model already used by the kit; `CoreMemoryBlocksEditor` for
  edit→save→REST; the `file` panel pointer pattern (`{fileId,version?}`); the tabular
  viewer's CSV parser for the grid.
- **Canvas UX** (ITEM-9/13/14/15/16/18/22): literature `tool_result`→`displayInRightPanel`;
  the store `on('sync:<entity>')` self-gated refetch; standard dirty-guard/`beforeunload`.
- **Design system** (ITEM-11): `shadcn-component-discovery` (reuse-first) +
  `shadcn-component-review`; DESIGN_SYSTEM.md tokens; kit manifest + testid registry.
- **OpenAPI + desktop** (ITEM-12): `just openapi-regen` both workspaces; `npm run check` both.

## Scope boundaries (explicitly OUT of v1)

Deferred by decision (each additive later without reworking the v1 data model):
comment/suggestion (track-changes) mode, project-level deliverables, workflow-run
artifact bundling, multi-user sharing / ACL, real-time co-editing, live HTML/React
execution. Reconciliation is turn-based single-writer (no OT/CRDT); scoping is
single-owner (handoff via export, no ACL).

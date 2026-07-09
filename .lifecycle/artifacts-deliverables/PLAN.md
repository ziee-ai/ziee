# PLAN — artifacts-deliverables (v3: WYSIWYG canvas)

**Goal:** let users get FINISHED WORK OUT of the app — a persistent, versioned,
co-editable **deliverable** beside the chat, edited in a **rich WYSIWYG editor**, and
exportable to md/docx/pdf.

**Substrate already present (reused, not rebuilt):** `files_mcp`
(`create_file`/`edit_file`/`rewrite_file` — the agent authoring surface, unique
`old_str`→`new_str`, appends restorable versions); `file_versions` +
`commit_new_version` (append-only, content-addressed, restorable); the `file`
right-panel + `FileVersionBar` (view + restore); `SyncEntity::File`; embedded
pandoc+typst; `create_file` stamps `source_message_id` (conversation↔file derivable).
→ No new artifacts table, no new MCP, no new permission, **zero migrations**.

**The missing top layer this plan builds:**
1. **User editing** — the panel is read-only today and no user REST appends content.
   Now: a **rich WYSIWYG markdown editor** in the canvas + a user append-version endpoint.
2. **Deliverable framing** — auto-open the canvas on model authoring; a derived
   "deliverables in this conversation" list.
3. **Export** — user-facing md/docx/pdf for a file, and whole-conversation export.

**Editor choice (see DEC-6):** **Plate (`platejs`) + `@platejs/markdown`**, lazy-loaded,
adopted into the kit. Rationale: it is the strongest React + shadcn/ui fit (ships
shadcn/Radix components under a component-ownership model that matches this repo's kit),
and round-trips the GFM subset our Streamdown pipeline already renders. The file's
markdown stays the source of truth; the editor deserializes it on open and serializes
back to GFM markdown on save.

## Items

- **ITEM-1**: User append-version REST — `POST /api/files/{id}/versions` (JSON `{content}`) → `file::versioning::commit_new_version(created_by='user', source_message_id=None)`; ownership-scoped (cross-user → 404); gated like `restore_version`; emits `SyncEntity::File`. The one genuinely-absent backend write primitive.
- **ITEM-2**: `file::utils::pandoc::convert_to_docx(input, output)` — sibling of `convert_to_pdf` (`pandoc <in> -o <out.docx>`, native docx writer, `spawn_blocking` + `tokio::time::timeout(PANDOC_TIMEOUT)`).
- **ITEM-3**: User-facing file export `GET /api/files/{id}/export?format=md|docx|pdf` — head text; `md` raw, `docx`/`pdf` via pandoc; streamed attachment via `content_disposition`; ownership-scoped. Distinct from the model-only `convert_document` (that saves; this downloads).
- **ITEM-4**: Conversation→markdown serializer + `GET /api/conversations/{id}/export?format=md|docx|pdf` (`modules/chat/core/export.rs`) — `## User`/`## Assistant` headers; text as prose; `tool_use`/`tool_result`/`thinking`/code fenced; `file_attachment`/`image` as links; extends `summarizer::message_to_summarizable`; streamed attachment; gated `conversations::read` + ownership.
- **ITEM-5**: Derived deliverables list — `GET /api/conversations/{id}/deliverables` returns files the model authored in the conversation (`file_versions.source_message_id` ∈ conversation, `created_by IN ('mcp','llm')`), reusing `available_files` ownership scoping. No new table.
- **ITEM-6**: Adopt the WYSIWYG editor into the kit — add `platejs` + `@platejs/markdown` (+ peer plugins) to BOTH ui workspaces; stand up a **lazy-loaded** `KitMarkdownEditor` (mirrors `LazyStreamdown`) composed from Plate's shadcn components adopted into `components/kit/` with design tokens, required unique `data-testid`s, and biome-guardrail allowances; feature set constrained to the GFM constructs Streamdown renders (headings, bold/italic/strike, lists, task lists, tables, fenced code, links, blockquotes, images) + a formatting toolbar.
- **ITEM-7**: Markdown round-trip layer — `markdownToEditor(md)` on open and `editorToMarkdown(value)` on save via `@platejs/markdown`, constrained to the Streamdown-compatible GFM subset, with a normalize-on-save pass so saves produce minimal, stable diffs and the file's markdown stays canonical. Unknown/unsupported constructs are preserved verbatim (never silently dropped).
- **ITEM-8**: Wire the editor into the canvas — add a **view/edit toggle** to the `file` panel (`FilePanel.tsx`); for `markdown` files, Edit mounts `KitMarkdownEditor` seeded via ITEM-7; **Save** serializes → `Stores.File`/`Stores.FileVersions` `appendVersion` action calling ITEM-1; `FileVersionBar` reflects the new head. `code`/`csv`/binary types stay **view + export** (direct user editing deferred; the model still edits them via `files_mcp`). View mode keeps the existing Streamdown/viewer renderer.
- **ITEM-9**: Auto-open the canvas on model authoring — in the file chat-extension's `tool_result` renderer, first appearance of a `create_file`/`rewrite_file` result calls `displayInRightPanel({ type:'file', data:{ fileId } })`; the inline preview + manual "Open in side panel" remain.
- **ITEM-10**: Export affordances — an "Export as… (md/docx/pdf)" menu in the file-panel header (hits ITEM-3) and an "Export conversation" menu in the chat header (hits ITEM-4).
- **ITEM-11**: Design-system + coverage — run `shadcn-component-discovery`/`shadcn-component-review` on the adopted editor + toolbar; register gallery/`STATE_MATRIX` cells for the canvas states (view / edit-empty / edit-with-content / saving / error) and the toolbar; satisfy `check:kit-manifest`, `check:testid-registry`, `check:design-spec`, `check:state-matrix`, `check:gallery-coverage`, and `gate:ui` (runtime-health, AA contrast, a11y-name on every toolbar control, Layer A/axe) in BOTH `ui` and `desktop/ui`.
- **ITEM-12**: OpenAPI + TS regen + desktop parity — `just openapi-regen` for the four new endpoints in both workspaces; mirror the editor + panel edits into `src-app/desktop/ui/`; verify `npm run check` in both `ui` and `desktop/ui`.

## Files to touch

New (backend):
- `src-app/server/src/modules/chat/core/export.rs`

Edited (backend):
- `src-app/server/src/modules/file/handlers/versions.rs` (`append_version`)
- `src-app/server/src/modules/file/handlers/export.rs` (new file, or extend `download.rs`)
- `src-app/server/src/modules/file/routes.rs` (`POST /files/{id}/versions`, `GET /files/{id}/export`)
- `src-app/server/src/modules/file/utils/pandoc.rs` (`convert_to_docx`)
- `src-app/server/src/modules/file/repository.rs` + `available_files.rs` (deliverables query)
- `src-app/server/src/modules/chat/core/routes.rs` + handlers (conversation export + deliverables)

New (frontend, mirrored in `src-app/desktop/ui/`):
- `src-app/ui/src/components/kit/editor/KitMarkdownEditor.tsx` (+ lazy wrapper `LazyMarkdownEditor.tsx`)
- `src-app/ui/src/components/kit/editor/*` (adopted Plate shadcn components: toolbar, nodes)
- `src-app/ui/src/modules/file/utils/markdownRoundtrip.ts` (ITEM-7)
- `src-app/ui/src/modules/file/components/FileEditBody.tsx` (edit-mode host)

Edited (frontend, mirrored in `src-app/desktop/ui/`):
- `src-app/ui/package.json` + `src-app/desktop/ui/package.json` (Plate deps; syncpack-aligned)
- `src-app/ui/src/modules/file/components/FilePanel.tsx` (view/edit toggle + export menu)
- `src-app/ui/src/modules/file/components/FileVersionBar.tsx` (reflect user-saved head)
- `src-app/ui/src/modules/file/stores/{File,FileVersions}.store.ts` (`appendVersion` action)
- `src-app/ui/src/modules/file/chat-extension/extension.tsx` (auto-open)
- `src-app/ui/src/modules/chat/core/components/*` (chat-header "Export conversation")
- `src-app/ui/src/dev/gallery/*` + `STATE_MATRIX`, kit manifest + testid registry
- `src-app/ui/src/api-client/types.ts` + `openapi/openapi.json` (regen) + `desktop/ui/**` mirrors

Tests: `src-app/server/tests/file/*.rs`, `src-app/server/tests/chat/*export*.rs`,
in-source `#[cfg(test)]`, `src-app/ui/**/*.test.ts(x)` (round-trip),
`src-app/ui/tests/e2e/14-artifacts/*.spec.ts`.

## Patterns to follow

- **User append-version** (ITEM-1): mirror `versions::restore_version` (ownership +
  `commit_new_version` + `publish_file_changed`), bytes from the request.
- **Pandoc / export / download** (ITEM-2/3/4): reuse `find_pandoc`/`convert_to_pdf`,
  copy its `spawn_blocking`+timeout shape for `convert_to_docx`; stream via
  `content_disposition` + `workspace_export`'s `Response::builder()`.
- **Conversation serialization** (ITEM-4): extend `summarizer::message_to_summarizable`.
- **Deliverables query** (ITEM-5): mirror `available_files::resolve_available_files`
  ownership join.
- **Editor adoption** (ITEM-6/7/8): the `LazyStreamdown` lazy-load idiom for the editor
  bundle; the shadcn component-ownership model already used by the kit for adopting
  Plate's components; `CoreMemoryBlocksEditor` for the edit→save→REST flow; the
  existing `file` panel pointer pattern (`{fileId,version?}`) for the panel.
- **Auto-open** (ITEM-9): the literature `tool_result` → `displayInRightPanel` pattern.
- **Design system** (ITEM-11): `shadcn-component-discovery` (reuse-first) +
  `shadcn-component-review`; DESIGN_SYSTEM.md tokens; kit manifest + testid registry.
- **OpenAPI + desktop** (ITEM-12): `just openapi-regen` both workspaces; `npm run check` both.

## Superseded

v1 (a new `artifacts` table + MCP + permission + migrations 132/133) was dropped as
redundant with `files_mcp`/`file_versions`/the file panel. v2 used a plain `Textarea`
edit-mode; **v3 replaces that with a real WYSIWYG editor (Plate)** per the requirement
that direct editing be rich, matching ChatGPT Canvas / Gemini Canvas / Claude Artifacts
(all of which allow direct WYSIWYG editing, versioned on save). See DEC-6.

# PLAN — artifacts-deliverables (v2, re-scoped)

**Goal:** let users get FINISHED WORK OUT of the app. Today output is trapped in
the chat transcript.

**Why this plan is smaller than a naive "build an artifacts subsystem":** a direct
read of the current code shows the agent-authoring + versioning + panel-viewing
substrate **already exists** and must be reused, not paralleled:

- `files_mcp` already exposes `create_file` ("author a document the user can view
  and revise across turns"), `edit_file` (unique `old_str`→`new_str`, **appends a
  restorable version**), `edit_file_lines`, `rewrite_file`, and `convert_document`
  (markdown→PDF→saved file). This IS the "create/update/rewrite artifact" surface.
- `file_versions` + `file::versioning::commit_new_version` already give append-only,
  content-addressed, restorable versioning; `SyncEntity::File` already syncs it.
- A model-returned file already renders inline at the `tool_result` and has an
  **"Open in side panel"** affordance that opens the `file` right-panel with a
  **version bar** (list + restore). `create_file` already stamps `source_message_id`,
  so a conversation's model-authored files are **derivable** (no linking table needed).

**So the feature is the missing top layer on top of that substrate**, confirmed by
the code:

1. **The user cannot edit.** The file panel is explicitly **read-only** (viewers are
   head-bound; the version bar is view + restore only), and there is **no user-facing
   REST endpoint to append content**. → the true "co-**edit**" gap.
2. **A created file doesn't feel like a deliverable.** It opens on click, not
   automatically, and is just another attachment.
3. **Export is thin.** `convert_document` is markdown→PDF only and model-tool-only —
   no **docx**, no **user-facing export button**, no **whole-conversation** export.

This plan adds exactly those three, reusing everything else. **No new MCP module, no
new permission, no new table, zero migrations.**

## Items

- **ITEM-1**: User append-version REST — `POST /api/files/{id}/versions` (JSON `{content}` for text; multipart for binary) → `file::versioning::commit_new_version(created_by='user', source_message_id=None)`; ownership-scoped (cross-user → 404); gated by the same permission that guards `restore_version`; emits `SyncEntity::File`. This is the user half of co-edit — the one backend primitive that is genuinely absent today.
- **ITEM-2**: `file::utils::pandoc::convert_to_docx(input_path, output_path)` — sibling of `convert_to_pdf`: `pandoc <in> -o <out.docx>` (native docx writer, no `--pdf-engine`), same `spawn_blocking` + `tokio::time::timeout(PANDOC_TIMEOUT)` shape.
- **ITEM-3**: User-facing file export endpoint `GET /api/files/{id}/export?format=md|docx|pdf` — loads the file's head text; `md` = raw bytes, `docx`/`pdf` = pandoc via ITEM-2 / `convert_to_pdf`; streamed attachment via `file::handlers::download::content_disposition`; ownership-scoped. (Distinct from the model-only `convert_document`: this is a user download in a chosen format, not a save-to-store.)
- **ITEM-4**: Conversation→markdown serializer + endpoint — `modules/chat/core/export.rs` renders a conversation's messages to one markdown string (`## User`/`## Assistant` headers; text as prose; `tool_use`/`tool_result`/`thinking`/code as fenced blocks; `file_attachment`/`image` as links; extends `summarization::summarizer::message_to_summarizable`'s block handling). `GET /api/conversations/{id}/export?format=md|docx|pdf` streams it as an attachment; gated `conversations::read` + ownership.
- **ITEM-5**: Derived "deliverables in this conversation" list — `GET /api/conversations/{id}/deliverables` returns the files the model authored in the conversation (query `files` joined via `file_versions.source_message_id` ∈ the conversation's messages, `created_by IN ('mcp','llm')`), reusing the ownership scoping of `available_files`. No new table. Backs the "find my deliverable again" UX + a panel re-open list.
- **ITEM-6**: Editable file panel (frontend) — add an **Edit** mode to the existing `file` panel (`FilePanel.tsx` + a text edit-body) for text types (`markdown|code|csv|text`): a `Textarea` source editor + explicit **Save** → a `Stores.FileVersions` (or `Stores.File`) `appendVersion` action calling ITEM-1; on save the `FileVersionBar` shows the new head. Non-text types (pdf/image/office) stay view-only. Mirrors `CoreMemoryBlocksEditor`'s edit→save→REST idiom; reuses the existing viewer registry for the view mode.
- **ITEM-7**: Auto-open the canvas on model authoring (frontend) — in the file chat-extension's `tool_result` renderer, when a `create_file`/`rewrite_file`/`edit_file` result first appears, call `displayInRightPanel({ type:'file', data:{ fileId } })` so the deliverable surfaces as a canvas immediately; the existing inline preview + "Open in side panel" remains for manual re-open of older ones.
- **ITEM-8**: Export affordances (frontend) — an **"Export as… (md/docx/pdf)"** menu in the file-panel header (hits ITEM-3) and an **"Export conversation"** menu in the chat header (hits ITEM-4), each downloading via a `Blob`/`<a download>` or direct navigation to the attachment endpoint. Placed in existing header-action slots.
- **ITEM-9**: Gallery + state-matrix coverage — register `gallery`/overlay cells for the file panel's new **edit** and **export-menu** states (view / edit / saving / error) so `check:state-matrix` + `check:gallery-coverage` + `gate:ui` (runtime-health, AA contrast, Layer A/axe) pass in both `ui` and `desktop/ui`.
- **ITEM-10**: OpenAPI + TS types regen for BOTH workspaces (`just openapi-regen`) — the new endpoints (append-version, file export, conversation export, deliverables) flow into `Api.*` + `types.ts`; mirror the frontend edits into `src-app/desktop/ui/` and verify `npm run check` in both `ui` and `desktop/ui`.

## Files to touch

New (backend):
- `src-app/server/src/modules/chat/core/export.rs` (conversation→markdown serializer + export handler)

Edited (backend):
- `src-app/server/src/modules/file/handlers/versions.rs` (add `append_version` handler)
- `src-app/server/src/modules/file/handlers/download.rs` or a new `handlers/export.rs` (file export handler)
- `src-app/server/src/modules/file/routes.rs` (`POST /files/{id}/versions`, `GET /files/{id}/export`)
- `src-app/server/src/modules/file/utils/pandoc.rs` (`convert_to_docx`)
- `src-app/server/src/modules/file/repository.rs` (deliverables query) + `src-app/server/src/modules/file/available_files.rs` (reuse scoping)
- `src-app/server/src/modules/chat/core/routes.rs` + `handlers` (conversation export + deliverables routes)

New (frontend, mirrored in `src-app/desktop/ui/`):
- `src-app/ui/src/modules/file/components/FileEditBody.tsx` (textarea edit body)

Edited (frontend, mirrored in `src-app/desktop/ui/`):
- `src-app/ui/src/modules/file/components/FilePanel.tsx` (view/edit toggle + export menu)
- `src-app/ui/src/modules/file/components/FileVersionBar.tsx` (reflect user-saved head)
- `src-app/ui/src/modules/file/stores/File.store.ts` and/or `FileVersions.store.ts` (`appendVersion` action)
- `src-app/ui/src/modules/file/chat-extension/extension.tsx` (auto-open on model authoring)
- `src-app/ui/src/modules/chat/core/components/*` (chat-header "Export conversation" menu)
- `src-app/ui/src/dev/gallery/*` + `STATE_MATRIX` (new panel states)
- `src-app/ui/src/api-client/types.ts` + `src-app/ui/openapi/openapi.json` (regen)
- `src-app/desktop/ui/**` mirrors + its own regen

Tests: `src-app/server/tests/file/*.rs`, `src-app/server/tests/chat/*export*.rs`,
in-source `#[cfg(test)]` in the touched backend files, `src-app/ui/tests/e2e/14-artifacts/*.spec.ts`.

## Patterns to follow

- **User append-version endpoint** (ITEM-1): mirror `file::handlers::versions::restore_version`
  (ownership check + `commit_new_version` + `publish_file_changed` sync) — it is the
  closest existing write path; the only difference is the bytes come from the request,
  not a prior version.
- **Pandoc reuse** (ITEM-2/3/4): `file::utils::pandoc::{find_pandoc,convert_to_pdf}` as-is;
  `convert_to_docx` copies `convert_to_pdf`'s `spawn_blocking`+timeout shape; the
  `files_mcp::handlers::convert_document` markdown→PDF path is the closest template.
- **Streamed download** (ITEM-3/4): `file::handlers::download::content_disposition`
  (RFC-5987) + `workflow::handlers::dev::workspace_export`'s `Response::builder()` attachment.
- **Conversation serialization** (ITEM-4): extend
  `summarization::engine::summarizer::{message_to_summarizable,build_transcript}`
  block handling (today text-only) into a faithful markdown renderer.
- **Deliverables query** (ITEM-5): mirror `file::available_files::resolve_available_files`
  ownership scoping (the join through `conversations → branches → branch_messages`).
- **Editable panel** (ITEM-6): the edit→save→REST idiom of
  `modules/memory/components/CoreMemoryBlocksEditor.tsx`; reuse the existing
  `file/viewers/*` registry for the view mode and `FileVersionBar` for history.
- **Auto-open + tool_result card** (ITEM-7): the literature module's `tool_result`
  renderer → `displayInRightPanel` pattern, applied to the file chat-extension.
- **Store action + sync** (ITEM-6): `defineStore` + `on('sync:file', …)` self-gated
  refetch (mirror `Citations.store` / the existing `File`/`FileVersions` stores).
- **OpenAPI + desktop parity** (ITEM-10): `just openapi-regen` regenerates BOTH
  `ui/` and `desktop/ui/`; verify `npm run check` in both.

## Superseded

The prior v1 plan proposed a new `artifacts` table + a new `artifacts` built-in MCP
(`create/update/rewrite/get_artifact`) + a new `artifacts::use` permission +
migrations 132/133. **All dropped as redundant** — `files_mcp` already provides that
agent surface with identical `old_str`/`new_str` semantics, `file_versions` already
provides versioning, and the `file` panel already provides viewing. See DEC-1.

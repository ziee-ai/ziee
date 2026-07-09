# PLAN — artifacts-deliverables

**Goal:** let users get FINISHED WORK OUT of the app. Today output is trapped in
the chat transcript. This feature adds (a) a persistent, versioned, co-editable
**artifact / canvas** — a named deliverable (report / table / code) that lives
beside the chat, that the agent and user co-edit across turns — and (b) **export**
of a conversation *or* an artifact into real handoff formats (markdown / docx /
pdf).

**Thesis (grounded in a substrate study of the repo):** the file-store already
provides versioning (`file_versions`, `commit_new_version`, append-only restore,
content-addressed no-op), generic viewers (markdown / tabular / pdf / image /
code), cross-device sync (`SyncEntity::File`), an agent write surface
(`files_mcp` `create_file`/`edit_file`/`rewrite_file`), and embedded
pandoc+typst export (`convert_to_pdf`). So an artifact is **not a parallel
subsystem** — it is a file-store `File` *designated* as a conversation
deliverable by a thin link row, surfaced in an editable right-panel canvas, with
the genuinely-missing pieces built on top: the deliverable designation + a
first-class agent tool surface, a user-editable canvas, and conversation/artifact
export.

## Items

- **ITEM-1**: Migration `132` — `artifacts` link table: `id UUID PK`, `conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE`, `file_id UUID NOT NULL REFERENCES files(id) ON DELETE CASCADE`, `title TEXT NOT NULL`, `artifact_type TEXT NOT NULL` (`markdown|code|csv|html`), `created_at`/`updated_at TIMESTAMPTZ`. Indexes on `conversation_id` and a UNIQUE on `file_id` (one artifact per file). Owner is derived via the conversation's `user_id` (no separate owner column, no ACL — matches the single-owner reality of `files`/`projects`).
- **ITEM-2**: Migration `133` — grant `artifacts::use` to the default `Users` group with the idempotent `DO $$ … FOREACH perm IN ARRAY[…] … array_append …` pattern (verbatim clone of migration `104`/`98`). Admins inherit via the `*` wildcard.
- **ITEM-3**: Backend module scaffold `modules/artifacts/{mod.rs,models.rs,repository.rs,permissions.rs,routes.rs}` — `Artifact` DTO + `ArtifactType` enum + `ArtifactVersionRef` (projection of the file's head), `permissions.rs` `ArtifactsUse`/`ArtifactsManage` (`PermissionCheck`), `repository.rs` `create`/`get`/`list_by_conversation`/`update_row`/`delete` all **ownership-scoped via the conversation join** (cross-user → not found).
- **ITEM-4**: Built-in `artifacts` MCP server — `modules/artifacts/mod.rs` `artifacts_server_id() = Uuid::new_v5(NAMESPACE_URL, b"artifacts.ziee.internal")`; `#[distributed_slice(MODULE_ENTRIES)]` init spawns `repository::upsert_builtin_server(id, loopback /api/artifacts/mcp)` (clone of `citations`); `routes.rs` mounts `POST /artifacts/mcp`; `handlers.rs::jsonrpc_handler` gated `RequirePermissions<(ArtifactsUse,)>` implementing `initialize`/`ping`/`tools/list`/`tools/call`; `tools.rs::tool_list()` schemas for `create_artifact`/`update_artifact`/`rewrite_artifact`/`get_artifact`.
- **ITEM-5**: Artifact content ops delegate to the file engine — `create_artifact(title, artifact_type, content)` → `file::ingest::ingest_bytes(created_by='llm')` + `repository::create` row, returns `{artifact_id, file_id, version}`; `update_artifact(artifact_id, old_str, new_str)` → exact-literal-**replace-once** on the head content (reuse `files_mcp::edits` str-replace semantics) → `file::versioning::commit_new_version`; `rewrite_artifact(artifact_id, content)` → `commit_new_version` full replace; `get_artifact(artifact_id)` → returns the head content (the model re-reads the latest, incl. user edits). Every result: `{content:[{type:text, text:<summary>}], structuredContent:{artifact_id,file_id,version,type,title}}`. A stale `old_str` (no unique match) returns `is_error` telling the model to `get_artifact` and retry — fail-loud, never silent.
- **ITEM-6**: The MCP wiring edits + chat extension — `modules/artifacts/chat_extension/{mod.rs,extension.rs,artifacts.rs}` (order **23**, `ATTACH_FLAG="attach_artifacts_mcp"`, set in `before_llm_call` when the model is tool-capable); **two** `mcp/chat_extension/mcp.rs` edits: add the flag branch to `auto_attach_builtin_ids` and the id to `is_builtin_server_id` (approval-bypass — artifact writes touch the caller's own append-only-versioned data). No `is_trusted_resource_emitter` edit (tools return `is_saved:true` structuredContent, never `ziee://` host paths).
- **ITEM-7**: Sync entity `SyncEntity::Artifact` (owner-scoped) — add the variant in `modules/sync/event.rs`; emit `Create`/`Update`/`Delete` from the artifacts ops (`origin`-aware for user-driven REST, `origin=None` for model/tool-driven) to `Audience::owner(conversation_owner)`. The frontend refetches via `sync:artifact`.
- **ITEM-8**: User-facing artifacts REST (`modules/artifacts/routes.rs` + handlers) — `GET /api/conversations/{id}/artifacts` (list), `GET /api/artifacts/{id}`, `PUT /api/artifacts/{id}` (title and/or content → appends a version with `created_by='user'` via `commit_new_version`; the user side of co-edit), `DELETE /api/artifacts/{id}`. All gated `ArtifactsUse` + ownership-scoped (cross-user → 404).
- **ITEM-9**: `file::utils::pandoc::convert_to_docx(input_path, output_path)` — a sibling of `convert_to_pdf`: `pandoc <in> -o <out.docx>` (native docx writer, no `--pdf-engine`), same `spawn_blocking` + `tokio::time::timeout(PANDOC_TIMEOUT)` hardening.
- **ITEM-10**: Conversation→markdown serializer (`modules/chat/core/export.rs`) — render a conversation's messages to a single markdown string: `## User` / `## Assistant` role headers, text blocks as markdown, `tool_use`/`tool_result`/thinking/code as fenced blocks, `file_attachment`/`image` as links. Extends the block-filtering approach of `summarization::engine::summarizer::message_to_summarizable` (which today drops non-text blocks) into a lossless-ish renderer.
- **ITEM-11**: Conversation export endpoint `GET /api/conversations/{id}/export?format=md|docx|pdf` (chat module) — gated `conversations::read` + ownership; `md` = raw serializer output; `docx`/`pdf` = serializer → temp `.md` → `convert_to_docx`/`convert_to_pdf`; streamed attachment via `file::handlers::download::content_disposition` (RFC-5987) mirroring `workflow::handlers::dev::workspace_export`.
- **ITEM-12**: Artifact export endpoint `GET /api/artifacts/{id}/export?format=md|docx|pdf` (artifacts module) — gated `ArtifactsUse` + ownership; loads the file's head content, `md` = raw bytes, `docx`/`pdf` = pandoc via ITEM-9 / `convert_to_pdf`; same streamed-attachment pattern.
- **ITEM-13**: Frontend `artifact` module scaffold — `src-app/ui/src/modules/artifact/{module.tsx,types.ts}` + `stores/Artifact.store.ts` (`defineStore('Artifact', …)`: `listByConversation`/`get`/`save`(PUT)/`remove`/`exportArtifact`; `init` subscribes `on('sync:artifact', reload)` + `on('sync:reconnect', …)` with a `hasPermissionNow` self-gate); `types.ts` declares `PanelRendererMap { artifact: { artifactId: string; fileId: string; version?: number } }` (pointer pattern, server-fetched — not inline localStorage).
- **ITEM-14**: `ArtifactCanvasPanel` component (`components/ArtifactCanvasPanel.tsx`) — **view mode** reuses the file viewer registry (`resolveFileViewer` body for `fileId`); **edit mode** for text types (`markdown|code|csv`) is a `Textarea` source editor with an explicit Save → `Stores.Artifact.save` (mirrors `CoreMemoryBlocksEditor`); reuses `FileVersionBar` for version history + restore; header **Export as… (md/docx/pdf)** menu hitting ITEM-12. `html` renders view-only.
- **ITEM-15**: Artifact chat-extension UI (`src-app/ui/src/modules/artifact/chat-extension/extension.tsx`) — `registerPanelRenderer('artifact', { icon, component: ArtifactCanvasPanel })` + a `tool_result` content renderer (`ArtifactToolResultCard`) for `create_artifact`/`update_artifact`/`rewrite_artifact` blocks that shows an inline "Open canvas" card and **auto-opens** the panel on a `create_artifact` result via `displayInRightPanel({ type:'artifact', data:{ artifactId, fileId } })`.
- **ITEM-16**: Conversation export affordance — a chat-header menu ("Export conversation" → md / docx / pdf) that downloads via `GET /api/conversations/{id}/export` (client `Blob`/`<a download>` or direct navigation to the attachment endpoint), placed in the existing chat header actions.
- **ITEM-17**: Gallery + state-matrix coverage — register `gallery-page`/overlay cells for `ArtifactCanvasPanel` (view / edit / empty / error) and `ArtifactToolResultCard`, so `check:state-matrix` + `check:gallery-coverage` + `gate:ui` (runtime-health, AA contrast, Layer A/axe) pass in both `ui` and `desktop/ui`.
- **ITEM-18**: OpenAPI + TS types regen for BOTH workspaces (`just openapi-regen`) — new endpoints, `Artifact`/`ArtifactType` schemas, `SyncEntity::Artifact`, `artifacts::use` in `Permissions`; **mirror** the `artifact` frontend module into `src-app/desktop/ui/` (desktop embeds the server, so the module is live there) and verify `npm run check` in both `ui` and `desktop/ui`.

## Files to touch

New (backend):
- `src-app/server/migrations/00000000000132_create_artifacts_table.sql`
- `src-app/server/migrations/00000000000133_grant_artifacts_permissions_to_users.sql`
- `src-app/server/src/modules/artifacts/mod.rs`
- `src-app/server/src/modules/artifacts/models.rs`
- `src-app/server/src/modules/artifacts/repository.rs`
- `src-app/server/src/modules/artifacts/permissions.rs`
- `src-app/server/src/modules/artifacts/routes.rs`
- `src-app/server/src/modules/artifacts/handlers.rs`
- `src-app/server/src/modules/artifacts/tools.rs`
- `src-app/server/src/modules/artifacts/content.rs` (create/update/rewrite/get content ops)
- `src-app/server/src/modules/artifacts/chat_extension/{mod.rs,extension.rs,artifacts.rs}`
- `src-app/server/src/modules/chat/core/export.rs` (conversation→markdown serializer + export handler)

Edited (backend):
- `src-app/server/src/modules/mod.rs` (register `artifacts` module)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (auto_attach + is_builtin edits)
- `src-app/server/src/modules/sync/event.rs` (`SyncEntity::Artifact`)
- `src-app/server/src/modules/file/utils/pandoc.rs` (`convert_to_docx`)
- `src-app/server/src/modules/chat/core/routes.rs` (conversation export route)
- `src-app/server/src/openapi/*` (spec + `emit_ts` picks up new types automatically)

New (frontend, mirrored in `src-app/desktop/ui/`):
- `src-app/ui/src/modules/artifact/module.tsx`
- `src-app/ui/src/modules/artifact/types.ts`
- `src-app/ui/src/modules/artifact/stores/Artifact.store.ts`
- `src-app/ui/src/modules/artifact/components/ArtifactCanvasPanel.tsx`
- `src-app/ui/src/modules/artifact/components/ArtifactToolResultCard.tsx`
- `src-app/ui/src/modules/artifact/chat-extension/extension.tsx`

Edited (frontend):
- `src-app/ui/src/modules/chat/core/components/*` (chat-header export menu — ITEM-16)
- `src-app/ui/src/dev/gallery/*` + `STATE_MATRIX` (ITEM-17)
- `src-app/ui/src/api-client/types.ts` + `src-app/ui/openapi/openapi.json` (regen — ITEM-18)
- `src-app/desktop/ui/**` mirrors of the above + its own regen

Tests: `src-app/server/tests/artifacts/*.rs`, in-source `#[cfg(test)]` in the new
backend files, `src-app/ui/tests/e2e/14-artifacts/*.spec.ts`.

## Patterns to follow

- **Built-in MCP module** (ITEM-3/4/5/6): mirror `modules/citations/**` (and
  `modules/web_search/**`) file-for-file — `mod.rs` deterministic-id +
  `upsert_builtin_server`, `handlers.rs::jsonrpc_handler` gated on one permission
  extractor, `tools.rs::tool_list()`, `chat_extension/` `ATTACH_FLAG`. The two
  `mcp/chat_extension/mcp.rs` edit sites are mandatory (silent failure otherwise).
- **Content edit engine** (ITEM-5): reuse `modules/files_mcp/edits.rs`
  str-replace-once semantics + `file::versioning::commit_new_version` (already
  content-addressed no-op + append-only). Do NOT reinvent versioning.
- **Link table + ownership** (ITEM-1/3): mirror `project_files` /
  `project_bibliography` (M:N link over a real entity, `ON DELETE CASCADE`,
  ownership via the parent's `user_id`).
- **Permission grant migration** (ITEM-2): verbatim clone of migration
  `00000000000104_grant_citations_permissions_to_users.sql`.
- **Sync entity** (ITEM-7): mirror `SyncEntity::File`/`BibliographyEntry`
  owner-scoped emit + the store self-gating `sync:<entity>` refetch
  (`mcp/stores/McpServer.store.ts` no-403 rule).
- **Pandoc reuse** (ITEM-9/11/12): `file::utils::pandoc::{find_pandoc,convert_to_pdf}`
  as-is; `convert_to_docx` copies `convert_to_pdf`'s `spawn_blocking`+timeout shape;
  the `files_mcp::handlers::convert_document` markdown→PDF→file-store path is the
  closest existing template.
- **Streamed download** (ITEM-11/12): `file::handlers::download::content_disposition`
  (RFC-5987) + `workflow::handlers::dev::workspace_export`'s `Response::builder()`
  attachment pattern.
- **Frontend module + panel** (ITEM-13/14/15): mirror `modules/literature/**`
  (panel renderer + `tool_result` card → `displayInRightPanel`) and the `file`
  panel's pointer-`{fileId}`-then-`Stores.File`-fetch pattern; edit-mode textarea
  mirrors `modules/memory/components/CoreMemoryBlocksEditor.tsx`; store mirrors
  `modules/citations/stores/Citations.store.ts` (`defineStore` + `on('sync:…')`).
- **Conversation serialization** (ITEM-10): extend
  `summarization::engine::summarizer::{message_to_summarizable,build_transcript}`
  block-handling (today text-only) into a full markdown renderer.
- **OpenAPI + desktop parity** (ITEM-18): `just openapi-regen` regenerates BOTH
  `ui/` and `desktop/ui/`; verify `npm run check` in both (per repo convention).

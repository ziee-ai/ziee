# PLAN_AUDIT — artifacts-deliverables (v2)

Audit of the re-scoped PLAN.md against the codebase. The re-scope itself is the
biggest audit outcome: a code read proved the agent-authoring + versioning +
panel-viewing substrate already exists (`files_mcp`, `file_versions`, the `file`
panel), so v1's new table / new MCP / new permission were redundant and were
dropped. What remains is the genuinely-missing top layer.

## Breakage risk

- **All backend changes are additive.** ITEM-1 (`POST /files/{id}/versions`) is a new
  route; ITEM-2 (`convert_to_docx`) is a new fn; ITEM-3/4/5 are new routes/handlers;
  ITEM-4 adds a new `chat/core/export.rs`. No existing signature changes.
- **ITEM-1 reuses `commit_new_version`**, which is already the append point for the
  `files_mcp` edit tools and code-sandbox version-back — so a user-appended version is
  indistinguishable from a model-appended one downstream (same head-mirror, same sync
  emit). Risk: a user save racing a model `edit_file` on the same file — mitigated
  because `append_version` already takes a `SELECT … FOR UPDATE` row lock (serializes
  writers; last writer wins as a new head, nothing lost since all versions are kept).
- **ITEM-6 edits the shared `file` panel**, so the edit affordance appears for *every*
  file, not only model-authored ones. That is acceptable (all files are single-owner
  and editable by their owner) but must be gated to text types (pdf/image/office have
  no sane source editor) — an explicit acceptance point + a unit test on the
  editable-type predicate.
- **ITEM-7 auto-open** could surprise a user if it fires on every incidental
  `edit_file`; mitigated by opening only on the *first* appearance of a create/rewrite
  result per file and reusing the existing (already-non-surprising) panel host.
- **Frontend**: additive panel states + a new edit body; `rehydrateTabs` already drops
  unknown panel types, and the `file` panel type is unchanged (still `{fileId,version?}`),
  so no persisted-tab migration is needed.

## Pattern conformance

- **ITEM-1** mirrors `versions::restore_version` (ownership + `commit_new_version` +
  `publish_file_changed`) — conformant; the only delta is request-supplied bytes.
- **ITEM-2/3/4** reuse `find_pandoc`/`convert_to_pdf` + `content_disposition` +
  `workspace_export`'s attachment builder — conformant.
- **ITEM-4** extends `summarizer::message_to_summarizable`'s block handling — conformant.
- **ITEM-5** mirrors `available_files::resolve_available_files` ownership join — conformant.
- **ITEM-6** mirrors `CoreMemoryBlocksEditor` edit→save→REST and reuses the viewer
  registry + `FileVersionBar` — conformant; the "no editor dependency" `Textarea`
  choice is deliberate (no editor primitive exists in the app — DEC-6).
- **ITEM-7** mirrors the literature `tool_result` → `displayInRightPanel` pattern —
  conformant.
- **ITEM-10** follows the both-workspaces `just openapi-regen` convention — conformant.

## Migration collisions

- **None — this feature introduces NO migration.** Versioning (`file_versions`),
  permissions (existing file + `conversations::read`), and the conversation↔file
  association (`file_versions.source_message_id`) all already exist. Verified: no new
  table, no permission grant, no `created_by` vocabulary change (model files are already
  `'mcp'`; user saves reuse `'user'`). `ls migrations/` tail `131` is untouched.

## OpenAPI regen

- **Required (endpoints only, no new domain types).** Four new endpoints
  (`POST /files/{id}/versions`, `GET /files/{id}/export`, `GET /conversations/{id}/export`,
  `GET /conversations/{id}/deliverables`) flow through `just openapi-regen` into the
  `Api.*` client + `types.ts`, in BOTH the server `ui/` and desktop `desktop/ui/`
  workspaces. The `emit_ts` golden-parity test enforces regen. The `deliverables`
  response reuses the existing `File`/`FileMetadata` schema (no new schema), so the
  diff is endpoint surface, not type surface. `SyncEntity` is unchanged (reuses
  `File`) — no `sync:*` vocabulary change.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — direct `restore_version` mirror; the genuinely-missing user-write primitive; row-lock already serializes concurrent writers.
- **ITEM-2** — verdict: PASS — `convert_to_docx` copies the proven `convert_to_pdf` shape; docx is a native pandoc writer (smoke-tested against the embedded 3.7 binary).
- **ITEM-3** — verdict: PASS — user download in a chosen format; reuses pandoc + `content_disposition`; distinct from the model-only `convert_document`.
- **ITEM-4** — verdict: CONCERN — the conversation→markdown serializer is genuinely new and must handle every `MessageContentData` variant faithfully (not the lossy `role: text` transcript); bounded and covered by a per-variant unit test.
- **ITEM-5** — verdict: CONCERN — deriving deliverables via `file_versions.source_message_id` ∈ conversation must reuse the exact `available_files` ownership join or risk a cross-user leak; covered by an ownership integration test.
- **ITEM-6** — verdict: CONCERN — edits the shared `file` panel; must gate edit-mode to text types and must render an arbitrary `fileId` editable outside the file drawer; covered by a unit predicate test + the e2e edit flow.
- **ITEM-7** — verdict: PASS — literature `tool_result`→`displayInRightPanel` mirror; first-appearance-only auto-open avoids surprise.
- **ITEM-8** — verdict: PASS — small menus in existing header slots hitting ITEM-3/4.
- **ITEM-9** — verdict: CONCERN — new edit/saving/error render states MUST have gallery cells or `check:state-matrix`/`gate:ui` fail; budgeted as its own item.
- **ITEM-10** — verdict: CONCERN — regen + desktop mirror + `npm run check` in both workspaces are hard gates; endpoints-only (no new type) keeps the surface small but the golden-parity test still gates it.

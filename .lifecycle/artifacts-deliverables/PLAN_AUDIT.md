# PLAN_AUDIT — artifacts-deliverables

Audit of PLAN.md **against the codebase** (five substrate explorations: file-store,
built-in-MCP anatomy, right-panel system, pandoc/export, external design patterns).

## Breakage risk

- **No existing caller is broken.** Every backend change is additive: a new module
  (`modules/artifacts`), two new migrations, a new `SyncEntity` variant, a new
  `pandoc::convert_to_docx` sibling fn, and two new REST route groups. No existing
  function signature changes.
- **The two `mcp/chat_extension/mcp.rs` edits** (ITEM-6) append to
  `auto_attach_builtin_ids` + `is_builtin_server_id`; both are additive `if`/`||`
  arms — verified against the current bodies (each built-in is one independent
  arm). Forgetting either is a *silent* failure (server registers, model never
  sees tools / stalls on approval), so both are explicit ITEM-6 acceptance points,
  covered by a unit test (TEST for the mcp.rs branches).
- **`SyncEntity::Artifact`** (ITEM-7): adding a variant does NOT force an audience
  at compile time — the risk is emitting to too broad an audience. Mitigated by the
  plan's explicit `Audience::owner(conversation_owner)` at every emit site (never
  `everyone()`), matching the read-perm (`ArtifactsUse` + ownership) the refetch
  endpoint enforces.
- **Cascade semantics** (ITEM-1): `artifacts.conversation_id … ON DELETE CASCADE`
  (conversations is `user_id … ON DELETE CASCADE`) means deleting a conversation
  removes its artifact *rows*; the underlying `files` rows survive (owned by the
  user) — identical to `project_files`' documented behavior. `file_id … ON DELETE
  CASCADE` means deleting the file removes the artifact row. No dangling FKs.
- **Frontend**: a new panel type + a new module are additive; the panel registry
  (`registerPanelRenderer`) and `PanelRendererMap` declaration-merge are the
  designed extension points. `rehydrateTabs` already drops tabs whose renderer is
  unregistered, so a persisted `artifact` tab in a build without the module
  degrades gracefully.
- **Real-LLM e2e risk** (ITEM-15 flow): auto-attached built-in MCP tools only reach
  a model marked tool-capable — the e2e must use a tool-capable model
  (`capabilities.tools=true`) or the model hallucinates the call. Captured as a
  DECISION and a test precondition, not a code risk.

## Pattern conformance

- **ITEM-1/3** mirror `project_files`/`project_bibliography` (link table over a real
  entity; ownership via parent `user_id`) — conformant.
- **ITEM-2** is a verbatim clone of migration `104` (citations grant) — conformant.
- **ITEM-4/5/6** mirror `modules/citations/**` + `modules/web_search/**` (deterministic
  `Uuid::new_v5`, `upsert_builtin_server` `ON CONFLICT DO UPDATE`, one-permission-gated
  `jsonrpc_handler`, `tool_list()`, `ATTACH_FLAG`) and reuse `files_mcp::edits` +
  `file::versioning::commit_new_version` for content — conformant, reuse-first.
- **ITEM-7** mirrors `SyncEntity::File`/`BibliographyEntry` owner-scoped emit + the
  store self-gating refetch — conformant.
- **ITEM-9/11/12** reuse `find_pandoc`/`convert_to_pdf` and copy their
  `spawn_blocking`+`timeout` shape; download mirrors `content_disposition` +
  `workspace_export` — conformant.
- **ITEM-13/14/15** mirror `modules/literature/**` (panel + tool_result card →
  `displayInRightPanel`) + the `file` panel pointer pattern + `CoreMemoryBlocksEditor`
  edit-save idiom + `Citations.store` `defineStore`/`on('sync:…')` — conformant.
- **Gap acknowledged (ITEM-14 edit mode):** there is *no* existing rich-text/code
  editor in the app (no TipTap/ProseMirror/CodeMirror/Monaco/contentEditable). The
  plan therefore uses a markdown-source `Textarea` edit-mode — a deliberate
  reuse-of-existing-primitives choice, not a missed pattern (DEC-7).

## Migration collisions

- `ls migrations/` tail is `…131_rewrite_hub_ids_phibya_to_ziee_ai.sql`. The plan's
  `132`/`133` are free — no collision. Verified no `artifacts` table or
  `artifacts.ziee.internal` id exists anywhere today (grep clean). The name
  `save_as_artifact` exists in code_sandbox but is unrelated (a file-save action,
  not a table/module) — no namespace clash.
- Concurrent-worktree note: build.rs migrates a per-worktree `ziee_build_<key>` db,
  so adding migrations here does not disturb other worktrees.

## OpenAPI regen

- **Required.** New endpoints (`/api/conversations/{id}/artifacts`, `/api/artifacts/*`,
  `/api/conversations/{id}/export`, `/api/artifacts/{id}/export`), new schemas
  (`Artifact`, `ArtifactType`), new `SyncEntity::Artifact` (JsonSchema → the TS
  `SyncEntity` union → the `sync:artifact` EventBus key), and new `artifacts::use`
  in the `Permissions` set all flow through `just openapi-regen`. The `emit_ts`
  golden-parity test fails if a backend type change is not regenerated — so ITEM-18
  is a hard gate, and it must run for **both** binaries (server `ui/` + desktop
  `desktop/ui/`, which has its own types/Permissions/AppEvents).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — link table mirrors `project_files`; migration 132 free; cascade semantics verified against `conversations`/`files` FKs.
- **ITEM-2** — verdict: PASS — verbatim clone of migration 104; grants only to `Users` (admins via `*`).
- **ITEM-3** — verdict: PASS — module scaffold + ownership-via-conversation-join mirrors citations/project repositories.
- **ITEM-4** — verdict: PASS — deterministic-id + `upsert_builtin_server` + gated `jsonrpc_handler` is a direct citations/web_search mirror.
- **ITEM-5** — verdict: PASS — reuses `files_mcp::edits` str-replace-once + `commit_new_version` (append-only, content-addressed no-op); fail-loud stale-match is the correct turn-based-writer behavior.
- **ITEM-6** — verdict: CONCERN — the two `mcp.rs` edits are silent-failure-prone if missed; mitigated by explicit acceptance + a unit test on both branches (mirrors web_search's `mcp.rs` `#[cfg(test)]`).
- **ITEM-7** — verdict: CONCERN — new `SyncEntity` variant needs `just openapi-regen` (ITEM-18) so the `sync:artifact` TS key exists; audience must be `owner`-scoped at every emit (no compile-time enforcement).
- **ITEM-8** — verdict: PASS — REST CRUD + PUT-appends-version mirrors existing owner-scoped handlers; `PUT` is the user side of co-edit (no existing user REST appends a file version, so this endpoint is genuinely needed — DEC-19).
- **ITEM-9** — verdict: PASS — `convert_to_docx` copies `convert_to_pdf`'s proven shape; markdown→docx is a native pandoc writer (smoke-tested against the embedded 3.7 binary).
- **ITEM-10** — verdict: CONCERN — no full conversation→markdown renderer exists (only a lossy `role: text` transcript); the new serializer must handle every `MessageContentData` variant (`Text`/`Thinking`/`Image`/`FileAttachment`/`ToolUse`/`ToolResult`) — non-trivial but bounded, covered by a unit test per variant.
- **ITEM-11** — verdict: PASS — streamed-attachment export mirrors `workspace_export` + `content_disposition`; gated `conversations::read` + ownership.
- **ITEM-12** — verdict: PASS — artifact export reuses the file head content + pandoc + the same download pattern.
- **ITEM-13** — verdict: PASS — `defineStore` + `sync:artifact` self-gated refetch mirrors `Citations.store`; pointer-pattern `PanelRendererMap` mirrors the `file` panel.
- **ITEM-14** — verdict: CONCERN — no editor primitive exists; the `Textarea` edit-mode is a deliberate reuse choice (DEC-7). Reusing the file viewer registry for the view mode is the load-bearing simplification and must be verified to render an arbitrary `fileId` outside the file drawer.
- **ITEM-15** — verdict: PASS — panel renderer + `tool_result` card → `displayInRightPanel` is a direct literature mirror; auto-open on `create` is the one UX addition (DEC-9).
- **ITEM-16** — verdict: PASS — a chat-header export menu → download endpoint; small additive UI in an existing slot.
- **ITEM-17** — verdict: CONCERN — new conditional render states (view/edit/empty/error) MUST have gallery cells or `check:state-matrix`/`gate:ui` fail; budgeted explicitly as its own item.
- **ITEM-18** — verdict: CONCERN — regen touches BOTH workspaces + the desktop mirror; `emit_ts` golden-parity + `npm run check` in both are hard gates. Desktop embeds the server so no module blocklist entry is needed (DEC-15), but both api-clients must regen.

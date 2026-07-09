# PLAN_AUDIT — knowledge-base-retrieval

Audited against the codebase (worktree base = origin/main @ 9a6fb88c6, which
already contains migration `00000000000132_add_openrouter_provider_type.sql`).

## Breakage risk

- **Reuse of `file_rag::retrieval::semantic_search` is call-only** (signature
  `(&[Uuid], Uuid, &str, i64, &FileRagAdminSettings) -> SearchResult`), verified
  in `file_rag/retrieval.rs:85` and already consumed by
  `files_mcp/handlers.rs:411`. The KB module adds a *second* caller; it changes
  nothing in `file_rag`, so no existing caller breaks.
- **No change to `file_chunks` schema or the embedding column** → no dimension
  disruption, no `file_rag` reindex. KBs are pure grouping over existing chunks.
- **Upload-time indexing already fires** at `file/handlers/upload.rs:247`
  (`spawn_index`); the KB upload route reuses `file::ingest::ingest_bytes`, so
  chunks appear without new indexing code. Risk: a just-uploaded doc shows
  `pending` until the detached index task completes — handled by surfacing
  per-doc `index_status` (ITEM-5/15), not by blocking.
- **Chat-extension order 24** sits between `control_mcp` (22) and `memory` (25),
  inside the pre-MCP (30) window and free (only 8/10/15/20/22/25/26/27/28/29 are
  taken). No collision; must still run before MCP=30 (it does).
- **New MCP built-in id** is a fresh `Uuid::new_v5` namespace string
  (`knowledge_base.ziee.internal`) → no `mcp_servers` id collision.
- **`document_count` denormalization** risks drift vs `knowledge_base_documents`;
  mitigated by updating it inside the same transaction as add/remove (repository
  invariant, covered by TEST).

## Pattern conformance

- Module skeleton, deterministic id, loopback upsert, REST-vs-JSON-RPC split →
  conforms to `web_search`/`citations` (`mod.rs` `ModuleEntry`, `init()`,
  `repository::upsert_builtin_server`).
- M:N membership + cap + attach joins → conforms to `project_files` /
  `attach_file_capped` / `project_bibliography`.
- Permissions (`use`/`manage`) + grant-to-Users migration → conforms to
  `web_search`/`citations` (note: KB has BOTH admin-free per-user semantics like
  citations AND reuses the deployment-wide file_rag embedding config, so it needs
  **no** new settings singleton — a deliberate deviation, see OpenAPI section).
- Owner-scoping (foreign id → 404), `SyncOrigin`, `Audience::owner` →
  conforms to `project/handlers.rs`.
- Frontend module/list+detail/upload/project-extension → conforms to
  `projects` + `file/project-extension` + `citations/project-extension`.

## Migration collisions

- `ls migrations/ | tail -1` = `00000000000132_add_openrouter_provider_type.sql`.
  Next free numbers **133** and **134** — no collision. (CLAUDE.md's "highest =
  131" is stale; the base branch has 132.)
- Both new migrations are additive (new tables; idempotent `array_append` grant).
  No `ALTER` of existing tables, no data backfill. `CREATE EXTENSION IF NOT
  EXISTS vector` is **not** needed (no new vector column — chunks stay in
  `file_chunks`).
- `build.rs` re-runs migrations on a clean build; a `cargo clean` is required
  after adding the files (documented in CLAUDE.md's build-DB section) — captured
  as an implementation note, not an item.

## OpenAPI regen

- New REST request/response types (`KnowledgeBase`, `KnowledgeBaseDocument`,
  `Create/Update/AttachDocumentsRequest`, `KnowledgeSearchHit`) each derive
  `schemars::JsonSchema` and are declared in `*_docs(op)` response builders →
  they enter the spec automatically; **`just openapi-regen` is REQUIRED** and
  must run for BOTH `ui` and `desktop/ui` (ITEM-12). The `openapi::emit_ts`
  golden-parity test enforces that regen happened.
- The `SyncEntity` enum gains variants → the generated `SyncEntity` TS union
  (and thus the `sync:${entity}` EventBus keys) regenerate in both workspaces.
- The `/…/mcp` JSON-RPC route is a plain `.route` (excluded from OpenAPI by
  design) — no schema surface there.
- **No settings singleton table/route** (KB inherits `file_rag` embedding
  config), so there is no `*Settings` type/route/SyncEntity to add — smaller
  OpenAPI surface than web_search/lit_search. Verified this is intentional and
  consistent (citations likewise ships no settings singleton).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive tables mirroring `project_files`/`project_bibliography`; numbers 133 free; no vector column.
- **ITEM-2** — verdict: PASS — idempotent grant mirrors migration 104; strings must match ITEM-4 (cross-checked by a unit test).
- **ITEM-3** — verdict: PASS — mirrors `web_search/mod.rs` module entry + loopback upsert; init order 104 free (>65 so `mcp_servers` exists).
- **ITEM-4** — verdict: PASS — `PermissionCheck` impls mirror `web_search/permissions.rs`; test pins strings to migration.
- **ITEM-5** — verdict: PASS — CRUD/membership mirror `project` + `file/project_extension/repository.rs`; `resolve_scope_file_ids` is the only genuinely new logic (a plain owner-scoped `SELECT file_id`).
- **ITEM-6** — verdict: CONCERN — introduces new REST types ⇒ requires `just openapi-regen` (ITEM-12); resolved by making ITEM-12 an explicit dependency.
- **ITEM-7** — verdict: PASS — cap idiom mirrors `attach_file_capped`; 422-on-cap mirrors project upload (`PROJECT_FILE_COUNT_CAP`). KB cap 2000 (decision DEC-8).
- **ITEM-8** — verdict: PASS — tool dispatch mirrors `citations`/`files_mcp`; retrieval is a call into the verified `semantic_search`; read-only tools.
- **ITEM-9** — verdict: PASS — mirrors `citations/chat_extension` shared-flag pattern; order 24 verified free.
- **ITEM-10** — verdict: CONCERN — the two `mcp.rs` edits are a documented silent-failure point (register but model never sees tools); mitigated by a dedicated unit TEST on `auto_attach_builtin_ids`/`is_builtin_server_id`.
- **ITEM-11** — verdict: PASS — owner-scoped `publish` mirrors `project/handlers.rs`; new variants regenerate the TS union via ITEM-12.
- **ITEM-12** — verdict: CONCERN — mechanical but mandatory in BOTH workspaces; golden `emit_ts` parity test fails the build if skipped. Explicitly an item so it is not forgotten.
- **ITEM-13** — verdict: PASS — module registration mirrors `projects/module.tsx`; desktop auto-discovers (no blocklist entry needed, matching memory/file_rag).
- **ITEM-14** — verdict: PASS — list page/store mirror `Projects.store.ts`; `sync` self-gate mirrors the McpServer store rule.
- **ITEM-15** — verdict: PASS — upload/list/cap panel mirrors `ProjectFilesManagePanel`; per-doc status is new but derives from `file_chunks` counts (ITEM-5).
- **ITEM-16** — verdict: PASS — attachment picker mirrors existing conversation-scoped pickers (MCP/project chips); reads generated types.
- **ITEM-17** — verdict: PASS — project-extension mirrors `citations/project-extension/extension.tsx` (`knowledge_kinds` slot).
- **ITEM-18** — verdict: PASS — reuses `FilePreviewDrawer` + `requestPreviewPage`; hit provenance (page/char span) already present on `SemanticHit`.
- **ITEM-19** — verdict: CONCERN — new render states MUST have gallery cells or `check:state-matrix` (inside `npm run check`) fails phase 8; budgeted as its own item.
- **ITEM-20** — verdict: PASS — desktop parity is regen + `npm run check`; pgvector proven available on desktop (memory/file_rag run there).
- **ITEM-21** — verdict: PASS — docs-only; mirrors existing module sections in CLAUDE.md.

No `BLOCKED` verdicts. The three `CONCERN`s (ITEM-6/12 regen, ITEM-10 wiring,
ITEM-19 gallery) are all handled by explicit items + tests, not plan changes.

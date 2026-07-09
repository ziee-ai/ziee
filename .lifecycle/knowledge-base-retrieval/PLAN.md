# PLAN — knowledge-base-retrieval

## Context (grounding)

ziee already contains a **complete hybrid RAG engine** — the `file_rag` module:
chunk → `halfvec(768)` HNSW (cosine) + `content_tsv` GIN full-text → 4-arm
retrieval (Hybrid/Vector/FTS/None) fused with Reciprocal Rank Fusion, an embed
worker with live dimension migration, boot-time backfill, and admin settings
(`file_rag_admin_settings`, a deployment-wide embedding model). Its retrieval
entry point is:

```
file_rag::retrieval::semantic_search(scope_ids: &[Uuid], user_id, query, top_k, admin)
    -> SearchResult { hits: Vec<SemanticHit>, mode, truncated }
// SemanticHit { file_id, blob_version_id, version, page_number, char_start, char_end, content, score }
```

Today that engine is reachable only through `files_mcp`'s `semantic_search`
tool, whose scope is **the current conversation's available files**
(`resolve_available_files(conversation_id, user_id)` → `file_id = ANY(scope)`).
There is **no persistent, named collection** a user can build once (500 PDFs)
and retrieve from across conversations. The `project` path still *prepends* full
file text (100-file cap, fallback path) — which cannot scale to 500 docs.

**This feature is therefore a thin collection + scoping + agent-tool layer over
the existing engine, NOT a new RAG implementation.** A "knowledge base" is a
named, owner-scoped set of `file_id`s whose chunks already live in `file_chunks`
(produced by `file_rag`'s existing upload-time `spawn_index`). Retrieval over a
KB = resolve KB → its `file_id`s → call the *unchanged* `semantic_search`. The
agent reaches it through a new built-in MCP tool `search_knowledge`, auto-attached
when ≥1 KB is bound to the chat, returning cited hits the chat UI links back to
the file viewer at the source page. Embedding, chunking, hybrid retrieval, RRF,
the airgapped FTS-only fallback, and dimension migration are all inherited from
`file_rag` — one shared embedding space (the deployment-wide `file_rag`
embedding model), so all KBs are comparable.

## Items

- **ITEM-1**: Migration `00000000000133_create_knowledge_bases.sql` — `knowledge_bases` (id, user_id FK users ON DELETE CASCADE, name, description, document_count INT default 0, created_at, updated_at; per-user unique name), `knowledge_base_documents` (knowledge_base_id FK ON DELETE CASCADE, file_id FK files ON DELETE CASCADE, added_at; composite PK; index on file_id), `conversation_knowledge_bases` (conversation_id FK ON DELETE CASCADE, knowledge_base_id FK ON DELETE CASCADE; composite PK), `project_knowledge_bases` (project_id FK ON DELETE CASCADE, knowledge_base_id FK ON DELETE CASCADE; composite PK). No new chunk/embedding tables — chunks live in `file_chunks`.
- **ITEM-2**: Migration `00000000000134_grant_knowledge_base_permissions_to_users.sql` — idempotent grant of `knowledge_base::use` + `knowledge_base::manage` to the `Users` system-default group (mirrors migration 104/98).
- **ITEM-3**: `knowledge_base` module skeleton — `modules/knowledge_base/mod.rs` (`ModuleEntry` order 104, deterministic MCP id `knowledge_base_server_id()` = `Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`, `init()` upserts the built-in MCP row at loopback `/api/knowledge-base/mcp`, `register_routes`), `pub mod knowledge_base;` line in `modules/mod.rs`.
- **ITEM-4**: `knowledge_base/permissions.rs` — `KnowledgeBaseUse` (`knowledge_base::use`) + `KnowledgeBaseManage` (`knowledge_base::manage`) `PermissionCheck` impls, with tests pinning the strings to the migration.
- **ITEM-5**: `knowledge_base/models.rs` — `KnowledgeBase`, `KnowledgeBaseDocument` (+ derived per-doc `index_status` ∈ `pending|indexed|failed` and `chunk_count`), `CreateKnowledgeBaseRequest`, `UpdateKnowledgeBaseRequest`, `AttachDocumentsRequest`, `KnowledgeSearchHit` (projection of `SemanticHit` + file name), all `schemars::JsonSchema`.
- **ITEM-6**: `knowledge_base/repository.rs` — owner-scoped CRUD; `add_documents_capped` (atomic cap enforcement, cap constant `KB_MAX_DOCUMENTS = 2000`); `remove_documents`; `list_documents_with_status` (LEFT JOIN `file_chunks` for chunk-count + embedded-count → derive status); `resolve_scope_file_ids(kb_ids, user_id)` (union of member `file_id`s, owner-filtered — the bridge to `semantic_search`); attach/detach KB↔conversation and KB↔project; `attached_kb_ids_for_conversation` (conversation direct ∪ project read-through).
- **ITEM-7**: `knowledge_base/routes.rs` + `handlers.rs` — REST: `GET/POST /knowledge-bases`, `GET/PATCH/DELETE /knowledge-bases/{id}`, `GET /knowledge-bases/{id}/documents`, `POST /knowledge-bases/{id}/documents` (attach existing file_ids), `POST /knowledge-bases/{id}/documents/upload` (multipart bulk upload → `file::ingest::ingest_bytes` → attach; **422** on cap), `DELETE /knowledge-bases/{id}/documents/{file_id}`, `PUT/DELETE /conversations/{cid}/knowledge-bases/{kb_id}`, `PUT/DELETE /projects/{pid}/knowledge-bases/{kb_id}`. Every route gated by `RequirePermissions` + owner-scope (foreign id → 404); mutations emit sync + take `SyncOrigin`.
- **ITEM-8**: `knowledge_base/tools.rs` + MCP dispatch in `handlers.rs::jsonrpc_handler` — built-in MCP server exposing `list_knowledge_bases()` and `search_knowledge(query, knowledge_base_ids?, top_k?)`: resolves scope via `resolve_scope_file_ids`, loads `Repos.file_rag.get_admin_settings()`, calls `file_rag::retrieval::semantic_search`, returns hits as text + `structuredContent` (each hit: file_id, file name, page, char span, score) with the standard untrusted-content guard note. Tool auth: both read-only → `knowledge_base::use`.
- **ITEM-9**: `knowledge_base/chat_extension/{mod.rs, extension.rs, knowledge_base.rs}` — `ChatExtension` order 24; `ATTACH_FLAG = "attach_knowledge_base_mcp"`; `before_llm_call` gates on tool-capability, resolves `attached_kb_ids_for_conversation`, and when ≥1 sets the attach flag + injects a one-line system note listing the attached KB names (data-not-instructions). No `before_llm_call` context injection of chunks (tool-only, by design).
- **ITEM-10**: `mcp/chat_extension/mcp.rs` — the two required edits: `auto_attach_builtin_ids` pushes `knowledge_base_server_id()` when the attach flag is set; `is_builtin_server_id` adds it to the approval-bypass allowlist (read-only search).
- **ITEM-11**: `sync/event.rs` — add `SyncEntity::KnowledgeBase` (owner-scoped) and `SyncEntity::KnowledgeBaseDocument`; publish `Create/Update/Delete` from every KB/document/attachment mutation with `Audience::owner(user_id)`.
- **ITEM-12**: OpenAPI + TS regen for BOTH binaries (`just openapi-regen`) — new request/response types land in `src-app/ui/src/api-client/types.ts` and `src-app/desktop/ui/src/api-client/types.ts`; the `Knowledge*` endpoint namespace + `SyncEntity` union update.
- **ITEM-13**: `src-app/ui/src/modules/knowledge-base/module.tsx` — module registration, route `/knowledge`, sidebar nav widget entry, `settingsUserPages`/`Permissions` wiring; auto-discovered on desktop too (NOT blocklisted).
- **ITEM-14**: KB list page + store — `KnowledgeBasesListPage.tsx` + `KnowledgeBases.store.ts` (list/create/delete, `sync:knowledge_base` subscribe + `sync:reconnect` self-gate on `knowledge_base::use`); loaded/empty/error states.
- **ITEM-15**: KB detail page + store — `KnowledgeBaseDetailPage.tsx` + `KnowledgeBaseDetail.store.ts` (rename/describe, document list with per-doc index-status badge, bulk drag-drop upload with progress, attach-existing-files, remove-doc). Mirrors `ProjectFilesManagePanel`.
- **ITEM-16**: Conversation KB attachment control — a picker (store + UI) in the chat conversation surface to attach/detach KBs for the current conversation; reflects attached KBs.
- **ITEM-17**: `knowledge_kinds` project-extension "Knowledge bases" — `knowledge-base/project-extension/extension.tsx` (inline preview + manage panel) letting a project bind KBs (mirrors the citations "References" project-extension).
- **ITEM-18**: Chat citation rendering — render `search_knowledge` tool-result hits (from `structuredContent`) as citation chips that open `FilePreviewDrawer` at `page_number` via `Stores.File.requestPreviewPage`; reuses the existing PDF page-image viewer.
- **ITEM-19**: Gallery coverage — `gallery-page-*` entries + loading/empty/error state cells for the KB list page, KB detail page, and the project-extension panel, satisfying `check:state-matrix` (both `ui` and `desktop/ui`).
- **ITEM-20**: Desktop UI parity — confirm the module loads on the embedded desktop server (pgvector present, like memory/file_rag; no blocklist entry), desktop `api-client` regenerated, `npm run check` green in `src-app/desktop/ui`.
- **ITEM-21**: `CLAUDE.md` "Knowledge Base Retrieval" section — document the module, the reuse-of-`file_rag` contract, the `search_knowledge` tool, scoping, and the debug/test seams (mirrors the web_search / lit_search / citations sections).

## Files to touch

Backend (`src-app/server/`):
- `migrations/00000000000133_create_knowledge_bases.sql` (new)
- `migrations/00000000000134_grant_knowledge_base_permissions_to_users.sql` (new)
- `src/modules/mod.rs` (add `pub mod knowledge_base;`)
- `src/modules/knowledge_base/mod.rs` (new)
- `src/modules/knowledge_base/permissions.rs` (new)
- `src/modules/knowledge_base/models.rs` (new)
- `src/modules/knowledge_base/repository.rs` (new)
- `src/modules/knowledge_base/routes.rs` (new)
- `src/modules/knowledge_base/handlers.rs` (new)
- `src/modules/knowledge_base/tools.rs` (new)
- `src/modules/knowledge_base/chat_extension/mod.rs` (new)
- `src/modules/knowledge_base/chat_extension/extension.rs` (new)
- `src/modules/knowledge_base/chat_extension/knowledge_base.rs` (new)
- `src/core/repository.rs` (register `knowledge_base: KnowledgeBaseRepository`)
- `src/modules/mcp/chat_extension/mcp.rs` (2 edits)
- `src/modules/sync/event.rs` (SyncEntity variants)
- `src-app/server/tests/knowledge_base/*.rs` (new integration tests) + `tests/integration_tests.rs` (module include)

OpenAPI/generated (mechanical, via `just openapi-regen`):
- `src-app/server/openapi/openapi.json`, `src-app/ui/openapi/openapi.json`, `src-app/desktop/ui/openapi/openapi.json`
- `src-app/ui/src/api-client/types.ts`, `src-app/desktop/ui/src/api-client/types.ts`

Frontend (`src-app/ui/src/modules/knowledge-base/` — mirrored into `src-app/desktop/ui/`):
- `module.tsx`, `types.ts`
- `stores/KnowledgeBases.store.ts`, `stores/KnowledgeBaseDetail.store.ts`, `stores/ConversationKnowledgeBases.store.ts`
- `pages/KnowledgeBasesListPage.tsx`, `pages/KnowledgeBaseDetailPage.tsx`
- `components/KnowledgeBaseDocumentsPanel.tsx`, `components/KnowledgeBaseFormDrawer.tsx`, `components/ConversationKnowledgeBasePicker.tsx`, `components/KnowledgeCitationChip.tsx`
- `project-extension/extension.tsx`, `project-extension/components/*`
- gallery: `src-app/ui/src/dev/gallery/` entries for the new surfaces
- E2E: `src-app/ui/tests/e2e/14-knowledge-base/*.spec.ts` (new)

Docs:
- `CLAUDE.md` (new module section)

## Patterns to follow

- **Built-in module + built-in MCP server** → mirror `modules/web_search/` and `modules/citations/` (deterministic `Uuid::new_v5` id, `init()` loopback upsert via `code_sandbox::loopback_host`, plain `.route("/…/mcp", post(jsonrpc_handler))` kept out of OpenAPI, REST via `.api_route(... get_with/post_with)`).
- **Collection + M:N membership + attach-to-conversation/project** → mirror `modules/project/` + the `file/project_extension/` join (`project_files`, `attach_file_capped`, the `PROJECT_MAX_FILES` cap idiom, `project_bibliography`).
- **Retrieval reuse** → call `file_rag::retrieval::semantic_search` exactly as `files_mcp/handlers.rs::semantic_search` does (build `scope_ids`, load `Repos.file_rag.get_admin_settings()`); do NOT re-implement chunk/embed/retrieval.
- **Embedding client** → reuse `memory::engine::dispatch::{embed, embed_batch}` transitively via `file_rag` (no direct calls needed).
- **Chat extension + auto-attach + approval-bypass** → mirror `modules/citations/chat_extension/` (shared `ATTACH_FLAG` const, order <30) and the two `mcp/chat_extension/mcp.rs` edits.
- **Permissions + grant-to-Users migration** → mirror `web_search/permissions.rs` + migration `00000000000104_grant_citations_permissions_to_users.sql`.
- **Sync** → mirror `project/handlers.rs` owner-scoped `sync_publish(SyncEntity::…, action, id, Audience::owner(uid), origin.0)` + `SyncOrigin` extractor.
- **Frontend module / list+detail pages / file upload panel** → mirror `ui/src/modules/projects/` (stores, pages) + `ui/src/modules/file/project-extension/components/ProjectFilesManagePanel.tsx` (upload/list/cap) + the `knowledge_kinds` project-extension registry (`projects/core/extensions/`).
- **Citation → viewer** → reuse `ui/src/modules/file/components/FilePreviewDrawer.tsx` + `file/viewers/pdf/body.tsx` `Stores.File.requestPreviewPage(file, page)`.
- **Store authoring** → `defineStore`/`defineExtensionStore` (store-kit), `sync:<entity>` subscription with `hasPermissionNow` self-gate.

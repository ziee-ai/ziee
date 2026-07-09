# PLAN — knowledge-base-retrieval

## Context (grounding)

ziee already contains a **complete hybrid RAG engine** — the `file_rag` module:
page-aware chunking → `halfvec` HNSW (cosine) + `content_tsv` GIN full-text →
4-arm retrieval (Hybrid/Vector/FTS/None) fused with Reciprocal Rank Fusion, an
embed worker with live dimension migration, boot-time backfill, and admin
settings. Its retrieval entry point is
`file_rag::retrieval::semantic_search(scope_ids: &[Uuid], user_id, query, top_k, admin)
-> SearchResult { hits: Vec<SemanticHit{file_id,blob_version_id,version,page_number,char_start,char_end,content,score}>, mode, truncated }`,
reachable today only through `files_mcp`'s `semantic_search` tool, scoped to a
conversation's available files. Embeddings run through the shared
`memory::engine::dispatch::{embed,embed_batch}` (resolve `llm_models` id →
provider → `ai_providers::Provider::embeddings`).

**This feature delivers a first-class KNOWLEDGE BASE the agent retrieves from at
scale, built on that engine.** Confirmed decisions (DECISIONS.md): retrieval is
an on-demand **`search_knowledge` MCP tool** (no context injection); KBs are
**user-owned** (attachable to the owner's chats/projects); citations render an
**exact-passage highlight overlay** in the file viewer; and v1 adds a
**self-hosted cross-encoder reranker** (retrieve-wide → rerank → top-k) as a new
model capability. The plan has four parts: **R** reranker (a shared `file_rag`/
runtime capability — also upgrades the existing `semantic_search`), **K** the KB
module (collection + tool + scoping), **C** the citation highlight overlay, and
**X** cross-cutting (OpenAPI/desktop/docs/gallery).

`grep -rni rerank src-app/` returns **zero hits** — the entire reranker surface
is net-new, mirrored on the embedding capability. Worktree base = origin/main @
9a6fb88c6 (highest migration `00000000000132_add_openrouter_provider_type.sql`);
next free numbers are **133/134/135**.

## Items

### Part R — Self-hosted cross-encoder reranker (shared capability)

- **ITEM-1**: `ai-providers` — add `RerankRequest { model, query, documents: Vec<String>, top_n: Option<usize> }` + `RerankResponse { results: Vec<RerankResult{index, score}> }` to `models/chat.rs`; add `rerank()` to the `AIProvider` trait with a **default "unsupported" impl** (mirrors `upload_file`); add the `Provider::rerank` wrapper; implement `rerank` **only** in `OpenAIProvider` (POST `{base_url}/rerank`, parse `results[]`). gemini/anthropic inherit the unsupported default.
- **ITEM-2**: Model capability — add `rerank: Option<bool>` to `ModelCapabilities` (`llm_model/models.rs`; JSONB, **no migration**); add `"rerank"` to the `ALLOWED_CAPABILITIES` filter (`llm_model/handlers/models.rs` + `types.rs` doc mirror); add `rerank_unsupported_reason` to `memory/engine/capability.rs` (mirrors `embedding_unsupported_reason`).
- **ITEM-3**: Shared dispatcher — add `pub async fn rerank(model_id, query, docs) -> Result<Vec<(usize, f32)>>` to `memory/engine/dispatch.rs`, mirroring `embed` (resolve model → check `capabilities.rerank` → resolve provider → `Provider::rerank`); export via the module. Callable by `file_rag` and any future consumer.
- **ITEM-4**: Local-runtime serving — add a `reranking: bool` thread through `llm_local_runtime/deployment/local.rs::llamacpp_argv` (push `--reranking` + `--pooling rank`) read from `config["reranking"]`; inject `config["reranking"]=true` from `capabilities.rerank` in `auto_start.rs::resolve_model_inputs` (parallel to `embeddings`); add a `proxy_rerank` handler + `proxy_rerank_docs` + a route in the **explicit** proxy allowlist (`proxy_router.rs`/`proxy_handlers.rs`) forwarding to the engine's `/v1/rerank`.
- **ITEM-5**: Migration `00000000000135_add_file_rag_reranker.sql` — `ALTER TABLE file_rag_admin_settings ADD COLUMN reranker_model_id UUID REFERENCES llm_models(id) ON DELETE SET NULL`, `rerank_enabled BOOLEAN NOT NULL DEFAULT FALSE`, `rerank_candidate_k INTEGER NOT NULL DEFAULT 30 CHECK (rerank_candidate_k BETWEEN 1 AND 200)` (mirrors migration 99's `embedding_model_id` idiom).
- **ITEM-6**: `file_rag` settings plumbing — add the three fields to `FileRagAdminSettings` + `UpdateFileRagAdminSettingsRequest` (`file_rag/models.rs`, nullable FK via `Option<Option<Uuid>>` + `deserialize_nullable_field`); thread them through `repository.rs::update_admin_settings` (positional `query_as!` SET + RETURNING); validate ranges + **probe-rerank on set** in `handlers.rs` (mirrors the embedding dimension-probe); publish the existing `SyncEntity::FileRagAdminSettings`.
- **ITEM-7**: `file_rag` retrieval rerank stage — in `retrieval.rs::semantic_search`, when `rerank_enabled && reranker_model_id.is_some()`: retrieve a candidate pool of `rerank_candidate_k` (≥ top_k) from the existing arms, call `dispatch::rerank(id, query, contents)`, reorder `hits` by returned score, then `truncate(top_k)`. Graceful fallback: on rerank error, keep the pre-rerank order (log, no fail). Reflect the applied reranker in `SearchResult`/`RetrievalMode`.
- **ITEM-8**: Reranker frontend — add a "Reranker" `CapabilityRow` (mutually exclusive with chat, like `text_embedding`) to `LlmModelCapabilitiesSection.tsx`; add a reranker-model dropdown (`?capability=rerank`) + enable toggle + candidate-k input to the `file-rag` admin UI (`EmbeddingSection.tsx`/a new `RerankSection.tsx` + `FileRagAdmin.store.ts`).

### Part K — Knowledge base module (collection + tool + scoping)

- **ITEM-9**: Migration `00000000000133_create_knowledge_bases.sql` — `knowledge_bases` (id, user_id FK ON DELETE CASCADE, name, description, document_count INT default 0, created_at, updated_at; per-user unique name), `knowledge_base_documents` (kb_id FK, file_id FK, added_at; composite PK; file_id index), `conversation_knowledge_bases`, `project_knowledge_bases` (composite-PK join tables, all FKs ON DELETE CASCADE). No new chunk/embedding tables.
- **ITEM-10**: Migration `00000000000134_grant_knowledge_base_permissions_to_users.sql` — idempotent grant of `knowledge_base::use` + `knowledge_base::manage` to the `Users` group (mirrors migration 104).
- **ITEM-11**: `knowledge_base` module skeleton — `mod.rs` (`ModuleEntry` order 104, `knowledge_base_server_id()`=`Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`, `init()` upserts the built-in MCP row at loopback `/api/knowledge-base/mcp`, `register_routes`), `pub mod knowledge_base;` in `modules/mod.rs`, repository registration in `core/repository.rs`.
- **ITEM-12**: `knowledge_base/permissions.rs` — `KnowledgeBaseUse` (`knowledge_base::use`) + `KnowledgeBaseManage` (`knowledge_base::manage`) with tests pinning the strings to migration 134.
- **ITEM-13**: `knowledge_base/models.rs` — `KnowledgeBase`, `KnowledgeBaseDocument` (+ derived `index_status ∈ pending|indexed|failed`, `chunk_count`), `Create/Update/AttachDocumentsRequest`, `KnowledgeSearchHit` (SemanticHit projection + file name), all `schemars::JsonSchema`.
- **ITEM-14**: `knowledge_base/repository.rs` — owner-scoped CRUD; `add_documents_capped` (atomic, `KB_MAX_DOCUMENTS = 2000`, tx-consistent `document_count`); `remove_documents`; `list_documents_with_status` (LEFT JOIN `file_chunks` for chunk/embedded counts → status); `resolve_scope_file_ids(kb_ids, user_id)` (owner-scoped union — the bridge to `semantic_search`); attach/detach KB↔conversation and KB↔project; `attached_kb_ids_for_conversation` (conversation-direct ∪ project read-through).
- **ITEM-15**: `knowledge_base/routes.rs` + `handlers.rs` — REST CRUD + documents (attach existing / multipart bulk upload → `file::ingest::ingest_bytes` → attach, **422** on cap / detach / list-with-status) + attach/detach to conversation & project; every route `RequirePermissions`-gated + owner-scoped (foreign id → 404); mutations emit sync + take `SyncOrigin`.
- **ITEM-16**: `knowledge_base/tools.rs` + JSON-RPC dispatch in `handlers.rs` — built-in MCP `list_knowledge_bases()` + `search_knowledge(query, knowledge_base_ids?, top_k?)`: resolve scope via `resolve_scope_file_ids`, load `Repos.file_rag.get_admin_settings()`, call `file_rag::retrieval::semantic_search` (now reranked per Part R), return hits as text + `structuredContent` (file_id, file name, page, char span, score). Tool description carries the **grounded-answer instruction** (answer only from results; say "not found"; cite the hit). Both tools read-only → `knowledge_base::use`.
- **ITEM-17**: `knowledge_base/chat_extension/{mod,extension,knowledge_base}.rs` — `ChatExtension` order 24; `ATTACH_FLAG="attach_knowledge_base_mcp"`; `before_llm_call` gates on tool-capability, resolves `attached_kb_ids_for_conversation`, and when ≥1 sets the flag + injects a one-line note listing attached KB names + the grounding nudge (no chunk injection). Plus the two `mcp/chat_extension/mcp.rs` edits (`auto_attach_builtin_ids` push + `is_builtin_server_id` approval-bypass) and `SyncEntity::KnowledgeBase`/`KnowledgeBaseDocument` in `sync/event.rs`.

### Part K (frontend)

- **ITEM-18**: `ui/src/modules/knowledge-base/module.tsx` — module registration, route `/knowledge`, sidebar nav, `Permissions` wiring; auto-discovered on desktop (NOT blocklisted).
- **ITEM-19**: KB list page + store — `KnowledgeBasesListPage.tsx` + `KnowledgeBases.store.ts` (list/create/delete, `sync:knowledge_base` subscribe + `sync:reconnect` self-gate on `knowledge_base::use`); loaded/empty/error states.
- **ITEM-20**: KB detail page + store — `KnowledgeBaseDetailPage.tsx` + `KnowledgeBaseDetail.store.ts` (rename/describe, document list with per-doc index-status badge, bulk drag-drop upload with progress, attach-existing, remove). Mirrors `ProjectFilesManagePanel`.
- **ITEM-21**: Conversation KB attachment picker — `ConversationKnowledgeBases.store.ts` + a picker in the chat surface to attach/detach KBs for the current conversation.
- **ITEM-22**: `knowledge_kinds` project-extension "Knowledge bases" — `knowledge-base/project-extension/extension.tsx` (inline preview + manage panel) to bind KBs to a project (mirrors citations "References").
- **ITEM-23**: Chat citation + transparency rendering — render `search_knowledge` `structuredContent` hits as **numbered citation chips** (hover preview of cited text) and a **retrieval-transparency panel** ("chunks used": file, page, score, snippet). Clicking a chip opens the viewer at the source page (Part C).

### Part C — Exact-passage citation highlight overlay

- **ITEM-24**: Alignment primitive + geometry source (**load-bearing risk**) — a `file/utils/pdfium.rs` routine that relocates a chunk's stored (cleaned-text) content on the raw PDFium page via `page.text().search(content)` (whitespace-insensitive), yielding a char-index range → per-char `tight_bounds()` boxes, normalized to **fractions** of page width/height (rotation-aware for landscape). PDF-only; returns empty on no-match. (Ingest-time geometry capture covering office docs is recorded in "Out of scope" as the v1.5 upgrade.)
- **ITEM-25**: Geometry endpoint — `GET /api/files/{id}/pages/{n}/text-rects?start=&end=` in `file/handlers/management.rs` + `routes.rs`, gated `RequirePermissions<(FilesRead,)>`, loads the original PDF blob, runs ITEM-24 in `spawn_blocking`, returns `{ page_w, page_h, rects: [{x,y,w,h}] }` (fractions). `*_docs` transform + OpenAPI.
- **ITEM-26**: Viewer overlay layer — in `file/viewers/pdf/body.tsx` wrap each page `<img>` in a `position:relative` container and render a `%`-positioned highlight box; extend `FilePreviewDrawer.store.ts` state with `{ targetPage?, charStart?, charEnd? }`, add scroll-into-view of `[data-page-index]`, fetch text-rects for the target span, and honor the landscape transform.
- **ITEM-27**: Citation deep-link wiring — the citation chip (ITEM-23) calls `FilePreviewDrawer.openPreview(file, { page, charStart, charEnd })`; **graceful degradation**: when text-rects are empty (no match / non-PDF) the chip still opens the page (page-level deep-link), just without the box.

### Part X — Cross-cutting

- **ITEM-28**: OpenAPI + TS regen for BOTH binaries (`just openapi-regen`) — new `Knowledge*` + `Rerank*`/capability + `text-rects` + `SyncEntity` types land in `ui/` and `desktop/ui/` `api-client/types.ts`; golden `emit_ts` parity test enforces it.
- **ITEM-29**: Desktop parity — module loads on the embedded desktop server (pgvector present; not blocklisted); reranker capability + file-rag reranker UI present; `npm run check` green in `src-app/desktop/ui`.
- **ITEM-30**: Gallery coverage — `gallery-page-*` + loading/empty/error cells for KB list/detail, the project-extension panel, the reranker admin section, and a citation-overlay state, satisfying `check:state-matrix` in both `ui` and `desktop/ui`.
- **ITEM-31**: `CLAUDE.md` — new "Knowledge Base Retrieval" section (module, reuse-of-`file_rag`, `search_knowledge`, scoping, highlight overlay) **and** a "Reranker capability" note in the local-runtime/file_rag sections (the new `rerank` capability, `--reranking` serving, retrieve→rerank→top-k), plus the debug/test seams.

## Out of scope for v1 (recorded roadmap, from the research passes)

- **Structure-aware scientific-PDF ingest (SEPARATE INITIATIVE)** — GROBID (Apache-2.0 sidecar; IMRaD + parsed references) / Docling (MIT; office/slides), PDFium fallback, via the `bio_mcp`-style managed-sidecar pattern; benefits ALL consumers, warrants its own lifecycle. **Licensing (verified): avoid Marker (GPL-3 + weights capped at $2M revenue) and Nougat (weights CC-BY-NC); use GROBID/Docling/Unstructured (permissive).**
- **Element-typed / table-atomic chunking**, **metadata-filtered retrieval** (reuse `lit_search`/`citations` connectors + OpenAlex CC0 offline snapshot) — depend on the parser above.
- **Ingest-time char-geometry capture** (capture char boxes in `PdfProcessor::extract_text` before `clean_extracted_text`, sidecar table + backfill) — the precise, **office-covering** upgrade to the best-effort on-demand overlay (ITEM-24).
- **MedCPT domain reranker A/B** (public-domain, biomedical) once the general BGE reranker path is proven; **MeSH synonym query expansion** (public); **SPECTER2 doc-level "related papers"** space (never the chunk store).
- **Elicit-style extraction tables**, **shared/org KBs** (cross-user `file_chunks` read RBAC), **three-pane sources|chat|notes**.

## Files to touch

Backend — reranker (Part R):
- `src-app/server/ai-providers/src/models/chat.rs`, `src/traits.rs`, `src/provider.rs`, `src/providers/openai.rs`
- `src-app/server/src/modules/llm_model/models.rs`, `handlers/models.rs`, `types.rs`
- `src-app/server/src/modules/memory/engine/{capability.rs,dispatch.rs}`
- `src-app/server/src/modules/llm_local_runtime/deployment/local.rs`, `auto_start.rs`, `proxy_router.rs`, `proxy_handlers.rs`
- `src-app/server/migrations/00000000000135_add_file_rag_reranker.sql` (new)
- `src-app/server/src/modules/file_rag/{models.rs,repository.rs,handlers.rs,retrieval.rs}`

Backend — KB (Part K):
- `src-app/server/migrations/00000000000133_create_knowledge_bases.sql`, `00000000000134_grant_knowledge_base_permissions_to_users.sql` (new)
- `src-app/server/src/modules/mod.rs`, `src/core/repository.rs`
- `src-app/server/src/modules/knowledge_base/{mod,permissions,models,repository,routes,handlers,tools}.rs` + `chat_extension/{mod,extension,knowledge_base}.rs` (new)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (2 edits), `src/modules/sync/event.rs`
- `src-app/server/tests/knowledge_base/*.rs` + `tests/integration_tests.rs` (module include)

Backend — highlight (Part C):
- `src-app/server/src/modules/file/utils/pdfium.rs`, `handlers/management.rs`, `routes.rs`, `types.rs`

OpenAPI/generated (mechanical via `just openapi-regen`):
- `src-app/server/openapi/openapi.json`, `src-app/ui/openapi/openapi.json`, `src-app/desktop/ui/openapi/openapi.json`
- `src-app/ui/src/api-client/types.ts`, `src-app/desktop/ui/src/api-client/types.ts`

Frontend (`src-app/ui/src/modules/knowledge-base/` + shared edits — mirrored into `src-app/desktop/ui/`):
- `module.tsx`, `types.ts`, `stores/{KnowledgeBases,KnowledgeBaseDetail,ConversationKnowledgeBases}.store.ts`
- `pages/{KnowledgeBasesListPage,KnowledgeBaseDetailPage}.tsx`
- `components/{KnowledgeBaseDocumentsPanel,KnowledgeBaseFormDrawer,ConversationKnowledgeBasePicker,KnowledgeCitationChip,KnowledgeRetrievalTransparency}.tsx`
- `project-extension/extension.tsx` + `project-extension/components/*`
- reranker UI: `src-app/ui/src/modules/llm-provider/components/llm-models/shared/LlmModelCapabilitiesSection.tsx`, `src-app/ui/src/modules/file-rag/components/sections/RerankSection.tsx`, `file-rag/stores/FileRagAdmin.store.ts`
- highlight overlay: `src-app/ui/src/modules/file/viewers/pdf/body.tsx`, `file/stores/FilePreviewDrawer.store.ts`
- gallery entries: `src-app/ui/src/dev/gallery/`
- E2E: `src-app/ui/tests/e2e/14-knowledge-base/*.spec.ts` (new)

Docs: `CLAUDE.md`.

## Patterns to follow

- **Reranker capability end-to-end** → mirror the **embedding** capability exactly: provider = `openai.rs` `embeddings`; DTOs = `models/chat.rs` `Embeddings*`; trait default = `upload_file`; dispatcher = `dispatch.rs::embed`; local flag = the `embeddings: bool` thread through `local.rs`/`auto_start.rs`; proxy = `proxy_embeddings`; settings = `embedding_model_id` from migration 99 → `models.rs` → `repository.rs` → `handlers.rs` → admin UI.
- **Built-in module + built-in MCP server** → mirror `modules/web_search/` and `modules/citations/`.
- **Collection + M:N + attach joins + cap** → mirror `project_files` / `attach_file_capped` / `project_bibliography`.
- **Retrieval reuse** → call `file_rag::retrieval::semantic_search` as `files_mcp/handlers.rs` does; do NOT re-implement chunk/embed/retrieval — extend it in place (rerank stage).
- **Chat extension + auto-attach + approval-bypass** → mirror `modules/citations/chat_extension/` + the two `mcp/chat_extension/mcp.rs` edits.
- **Permissions + grant-to-Users migration** → `web_search/permissions.rs` + migration 104.
- **Sync** → `project/handlers.rs` owner-scoped `sync_publish(...Audience::owner(uid), origin.0)` + `SyncOrigin`.
- **Frontend module / list+detail / upload / project-extension** → `ui/src/modules/projects/` + `file/project-extension/components/ProjectFilesManagePanel.tsx` + the `knowledge_kinds` registry.
- **Highlight overlay** → extend `file/viewers/pdf/body.tsx` (page-image viewer) + `FilePreviewDrawer` state; geometry from pdfium-render `page.text().search()`/`tight_bounds()`; endpoint mirrors `file/handlers/management.rs::get_preview` (gated `FilesRead`).
- **Store authoring** → `defineStore`/`defineExtensionStore` (store-kit), `sync:<entity>` subscription with `hasPermissionNow` self-gate.

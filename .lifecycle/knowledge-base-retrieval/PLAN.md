# PLAN — knowledge-base-retrieval

## Context (grounding)

ziee has a complete hybrid RAG engine (`file_rag`: page-aware chunking → `halfvec`
HNSW + `content_tsv` GIN → 4-arm retrieval + RRF, embed worker, backfill),
reachable today only via `files_mcp`'s `semantic_search` (conversation-file
scope). This feature delivers a **first-class, user-owned KNOWLEDGE BASE** the
agent retrieves from at scale, with a carefully-designed UI/UX. User-confirmed
decisions: on-demand `search_knowledge` tool (no injection); user-owned KBs
(standalone-reusable, attach to chats/projects); **exact-passage highlight via
ingest-time geometry**; a **self-hosted cross-encoder reranker delivered through
the `ziee-ai/hub`**.

This plan was hardened by a three-angle adversarial audit that found (and this
revision fixes): the highlight can't work on-demand against cleaned-text offsets
(→ ingest-time geometry, DEC-31); the per-doc index-status UX had no backend state
or emit source (→ Part I); `document_count` drifts on external file-delete (→
derive at read, DEC-32); chat-extension order 24 collides with `summarization` (→
**order 23**); cross-user `search_knowledge` leak, attach-existing files never
indexed, duplicate re-drops, half-indexed silent answers, scanned/zero-text docs,
per-file size cap + batch-reject UX — all now have items/decisions/tests.

**Parts:** R reranker capability · **H hub reranker model** · **I file_rag
indexing observability (shared)** · K KB module · **C ingest-time citation
geometry + highlight** · K-UI / C-UI / R-UI frontend · X cross-cutting.
`grep rerank src-app/` = 0. Worktree base @ 4a3769691 (highest migration 132);
next free 133–137.

## Items

### Part R — Reranker capability (backend)

- **ITEM-1**: `ai-providers` — `Rerank{Request,Response}` DTOs; `rerank()` trait method (default "unsupported"); `Provider::rerank`; `OpenAIProvider` impl POSTing to **`/v1/rerank`** (reconcile the path once vs DEC-30; llama.cpp serves both — pick `/v1/rerank` end-to-end).
- **ITEM-2**: Capability — `ModelCapabilities.rerank: Option<bool>`; `"rerank"` in `ALLOWED_CAPABILITIES`; `rerank_unsupported_reason` guard; the **hub→llm_model capability conversion** (`hub/handlers.rs:1612`) maps `rerank`.
- **ITEM-3**: `dispatch::rerank(model_id, query, docs) -> Vec<(usize,f32)>` (`memory/engine/dispatch.rs`), mirroring `embed`, and **auto-starts** the local reranker model on first call (as embedding does via `auto_start`).
- **ITEM-4**: Local serving — `--reranking`(+`--pooling rank`) in `llamacpp_argv`; capability→`config["reranking"]` in `auto_start.rs`; `proxy_rerank` handler + `/rerank`→engine `/v1/rerank` route in the explicit proxy allowlist.
- **ITEM-5**: Migration `00000000000135_add_file_rag_reranker.sql` — `ALTER file_rag_admin_settings ADD reranker_model_id UUID FK ON DELETE SET NULL / rerank_enabled BOOL DEFAULT FALSE / rerank_candidate_k INT DEFAULT 30 CHECK 1..200`.
- **ITEM-6**: `file_rag` settings plumbing — the three fields through `models.rs`/`repository.rs`(SET+RETURNING)/`handlers.rs`(range-validate + probe-rerank on set) + capability **mutual-exclusion** intent; publish `SyncEntity::FileRagAdminSettings`.
- **ITEM-7**: `file_rag` retrieval rerank stage — in `semantic_search`, gated `rerank_enabled && reranker_model_id`: expand to `rerank_candidate_k` (candidate pool **wider than top_k** so a low-ranked doc can be promoted) → `dispatch::rerank` → reorder → **recompute `truncated`** → `truncate(top_k)`; on error keep pre-rerank order; preserve empty-scope/embed-failure guards. Also benefits the existing `files_mcp` path.

### Part H — Hub reranker model (ziee-ai/hub + vendored seed)

- **ITEM-8**: Hub **schema** — add `rerank: boolean` to `capabilities` in BOTH `schemas/2026-06-12/model.schema.json` and `schemas/v1/model.schema.json` (currently `additionalProperties:false`, no `rerank`), authored in the cloned `ziee-ai/hub`.
- **ITEM-9**: Hub **manifest** — `models/bge-reranker-v2-m3-gguf.yaml` (`capabilities.rerank: true`, `runtimeHint: llamacpp`, `fileFormat: gguf`, quantizations, `_hub_curation` tags incl. `reranker`/`rag`/`rag-recommended`, Apache-2.0). (Embedding `nomic-embed-text-v1-5-gguf` already exists — no change.)
- **ITEM-10**: Hub **index regen + seed mirror** — run the hub build pipeline (`scripts/validate.py` + `build-pages.py`) to emit the model JSON; mirror the updated index + manifest into the vendored seed `src-app/server/binaries/hub-seed/` so dev/build picks it up; bump `SEED_HUB_VERSION`. (Two coordinated PRs: `ziee-ai/hub` + this repo — per the clone-first, mirror-to-seed rule.)

### Part I — file_rag indexing observability (shared; the index-status backend)

- **ITEM-11**: Migration `00000000000136_create_file_index_state.sql` — `file_index_state (file_id PK FK ON DELETE CASCADE, user_id, status TEXT CHECK IN ('pending','indexing','indexed','failed','no_text'), error TEXT, chunk_count INT, updated_at)`. Owner of per-file lifecycle status (chunk counts alone can't distinguish pending/indexing/failed/no-text).
- **ITEM-12**: Write status in `file_rag/ingest.rs` — set `pending`→`indexing`→`indexed`/`failed`/`no_text` around `index_file_version` (the current no-text early-return becomes an explicit `no_text` write; the `warn!`-only failure becomes a `failed` write) and **emit `SyncEntity::FileIndexState`** (owner-scoped) on each transition. This is the source that drives the live KB doc-status stream.
- **ITEM-13**: A re-index trigger — `POST /api/file-rag/files/{id}/reindex` (or reuse `spawn_reindex`) so a `failed`/`no_text`/zero-chunk file can be retried; used by the KB "Retry" affordance and the attach-existing path.

### Part K — Knowledge base module (backend)

- **ITEM-14**: Migration `00000000000133_create_knowledge_bases.sql` — `knowledge_bases` (id, user_id FK CASCADE, name, description, timestamps; per-user unique name — **NO denormalized `document_count`**, derived at read per DEC-32), `knowledge_base_documents` (kb_id, file_id, added_at; composite PK; file_id index), `conversation_knowledge_bases`, `project_knowledge_bases` (composite-PK joins, all CASCADE).
- **ITEM-15**: Migration `00000000000134_grant_knowledge_base_permissions_to_users.sql` — idempotent grant `knowledge_base::use` + `knowledge_base::manage` to Users.
- **ITEM-16**: Module skeleton — `mod.rs` (`ModuleEntry` order 104; `knowledge_base_server_id()`; loopback upsert `/api/knowledge-base/mcp`), `pub mod`, `core/repository.rs`, `permissions.rs`.
- **ITEM-17**: `models.rs` — `KnowledgeBase` (+ derived `document_count`, `indexing_summary{indexed,indexing,failed,no_text,total}`), `KnowledgeBaseDocument` (+ `index_status` from `file_index_state`, `chunk_count`), `Create/Update/AttachDocumentsRequest`, `KnowledgeSearchHit`, all `JsonSchema`.
- **ITEM-18**: `repository.rs` — owner-scoped CRUD (**count via `COUNT(*)` subquery**, no drift); `add_documents_capped` (atomic, `KB_MAX_DOCUMENTS=2000`, **dedup by files.checksum** → skip-and-report existing); attach-existing **triggers reindex when the file has 0 chunks** (ITEM-13); `remove_documents` (deletes ONLY the join row — never `file_chunks`); `list_documents_with_status` (**server-paginated**, joins `file_index_state`); `resolve_scope_file_ids(kb_ids, user_id)` (**owner-filtered** — the cross-user guard); attach/detach conv+project; `attached_kb_ids_for_conversation` (direct ∪ project read-through); `kb_attachment_targets`; `indexing_summary`.
- **ITEM-19**: `routes.rs` + `handlers.rs` — CRUD + documents (attach existing / **bulk multipart upload with server-side per-file size + type validation + checksum dedup + itemized reject report** → `ingest_bytes` → attach, 422 on cap / detach / **paginated** list-with-status) + conv/project attach + `GET /knowledge-bases/{id}` (with `indexing_summary`); every handler resolves via `get_by_id_and_user` (foreign→404) + `RequirePermissions`; mutations emit sync + `SyncOrigin`.
- **ITEM-20**: `tools.rs` + JSON-RPC — `list_knowledge_bases()` + `search_knowledge(query, knowledge_base_ids?, top_k?)`: **owner-filtered** scope resolve → `file_rag::retrieval::semantic_search` (reranked) → hits as text + `structuredContent` (**capped at the shared 1 MB** limit) with file/name/page/char-span/score; result carries an **`indexing_incomplete{searchable,total}`** signal when the KB isn't fully indexed; tool description carries the grounded-answer instruction. Read-only → `knowledge_base::use`.
- **ITEM-21**: `chat_extension/{mod,extension,knowledge_base}.rs` (backend, **order 23** — 24 collides with summarization) — `ATTACH_FLAG`; gate tool-capability; resolve owner-scoped attached KBs; set flag + KB-names note + grounding nudge conditioned on indexing-complete. Plus the two `mcp/chat_extension/mcp.rs` edits + `SyncEntity::KnowledgeBase`/`KnowledgeBaseDocument` in `sync/event.rs`.

### Part C — Ingest-time citation geometry + highlight

- **ITEM-22**: **Cleaned-char→box map at extraction** — change `file/processing/pdf.rs`: while extracting per-page text, capture each PDFium char's `tight_bounds`, and modify `clean_extracted_text` to emit, alongside the cleaned string, a **parallel per-cleaned-char geometry array** (rotation-normalized fractions) so a cleaned `[start,end)` span maps directly to boxes. Office docs: geometry captured from the temp PDF before it's deleted. (This resolves the audit's fatal on-demand-search flaw.)
- **ITEM-23**: Migration `00000000000137_create_file_page_geometry.sql` + storage — persist the per-page geometry (a storage derivative keyed by `(user_id, blob_version_id, page)`, mirroring text-page storage) and/or per-chunk box lists; `file_rag` ingest writes chunk geometry from the page map.
- **ITEM-24**: **Backfill** — a boot-time/one-shot pass to (re)capture geometry for already-ingested files (mirrors `file_rag::run_backfill`); files without geometry degrade to page-level until backfilled.
- **ITEM-25**: **Geometry endpoint** — `GET /api/files/{id}/pages/{n}/text-rects?start=&end=` (`file/handlers/management.rs`), resolves via **`get_by_id_and_user`** (owner-scoped, foreign→404), reads STORED geometry (no live re-parse), returns `{page_w,page_h,rects}` fractions; non-PDF / no-geometry → `200 {rects:[]}` (page-level fallback). `*_docs` + OpenAPI.

### Part K-UI — Knowledge base frontend

- **ITEM-26**: Module registration + IA — `createModule` routes (`/knowledge` list + `/knowledge/:id` detail, `KnowledgeBaseUse`, `lazyWithPreload`); `sidebarNavigation` `{id:'knowledge',icon:<Library/>,label:'Knowledge',path:'/knowledge',order:15}`; `stores`; `./extensions` import.
- **ITEM-27**: Stores — `KnowledgeBases`, `KnowledgeBaseDetail` (documents-with-status; **live via `sync:file_index_state` + `sync:knowledge_base_document`**; **refetch on `sync:file`** so external deletes update the count), `ConversationKnowledgeBases`; store-kit + self-gated `sync` subscriptions.
- **ITEM-28**: KB **list page** + `KnowledgeBaseCard` — `ProjectsListPage`/`ProjectCard` mirror; card shows derived doc-count + a status summary chip (all-indexed / M-indexing / K-failed / P no-text); full state trio; Load-More.
- **ITEM-29**: KB **create/edit drawer** — app `Drawer` + `Form`/`FormField` + zod (name required/unique, description); Save hidden without `KnowledgeBaseManage`.
- **ITEM-30**: KB **detail page** — `ProjectDetailPage` mirror: Overview card (`Statistic`/`Descriptions` + KB-level indexing `Progress` while any doc indexing + a **retrieval-mode line**: hybrid+rerank / hybrid / keyword-only, and embedding/reranker status), the Documents panel, a **"Used in"** card (bounded chip list), and a **direct KB search box** (reuses `search_knowledge` + hit rows + deep-link, so users verify retrieval outside chat); state: loading/not-found/else.
- **ITEM-31**: **Documents panel** — `ProjectFilesManagePanel`/`FileCard` mirror + KB specifics: kit `Upload` **`directory`+`multiple`** + drag-drop overlay; **virtualized/paginated** list (2,000-doc scale); per-doc **index-status badge** (indexing+`Spin` / indexed / failed+Retry / **no_text** advisory); **itemized batch-reject toast** (which files, why: size/type/duplicate); **dedup "N already in this KB" report**; **bulk "Retry all failed"**; cap counter `Tag`; multi-select bulk remove; live status (no blink). States: loading/empty/error.
- **ITEM-32**: Conversation KB attach — frontend `knowledge-base/chat-extension/extension.tsx`: `toolbar_plus_items` menu item + `toolbar_status` **"Knowledge · N" pill** (per-KB remove) + read-only **project-inherited** chips; `onConversationLoad` hydrate + `composeRequestFields` send ids; persist via `PUT /conversations/{id}/knowledge-bases`. Responsive wrap of pills.
- **ITEM-33**: KB **picker** (`ConversationKnowledgeBasePicker.tsx`) — searchable checklist of the user's KBs (name + doc-count + status); empty → link to `/knowledge`; always-mounted (`input_area_suffix`).
- **ITEM-34**: **project-extension** "Knowledge bases" knowledge-kind — inline preview (chips) + manage panel (attach/detach), mirroring `citations/project-extension`.

### Part C-UI — Citation + highlight UX

- **ITEM-35**: Citation **chips** — `[n]` tokenizer (`chatMarkdownPlugins.ts`) + `useStreamdownComponents` override (per `content.id`): focusable inline chip (info tone) + hover `Popover` preview (file · page · snippet); click/Enter → deep-link.
- **ITEM-36**: **Retrieval-transparency panel** — a `tool_result` renderer with `contentMatch` claiming ONLY `search_knowledge` blocks, modeled on `McpToolCallUI`: header "Searched K KBs · M chunks" + a **retrieval-mode line** + an **"indexing incomplete: S of T searchable"** banner when set; default collapsed; expanded chunk rows (file·page·score·snippet) → deep-link; **empty → "No matching passages found"** (grounded "not found").
- **ITEM-37**: **Deep-link + highlight overlay** — extend `PanelRendererMap['file']` with `{page,charRange}`; chip/row click → `displayInRightPanel` (chat) / `FilePreviewDrawer.openPreview` (elsewhere); in `pdf/body.tsx` add scroll-to-page + a `%`-positioned highlight box from the geometry endpoint (ITEM-25), landscape-aware; empty rects → page-level only. Text/markdown viewers scroll-to-offset.

### Part R-UI — Reranker admin UX + hub discoverability

- **ITEM-38**: Reranker admin — a "Reranker" capability `Switch` (mutually exclusive with chat) in `LlmModelCapabilitiesSection.tsx`; a `RerankSection.tsx` in `FileRagAdminPage` (all controls in `FormField`): reranker-model `Select` (`?capability=rerank`), enable `Switch`, candidate-k `InputNumber`, and a **hub discoverability nudge** ("No reranker installed — get **BGE-reranker-v2-m3** from the Hub to improve retrieval quality" → deep-links to the hub model). Wired via `FileRagAdmin.store.ts`.

### Part X — Cross-cutting

- **ITEM-39**: **OpenAPI + TS regen for BOTH binaries (`just openapi-regen`)** — runs AFTER all backend REST/type items (Parts R/I/K/C) and BEFORE the frontend items (execution-order constraint); `Knowledge*`/`Rerank*`/capability/`text-rects`/`FileIndexState`/`SyncEntity` types into `ui/` + `desktop/ui/`; golden `emit_ts` enforces.
- **ITEM-40**: Desktop parity — mirror the `knowledge-base` UI module + reranker/highlight edits into `src-app/desktop/ui/`; not blocklisted; `npm run check` green in `desktop/ui`.
- **ITEM-41**: **Gallery + state coverage + `gate:ui`** — every new surface auto-enumerates; a `STATE_COVERAGE` entry for every loading/error/empty/overlay/panel branch (incl. no_text, failed-retry, empty transparency panel, indexing-incomplete banner, office page-level fallback, **narrow-viewport** states); `GalleryStory` fixtures (`KnowledgeBaseCard` tones, `FileCard` statuses, citation chip, transparency panel empty+populated, highlight overlay portrait+landscape, reranker section w/ + w/o model); runtime-health zero-HIGH + Layer-A layout/axe across theme/accent/RTL.
- **ITEM-42**: Docs — `CLAUDE.md` "Knowledge Base Retrieval" + "Reranker capability (hub-delivered)" + "file_rag index-state" sections; the hub-repo change documented (schema+manifest+seed mirror); debug/test seams.

## UX principles (apply across Part K-UI / C-UI)

State trio everywhere · trust-first (numbered chips → hover preview → exact
highlight → transparency panel) · **honesty at scale** (surface indexing-incomplete
in chat, no_text/failed per doc, itemized batch rejects, dedup reports,
retrieval-mode line) · scale-aware (virtualized/paginated 2,000-doc list, live
status, folder upload) · scope legibility (composer pills + inherited chips) ·
**responsive** (pill wrap, right-panel overlay on narrow viewports, touch
virtualization) · kit+tokens only, `FormField` for settings, `data-testid`, AA
contrast, keyboard-operable · overlay hygiene (app `Drawer` higher-layer guard,
single-sibling Tooltip, controlled Confirm).

## Out of scope for v1 (roadmap)

- Structure-aware scientific-PDF ingest (GROBID/Docling; licensing: avoid Marker/Nougat) → separate initiative; enables table-atomic chunking + metadata-filtered retrieval.
- OCR for scanned PDFs (v1 marks them `no_text`, doesn't OCR); MedCPT domain-reranker A/B; MeSH query expansion; SPECTER2 "related papers"; Elicit-style extraction tables; shared/org KBs (cross-user RBAC); three-pane workspace; document versioning re-index policy beyond DEC-33; upload resume/recovery beyond DEC-34.

- **ITEM-43**: Promote previously-hardcoded retrieval/limit constants to admin settings on `file_rag_admin_settings` (migration 137): `kb_max_documents` (was KB_MAX_DOCUMENTS), `search_max_hit_chars` (was MAX_HIT_CHARS), `search_snippet_chars` (was 160), `search_max_top_k` (was the top_k clamp ceiling 50). Defaults preserve prior behaviour; `search_knowledge` + the KB attach handler read the settings; a `RetrievalLimitsSection` card on the Document RAG admin page exposes them. (The PDF-geometry line-merge tolerance stays a named const — a rendering heuristic, not a deployment policy.)

## Files to touch

Backend R: `ai-providers/src/{models/chat.rs,traits.rs,provider.rs,providers/openai.rs}`; `modules/llm_model/{models.rs,handlers/models.rs,types.rs}`; `modules/hub/handlers.rs` (capability map); `modules/memory/engine/{capability.rs,dispatch.rs}`; `modules/llm_local_runtime/{deployment/local.rs,auto_start.rs,proxy_router.rs,proxy_handlers.rs}`; `migrations/…135…`; `modules/file_rag/{models.rs,repository.rs,handlers.rs,retrieval.rs}`.
Hub H (in cloned `ziee-ai/hub` at /data/pbya/ziee/tmp/hub): `schemas/{2026-06-12,v1}/model.schema.json`, `models/bge-reranker-v2-m3-gguf.yaml`, regen `dist/`; mirror → `src-app/server/binaries/hub-seed/` + `SEED_HUB_VERSION`.
Backend I: `migrations/…136…`; `modules/file_rag/{ingest.rs,repository.rs,routes.rs,handlers.rs}`; `modules/sync/event.rs` (`FileIndexState`).
Backend K: `migrations/…133,134…`; `modules/mod.rs`, `core/repository.rs`; `modules/knowledge_base/{mod,permissions,models,repository,routes,handlers,tools}.rs` + `chat_extension/*`; `modules/mcp/chat_extension/mcp.rs`; `modules/sync/event.rs`; `tests/knowledge_base/*`.
Backend C: `migrations/…137…`; `modules/file/{processing/pdf.rs,utils/pdfium.rs,storage/*,ingest.rs,handlers/management.rs,routes.rs,types.rs}`; `modules/file_rag/{chunking.rs,ingest.rs,models.rs,repository.rs}`.
OpenAPI (mechanical): server/ui/desktop `openapi.json` + `api-client/types.ts`.
Frontend (`ui/src/` + mirror `desktop/ui/`): `modules/knowledge-base/{module.tsx,types.ts,stores/*,pages/*,components/*,chat-extension/*,project-extension/*}`; `modules/chat/core/{utils/chatMarkdownPlugins.ts,utils/useStreamdownComponents.tsx,stores/Chat.store.ts}`; `modules/file/{viewers/pdf/body.tsx,components/FilePanel.tsx,stores/FilePreviewDrawer.store.ts,chat-extension/extension.tsx}`; `modules/llm-provider/.../LlmModelCapabilitiesSection.tsx`; `modules/file-rag/{components/sections/RerankSection.tsx,stores/FileRagAdmin.store.ts}`; `dev/gallery/{stateCoverage.ts,stories/*,overlays.tsx}`; `tests/e2e/14-knowledge-base/*`.
Docs: `CLAUDE.md`.

## Patterns to follow

Reranker → mirror the embedding capability end-to-end. Hub model → mirror
`nomic-embed-text-v1-5-gguf.yaml` + the hub build pipeline. Index-state → mirror
`file_rag` ingest + the sync `publish` owner-scoped pattern. KB module/MCP/
collection/perms/sync → `web_search`/`citations`/`project`/`project_files`. Geometry
→ mirror text-page storage + `get_preview` gating. List/card/detail/form/upload/
settings/project-extension → `Projects*`/`ProjectFilesManagePanel`/`FileCard`/
`FileRagAdminPage`/`citations/project-extension`. Composer attach → `mcp`/`memory`
chat-extension; citation/transparency → `useStreamdownComponents`+`McpToolCallUI`;
deep-link → `InlineFilePreview.handleOpenInPanel`+`PanelRendererMap`. State/gallery
→ `stateCoverage.ts`+`GalleryStory`; store-kit `defineStore`+self-gated `sync`.

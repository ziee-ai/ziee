# PLAN — knowledge-base-retrieval

## Context (grounding)

ziee already contains a **complete hybrid RAG engine** (`file_rag`: page-aware
chunking → `halfvec` HNSW + `content_tsv` GIN → 4-arm Hybrid/Vector/FTS retrieval
with RRF, embed worker, dimension migration, backfill), reachable today only via
`files_mcp`'s `semantic_search` (scoped to a conversation's files). Embeddings run
through the shared `memory::engine::dispatch`.

**This feature delivers a first-class KNOWLEDGE BASE the agent retrieves from at
scale, with a carefully-designed UI/UX.** User-confirmed decisions: on-demand
`search_knowledge` MCP tool (no injection); user-owned KBs (standalone-reusable,
attach to chats/projects); exact-passage citation highlight overlay; a self-hosted
cross-encoder reranker. `grep rerank src-app/` = 0 hits (Part R is net-new).

**UI grounding (this replan's focus).** The frontend is web-only **shadcn
(new-york-v4) + Tailwind v4**, consumed through a **67-component kit** at
`@/components/ui` (import-only; `data-testid` required+unique; `Empty`/`ErrorState`/
`Spin` state trio; `Card`/`SectionHeader`/`Table`/`List`/`Tag`/`Badge`(tones)/
`Progress`/`Upload`(folder-capable)/`Attachment`/`Form`+`FormField`/`Confirm`).
Hard lint gates: **semantic tokens only** (`lint:colors` — no `bg-blue-500`/hex),
**logical direction** (`ps/pe/start`), **`FormField`/`Field` for every settings
control** (`lint:settings-field`), no antd/raw-DOM. The design contract is
`DESIGN_SYSTEM.md`; the kit contract is `components/ui/KIT_MANIFEST.md`. The app
uses `@/modules/layouts/app-layout/components/Drawer` (not kit `Sheet`) with known
overlay-stacking + tooltip-flicker gotchas. Every new surface auto-appears in the
**component gallery** and MUST register each loading/error/empty/overlay/panel
branch in `stateCoverage.ts` (compile-gated `check:state-matrix`) and pass
`gate:ui` (tsc + lint + runtime-health zero-HIGH + Layer-A layout/axe).

**Reference surfaces to mirror 1:1:** list ← `ProjectsListPage`; card ←
`ProjectCard`; detail ← `ProjectDetailPage`; create/edit ← `ProjectFormDrawer`
(Drawer + `Form`/`FormField` + zod); bulk ingest ← `ProjectFilesManagePanel` +
`FileCard`; settings ← `SettingsPageContainer` + `FileRagAdminPage` + `sections/*`;
project-extension knowledge-kind ← `citations/project-extension`. **Chat is an
extension registry** — KB attaches via its own frontend `chat-extension/extension.tsx`
(composer `+`-menu item + `toolbar_status` pill, mirroring the memory-mode pill /
MCP status row — NOT a new drawer); citation chips inject via a `[n]` tokenizer in
`chatMarkdownPlugins` + a `useStreamdownComponents` override; the transparency panel
is a `tool_result` renderer with a `contentMatch` claiming `search_knowledge`; the
citation deep-link opens the **chat right panel** (`displayInRightPanel`), which
requires extending `PanelRendererMap['file']` with `{page, charRange}`.

Worktree base = origin/main @ 9a6fb88c6 (highest migration 132); next free 133/134/135.

## Items

### Part R — Self-hosted cross-encoder reranker (shared capability)

- **ITEM-1**: `ai-providers` — `Rerank{Request,Response}` DTOs (`models/chat.rs`); `rerank()` on the `AIProvider` trait with a default "unsupported" impl; `Provider::rerank` wrapper; real impl only in `OpenAIProvider` (POST `{base_url}/rerank`).
- **ITEM-2**: Capability — `ModelCapabilities.rerank: Option<bool>` (JSONB, no migration); `"rerank"` in `ALLOWED_CAPABILITIES`; `rerank_unsupported_reason` guard.
- **ITEM-3**: Shared `dispatch::rerank(model_id, query, docs) -> Vec<(usize,f32)>` (`memory/engine/dispatch.rs`), mirroring `embed`.
- **ITEM-4**: Local serving — `--reranking`(+`--pooling rank`) in `llamacpp_argv`, capability→`config["reranking"]` in `auto_start.rs`, `proxy_rerank` handler + `/rerank` route in the explicit proxy allowlist.
- **ITEM-5**: Migration `00000000000135_add_file_rag_reranker.sql` — `ALTER file_rag_admin_settings ADD reranker_model_id UUID FK ON DELETE SET NULL / rerank_enabled BOOL DEFAULT FALSE / rerank_candidate_k INT DEFAULT 30 CHECK 1..200`.
- **ITEM-6**: `file_rag` settings plumbing — the three fields through `models.rs` (nullable-FK `Option<Option<Uuid>>`), `repository.rs::update_admin_settings` (SET+RETURNING), `handlers.rs` (range-validate + probe-rerank on set), publish `SyncEntity::FileRagAdminSettings`.
- **ITEM-7**: `file_rag` retrieval rerank stage — in `retrieval.rs::semantic_search`, gated `rerank_enabled && reranker_model_id.is_some()`: expand to `rerank_candidate_k` → `dispatch::rerank` → reorder → `truncate(top_k)`; on error keep pre-rerank order; preserve empty-scope/embed-failure guards.

### Part K — Knowledge base module (backend)

- **ITEM-8**: Migration `00000000000133_create_knowledge_bases.sql` — `knowledge_bases` (id, user_id FK CASCADE, name, description, document_count INT default 0, timestamps; per-user unique name), `knowledge_base_documents` (kb_id, file_id, added_at; composite PK; file_id index), `conversation_knowledge_bases`, `project_knowledge_bases` (composite-PK joins, all CASCADE).
- **ITEM-9**: Migration `00000000000134_grant_knowledge_base_permissions_to_users.sql` — idempotent grant `knowledge_base::use` + `knowledge_base::manage` to Users (mirror 104).
- **ITEM-10**: Module skeleton — `mod.rs` (`ModuleEntry` order 104; `knowledge_base_server_id()`; `init()` loopback upsert `/api/knowledge-base/mcp`; routes), `pub mod` in `modules/mod.rs`, `core/repository.rs` registration, `permissions.rs` (`KnowledgeBaseUse`/`KnowledgeBaseManage`).
- **ITEM-11**: `models.rs` — `KnowledgeBase`, `KnowledgeBaseDocument` (+ derived `index_status ∈ pending|indexing|indexed|failed`, `chunk_count`, `indexed_chunk_count`), `Create/Update/AttachDocumentsRequest`, `KnowledgeSearchHit` (SemanticHit + file name), all `JsonSchema`.
- **ITEM-12**: `repository.rs` — owner-scoped CRUD; `add_documents_capped` (atomic, `KB_MAX_DOCUMENTS = 2000`, tx-consistent `document_count`); `remove_documents`; `list_documents_with_status` (LEFT JOIN `file_chunks` counts → status + a doc-level indexing summary for the KB); `resolve_scope_file_ids(kb_ids, user_id)`; attach/detach KB↔conversation & KB↔project; `attached_kb_ids_for_conversation` (direct ∪ project read-through); `kb_attachment_targets` (which projects/chats a KB is attached to, for the detail page).
- **ITEM-13**: `routes.rs` + `handlers.rs` — REST CRUD + documents (attach existing / multipart **bulk** upload → `file::ingest::ingest_bytes` → attach, 422 on cap / detach / list-with-status) + `PUT/DELETE /conversations/{id}/knowledge-bases/{kb}` + `PUT/DELETE /projects/{id}/knowledge-bases/{kb}` + `GET /knowledge-bases/{id}/indexing-status` (poll/refresh for progress); `RequirePermissions` + owner-scope (foreign→404); mutations emit sync + `SyncOrigin`.
- **ITEM-14**: `tools.rs` + JSON-RPC — `list_knowledge_bases()` + `search_knowledge(query, knowledge_base_ids?, top_k?)`: resolve scope → `Repos.file_rag.get_admin_settings()` → `file_rag::retrieval::semantic_search` (reranked per Part R) → hits as text + `structuredContent` (file_id, name, page, char span, score); tool description carries the grounded-answer instruction. Both read-only → `knowledge_base::use`.
- **ITEM-15**: `chat_extension/{mod,extension,knowledge_base}.rs` (backend, order 24) — `ATTACH_FLAG="attach_knowledge_base_mcp"`; gate tool-capability; resolve `attached_kb_ids_for_conversation`; set flag + one-line KB-names note + grounding nudge (no chunk injection). Plus the two `mcp/chat_extension/mcp.rs` edits + `SyncEntity::KnowledgeBase`/`KnowledgeBaseDocument` in `sync/event.rs`.

### Part K-UI — Knowledge base frontend (the UX focus)

- **ITEM-16**: Module registration + IA — `ui/src/modules/knowledge-base/module.tsx` via `createModule`: `routes` (`/knowledge` list + `/knowledge/:id` detail, `AppLayoutDef`, `requiresAuth`, `permission: KnowledgeBaseUse`, `lazyWithPreload`); `slots.sidebarNavigation` entry `{ id:'knowledge', icon:<Library/>, label:'Knowledge', path:'/knowledge', order:15, permission:KnowledgeBaseUse }` (between Chats=10 and Projects=20); `stores`; side-effect import of `./extensions`.
- **ITEM-17**: KB stores — `KnowledgeBases.store.ts` (list/create/delete + `sync:knowledge_base` subscribe + `sync:reconnect` self-gate on `KnowledgeBaseUse`), `KnowledgeBaseDetail.store.ts` (single KB + documents-with-status + upload/attach/remove/multi-select + **live index-status via `sync:knowledge_base_document`** + poll fallback), `ConversationKnowledgeBases.store.ts` (per-conversation attached ids). `defineStore`/store-kit authoring.
- **ITEM-18**: KB **list page** (`KnowledgeBasesListPage.tsx`) — shell mirrors `ProjectsListPage`: `HeaderBarContainer` (`Title level={4}` "Knowledge" + `Can`-gated create icon `Button`); responsive `grid grid-cols-1 sm:grid-cols-2 gap-3 max-w-4xl` of `KnowledgeBaseCard`; Load-More paging with `aria-live` count. **State trio (canonical order):** data → grid; loading → `Spin`; error → `ErrorState resource="knowledge bases" onRetry`; empty → `Empty` (Library icon + concept one-liner + "Create knowledge base" CTA). Mutation errors → `message.error` toast.
- **ITEM-19**: `KnowledgeBaseCard.tsx` — mirrors `ProjectCard`: `Card` (hoverable, click→detail) with name (`Title`), `description` (ellipsis `Text`), a footer row of `Badge`/`Tag` — **"N documents"** + a status chip: `all indexed` (tone success) / `M indexing…` (tone warning, tiny inline `Spin`) / `K failed` (tone destructive); overflow `Dropdown` (Open / Rename / Delete). Delete via controlled `Confirm` (danger) decoupled from the row tooltip (avoid the documented tooltip-flicker).
- **ITEM-20**: KB **create/edit drawer** (`KnowledgeBaseFormDrawer.tsx`) — app `Drawer` (size default) + `Form`/`FormField` + zod: `name` (required, per-user-unique, trimmed) + `description` (optional `Textarea`). Footer `Cancel`(outline) + `Save`(loading); Save **hidden** without `KnowledgeBaseManage`. Fresh-open vs in-place-update reset semantics copied from `ProjectFormDrawer`.
- **ITEM-21**: KB **detail page** (`KnowledgeBaseDetailPage.tsx`) — shell mirrors `ProjectDetailPage`: `HeaderBarContainer` (back, truncating `Title`, `Can`-gated Edit + Delete); `DivScrollY` body; stacked `Card`s: **(a) Overview** — `Descriptions`/`Statistic` (documents, indexed/total, embedding model status, reranker on/off, created) + a KB-level **indexing `Progress`** bar shown only while any doc is `indexing`; **(b) Documents** — the panel (ITEM-22); **(c) Used in** — read-only `Tag` chips of projects/chats this KB is attached to (`kb_attachment_targets`). State: loading → `Spin`; not-found → `Result status="error"` (Retry + Back); else page.
- **ITEM-22**: **Documents panel** (`KnowledgeBaseDocumentsPanel.tsx`) — the core ingest UX, mirrors `ProjectFilesManagePanel` + `FileCard`, with KB specifics: kit `Upload` with **`directory` (folder-of-500) + `multiple`**, drag-drop overlay portal into the page, sticky header with a count `Tag` "N / 2000" (tones near/at cap) + Upload button + selection bar; per-doc rows via `FileCard variant="row"` showing during upload a circular `Progress`, and after upload an **index-status badge**: `indexing` (warning + `Spin`), `indexed` (success), `failed` (destructive + Retry). **Scale:** the list is **virtualized / paginated** (kit `Table virtualized` or `List` + Load-More) so 2,000 docs don't mount 2,000 cards. Multi-select bulk-remove via `dialog.confirm`. States: loading `Spin` (first load only), empty `Empty` ("Add documents" CTA + drop hint), error `ErrorState`. Live status via `sync:knowledge_base_document` (no full refetch blink).
- **ITEM-23**: Conversation KB attach — **frontend** `knowledge-base/chat-extension/extension.tsx` registering: `toolbar_plus_items` "Attach knowledge base" menu item (copy `McpMenuItem`) opening the KB picker; `toolbar_status` a **"Knowledge · N"** pill (copy `MemoryStatusPill`/`McpStatusRow`) listing attached KBs with per-KB remove, plus **read-only project-inherited KB chips** (distinct muted styling) so the active scope is visible; `onConversationLoad` hydrate + `composeRequestFields` send the attached ids; persist via `PUT /conversations/{id}/knowledge-bases`. No new drawer.
- **ITEM-24**: KB **picker** (`ConversationKnowledgeBasePicker.tsx`) — a `Dialog`/`Popover` with a searchable checklist (`MultiSelect` or list + `Checkbox`) of the user's KBs (name + doc-count + index status); empty → "You have no knowledge bases" + link to `/knowledge`; confirm applies the selection. Always-mounted via `input_area_suffix` (dropdown-unmount-safe, per the MCP modal precedent).
- **ITEM-25**: **project-extension** "Knowledge bases" knowledge-kind — `knowledge-base/project-extension/extension.tsx` (mirrors `citations/project-extension`): `inlinePreview` = attached-KB chips + count; `managePanel` = attach/detach picker binding KBs to the project (`PUT/DELETE /projects/{id}/knowledge-bases/{kb}`). Registers into the `knowledge_kinds` slot next to "Knowledge files" / "References".

### Part C — Citation + highlight UX (retrieve-to-source)

- **ITEM-26**: Citation **chips in answers** — a `[n]` tokenizer in `chat/core/utils/chatMarkdownPlugins.ts` + a component override in `useStreamdownComponents.tsx` (per `content.id`, mirroring the footnote `a()` / blockquote overrides): each `[n]` renders an inline, focusable chip (kit `Tag`/`Badge`, tone info) with a hover/focus `Popover` preview (file name · page · snippet); click/Enter → open the source (ITEM-28). Numbered per message, mapped from the `search_knowledge` `structuredContent`.
- **ITEM-27**: **Retrieval-transparency panel** — a `tool_result` renderer registered with a static `contentMatch` claiming only `search_knowledge` blocks (renders before the file catch-all), modeled on `McpToolCallUI`'s collapsible `Card`: header "Searched {K} knowledge base(s) · {M} chunks", default **collapsed**; expanded = a list of chunk rows (file name · page · score `Badge` · snippet), each row click → open source (ITEM-28). Empty result → "No matching passages found" (reinforces the grounded "not found" behavior).
- **ITEM-28**: **Deep-link contract** — extend `PanelRendererMap['file']` (`chat/core/stores/Chat.store.ts` + `file/chat-extension/extension.tsx`) with `{ page?: number; charRange?: [number,number] }`; chip/row click calls `displayInRightPanel({ type:'file', data:{ fileId, version, page, charRange } })`; thread through `FilePanel.tsx`. Outside chat (KB detail "view document") the same params flow through `FilePreviewDrawer.store.ts::openPreview(file, { page, charRange })`.
- **ITEM-29**: **PDF viewer highlight overlay** — in `file/viewers/pdf/body.tsx`: a `scroll-to-page` effect for `page`; wrap each page `<img>` in a `relative` container and render a `%`-positioned highlight box (semantic token fill, `rounded-sm`) for `charRange` rects, honoring the landscape-rotation transform; fetch rects from the endpoint (ITEM-31). **Graceful fallback:** empty rects (no-match / non-PDF) → scroll-to-page only, no box. Text/markdown viewers get scroll-to-offset (no box) as a lighter parallel.
- **ITEM-30**: **Alignment primitive + geometry** (backend, load-bearing risk) — `file/utils/pdfium.rs` relocates a chunk's cleaned-text content on the raw PDF via `page.text().search()` → per-char `tight_bounds()` → fraction-normalized rects (rotation-aware), empty-on-no-match. (Ingest-time geometry covering office docs = roadmap.)
- **ITEM-31**: **Geometry endpoint** — `GET /api/files/{id}/pages/{n}/text-rects?start=&end=` (`file/handlers/management.rs` + `routes.rs`), gated `FilesRead`, runs ITEM-30 in `spawn_blocking`, returns `{page_w,page_h,rects:[{x,y,w,h}]}` (fractions). `*_docs` + OpenAPI.

### Part R-UI — Reranker admin UX

- **ITEM-32**: Reranker admin surfaces — (a) a "Reranker" capability `Switch` (mutually exclusive with chat, like `text_embedding`) in `LlmModelCapabilitiesSection.tsx`; (b) a **`RerankSection.tsx`** card in `FileRagAdminPage` (via `SettingsPageContainer` + `sections/*`): reranker-model `Select` (`?capability=rerank`, empty→hint to tag a model), enable `Switch`, candidate-k `InputNumber` — **all inside `FormField`** (`lint:settings-field`); disabled/empty states; wired via `FileRagAdmin.store.ts`.

### Part X — Cross-cutting

- **ITEM-33**: OpenAPI + TS regen for BOTH binaries (`just openapi-regen`) — `Knowledge*` + `Rerank*`/capability + `text-rects` + `SyncEntity` types into `ui/` and `desktop/ui/`; golden `emit_ts` parity enforces it.
- **ITEM-34**: Desktop parity — mirror the `knowledge-base` UI module + reranker/highlight edits into `src-app/desktop/ui/`; not blocklisted (pgvector + local-runtime run on desktop); `npm run check` green in `desktop/ui`.
- **ITEM-35**: **Gallery + state coverage + `gate:ui`** — every new surface (KB list/detail, documents panel, form drawer, picker, project-extension panel, citation chip, transparency panel, PDF highlight overlay, reranker section) auto-enumerates as `gallery-page-*`/stories; add a `STATE_COVERAGE` entry for **every** loading/error/empty/overlay/panel branch (compile-gated `check:state-matrix`); add `GalleryStory` fixtures for `KnowledgeBaseCard` (all status tones), `FileCard` index-status, the citation chip, the transparency panel, and the highlight overlay (portrait+landscape); pass runtime-health (zero HIGH) + Layer-A layout/axe across theme/accent/RTL.
- **ITEM-36**: `CLAUDE.md` — a "Knowledge Base Retrieval" section (module, reuse-of-`file_rag`, `search_knowledge`, scoping, the UI surfaces + IA, the highlight overlay + its PDF-only best-effort caveat) and a "Reranker capability" note (new `rerank` capability, `--reranking` serving, retrieve→rerank→top-k); debug/test seams.

## UX principles (apply across all Part K-UI / Part C items)

- **State trio everywhere** — every data surface renders loading (`Spin`/skeleton), empty (`Empty` with a create/add CTA + a teaching one-liner), and error (`ErrorState resource onRetry`), in the canonical `data ? … : loading ? … : error ? … : empty` order; mutation-while-loaded errors are toasts, cold-load errors are inline.
- **Trust-first** (the life-science bar) — numbered citation chips, hover-preview the cited text, click → **exact-passage highlight** at the source page; the transparency panel makes "what was searched/found" auditable; grounded "not found" copy when empty.
- **Scale-aware** — the documents list is virtualized/paged (2,000-doc cap); indexing status is live (sync) with a KB-level progress bar; folder upload (`Upload directory`) for 500-PDF drops.
- **Scope legibility** — attached KBs are visible in the composer as pills; project-inherited KBs show as distinct read-only chips so the user always knows the active retrieval scope.
- **Kit + tokens only** — no raw DOM/antd, semantic tokens only, `FormField` for every settings control, logical-direction utilities, `data-testid` on every functional node, icon-buttons tooltipped; AA contrast via tokens; keyboard-operable chips/panels/upload.
- **Overlay hygiene** — use the app `Drawer`; respect the higher-layer-open guard when opening the file viewer from within a drawer; avoid the tooltip-flicker pattern (single sibling `Tooltip` + `data-tooltip-wrapped`; controlled `Confirm`).

## Out of scope for v1 (recorded roadmap)

- Structure-aware scientific-PDF ingest (GROBID/Docling sidecar) — separate initiative; benefits all consumers. **Licensing: GROBID/Docling/Unstructured (permissive); avoid Marker (GPL + $2M-cap weights) / Nougat (CC-BY-NC).**
- Element-typed/table-atomic chunking; metadata-filtered retrieval (reuse `lit_search`/`citations` + OpenAlex); **ingest-time char-geometry** (precise, office-covering highlight); MedCPT domain-reranker A/B; MeSH query expansion; SPECTER2 "related papers"; Elicit-style extraction tables; shared/org KBs; three-pane sources|chat|notes.

## Files to touch

Backend — reranker (R): `ai-providers/src/{models/chat.rs,traits.rs,provider.rs,providers/openai.rs}`; `modules/llm_model/{models.rs,handlers/models.rs,types.rs}`; `modules/memory/engine/{capability.rs,dispatch.rs}`; `modules/llm_local_runtime/{deployment/local.rs,auto_start.rs,proxy_router.rs,proxy_handlers.rs}`; `migrations/00000000000135_add_file_rag_reranker.sql`; `modules/file_rag/{models.rs,repository.rs,handlers.rs,retrieval.rs}`.
Backend — KB (K): `migrations/00000000000133_*`, `00000000000134_*`; `modules/mod.rs`, `core/repository.rs`; `modules/knowledge_base/{mod,permissions,models,repository,routes,handlers,tools}.rs` + `chat_extension/{mod,extension,knowledge_base}.rs`; `modules/mcp/chat_extension/mcp.rs`; `modules/sync/event.rs`; `tests/knowledge_base/*` + `tests/integration_tests.rs`.
Backend — highlight (C): `modules/file/{utils/pdfium.rs,handlers/management.rs,routes.rs,types.rs}`.
OpenAPI/generated (mechanical): `server/openapi/openapi.json`, `ui/openapi/openapi.json`, `desktop/ui/openapi/openapi.json`, `ui/src/api-client/types.ts`, `desktop/ui/src/api-client/types.ts`.
Frontend (`src-app/ui/src/` + mirror into `src-app/desktop/ui/`):
- `modules/knowledge-base/module.tsx`, `types.ts`
- `modules/knowledge-base/stores/{KnowledgeBases,KnowledgeBaseDetail,ConversationKnowledgeBases}.store.ts`
- `modules/knowledge-base/pages/{KnowledgeBasesListPage,KnowledgeBaseDetailPage}.tsx`
- `modules/knowledge-base/components/{KnowledgeBaseCard,KnowledgeBaseFormDrawer,KnowledgeBaseDocumentsPanel,ConversationKnowledgeBasePicker}.tsx`
- `modules/knowledge-base/chat-extension/extension.tsx` + `components/{KnowledgeStatusPill,KnowledgeAttachMenuItem,KnowledgeCitationChip,KnowledgeRetrievalPanel}.tsx`
- `modules/knowledge-base/project-extension/extension.tsx` + `components/*`
- chat wiring: `modules/chat/core/utils/{chatMarkdownPlugins.ts,useStreamdownComponents.tsx}`, `modules/chat/core/stores/Chat.store.ts` (`PanelRendererMap`)
- viewer/deep-link: `modules/file/viewers/pdf/body.tsx`, `modules/file/components/FilePanel.tsx`, `modules/file/stores/FilePreviewDrawer.store.ts`, `modules/file/chat-extension/extension.tsx`
- reranker UI: `modules/llm-provider/components/llm-models/shared/LlmModelCapabilitiesSection.tsx`, `modules/file-rag/components/sections/RerankSection.tsx`, `modules/file-rag/stores/FileRagAdmin.store.ts`
- gallery: `src/dev/gallery/{stateCoverage.ts, stories/*.story.tsx, overlays.tsx, deepStates.tsx}`
- E2E: `src-app/ui/tests/e2e/14-knowledge-base/*.spec.ts`
Docs: `CLAUDE.md`.

## Patterns to follow

- Reranker end-to-end → mirror the **embedding** capability (provider `openai.rs::embeddings`, `dispatch::embed`, the `embeddings:bool` local-flag thread, `proxy_embeddings`, `embedding_model_id` settings).
- KB module/MCP/collection/perms/sync → `web_search`+`citations`+`project`+`project_files`.
- **List page** → `ProjectsListPage`; **card** → `ProjectCard`; **detail** → `ProjectDetailPage`; **create/edit** → `ProjectFormDrawer`; **bulk ingest** → `ProjectFilesManagePanel` + `FileCard`; **settings section** → `FileRagAdminPage` + `SettingsPageContainer`; **project-extension** → `citations/project-extension`.
- **Composer attach** → `mcp/chat-extension` (`McpMenuItem`/`McpStatusRow`) + `memory` `MemoryStatusPill`; **citation chip/transparency** → `useStreamdownComponents` overrides + `McpToolCallUI`; **right-panel deep-link** → `InlineFilePreview.handleOpenInPanel` + `PanelRendererMap`.
- State/gallery → `stateCoverage.ts` entries + `GalleryStory` fixtures; `Empty`/`ErrorState`/`Spin`; app `Drawer`; store-kit `defineStore` + `sync:<entity>` self-gated subscription.

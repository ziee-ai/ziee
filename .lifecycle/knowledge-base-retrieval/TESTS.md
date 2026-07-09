# TESTS — knowledge-base-retrieval

Every ITEM (1–36) is covered by ≥1 TEST; user-visible UI items carry `tier: e2e`.
Real-path — only the model/provider boundary is mocked (loopback rerank/embed);
retrieval runs against real `file_rag` chunks. UI state branches are covered by
gallery + e2e. No cosmetic mocks.

## Part R — reranker (backend)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/providers/openai.rs` — asserts: `rerank` POSTs `{model,query,documents}` to `/rerank`, parses `results[{index,score}]`; the trait default returns "unsupported" for anthropic/gemini.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/memory/engine/capability.rs` — asserts: `rerank_unsupported_reason` passes only when `capabilities.rerank==Some(true)`; `"rerank"` in `ALLOWED_CAPABILITIES`.
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/llm_local_runtime/deployment/local.rs` — asserts: `llamacpp_argv` emits `--reranking`(+`--pooling rank`) iff `reranking=true`.
- **TEST-4** (tier: integration) [covers: ITEM-3, ITEM-1] file: `src-app/server/tests/file_rag/rerank_dispatch_test.rs` — asserts: `dispatch::rerank` reorders `(index,score)` against a loopback mock; errors when the model lacks `rerank`.
- **TEST-5** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/llm_local_runtime/rerank_proxy_test.rs` — asserts: the proxy forwards `/api/local-llm/v1/rerank` to the engine `/v1/rerank` (stub-engine `/v1/rerank`); a rerank-capable local model launches with the flags.
- **TEST-6** (tier: integration) [covers: ITEM-5, ITEM-6] file: `src-app/server/tests/file_rag/rerank_settings_test.rs` — asserts: migration 135 columns exist; `GET/PUT /file-rag/admin-settings` round-trips the three fields; range 400; probe-rerank rejects a non-rerank model.
- **TEST-7** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/file_rag/rerank_retrieval_test.rs` — asserts: with `rerank_enabled` + a score-inverting mock, `semantic_search` reorders + truncates to `top_k`; disabled → unchanged; rerank error → pre-rerank order.

## Part K — KB backend

- **TEST-8** (tier: unit) [covers: ITEM-10, ITEM-9] file: `src-app/server/src/modules/knowledge_base/permissions.rs` — asserts: `KnowledgeBaseUse/Manage::PERMISSION` strings match migration 134.
- **TEST-9** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/modules/knowledge_base/mod.rs` — asserts: deterministic `knowledge_base_server_id()`.
- **TEST-10** (tier: unit) [covers: ITEM-12] file: `src-app/server/src/modules/knowledge_base/repository.rs` — asserts: `KB_MAX_DOCUMENTS==2000`; the index-status derivation (0→pending; some<total→indexing; all→indexed; error→failed).
- **TEST-11** (tier: unit) [covers: ITEM-14] file: `src-app/server/src/modules/knowledge_base/tools.rs` — asserts: `tool_list()` schema + the grounded-answer instruction in `search_knowledge`; empty scope → no hits.
- **TEST-12** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/modules/knowledge_base/chat_extension/knowledge_base.rs` — asserts: attach flag + note only when tool-capable AND ≥1 KB; never chunk injection.
- **TEST-13** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `auto_attach_builtin_ids` push iff flag; `is_builtin_server_id(kb id)` true.
- **TEST-14** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/modules/sync/event.rs` — asserts: `KnowledgeBase`/`KnowledgeBaseDocument` snake_case wire strings.
- **TEST-15** (tier: integration) [covers: ITEM-8, ITEM-13] file: `src-app/server/tests/knowledge_base/crud_test.rs` — asserts: CRUD; per-user unique-name conflict; cascade delete.
- **TEST-16** (tier: integration) [covers: ITEM-12, ITEM-13] file: `src-app/server/tests/knowledge_base/documents_test.rs` — asserts: attach existing, bulk multipart upload (ingested+attached), detach; `document_count` consistent; 2001st → 422; `list_documents_with_status` derives per-doc status.
- **TEST-17** (tier: integration) [covers: ITEM-10, ITEM-13] file: `src-app/server/tests/knowledge_base/permissions_test.rs` — asserts: no `use` → 403; foreign KB → 404; Users member succeeds.
- **TEST-18** (tier: integration) [covers: ITEM-14, ITEM-12] file: `src-app/server/tests/knowledge_base/search_scope_test.rs` — asserts: FTS-only (no embed model) `search_knowledge` returns hits scoped to exactly the KB's files, nothing from another KB (isolation).
- **TEST-19** (tier: integration) [covers: ITEM-14, ITEM-7] file: `src-app/server/tests/knowledge_base/search_reranked_test.rs` — asserts: with embed model + `rerank_enabled` + mock reranker, `search_knowledge` returns reranked hits with full provenance (file_id, page, char span, score).
- **TEST-20** (tier: integration) [covers: ITEM-12, ITEM-13] file: `src-app/server/tests/knowledge_base/attachment_test.rs` — asserts: attach KB to conversation+project; union read-through; detach; foreign → 404; `kb_attachment_targets`.
- **TEST-21** (tier: integration) [covers: ITEM-15] file: `src-app/server/tests/knowledge_base/sync_emit_test.rs` — asserts: mutations emit owner-scoped `KnowledgeBase`/`KnowledgeBaseDocument`, never to another user.
- **TEST-22** (tier: integration) [covers: ITEM-10, ITEM-14] file: `src-app/server/tests/knowledge_base/mcp_test.rs` — asserts: JSON-RPC `initialize`+`tools/list`; `search_knowledge` gates on `use` (403); built-in row registered at loopback.
- **TEST-23** (tier: integration) [covers: ITEM-14, ITEM-15] file: `src-app/server/tests/knowledge_base/agent_retrieval_real_llm_test.rs` — asserts: a tool-capable model with a KB attached CALLS `search_knowledge` and answers from a KB-only fact. Requires tools+embed; skips only without a key/bridge (never `#[ignore]`-to-green).

## Part C — highlight (backend)

- **TEST-24** (tier: unit) [covers: ITEM-30] file: `src-app/server/src/modules/file/utils/pdfium.rs` — asserts: against a bundled PDF, the alignment routine relocates a known string → fraction rects in [0,1]; empty for a not-present string; landscape rotation applied.
- **TEST-25** (tier: integration) [covers: ITEM-31] file: `src-app/server/tests/file/text_rects_test.rs` — asserts: `GET /files/{id}/pages/{n}/text-rects` returns `{page_w,page_h,rects}` (non-empty for a real span, empty on no-match); gated `FilesRead` (403); foreign file → 404.

## Part K-UI / Part C / Part R-UI — frontend

- **TEST-26** (tier: unit) [covers: ITEM-17] file: `src-app/ui/src/modules/knowledge-base/stores/KnowledgeBases.store.ts` — asserts: create/delete reducers; `sync:knowledge_base` self-gates on `KnowledgeBaseUse`.
- **TEST-27** (tier: unit) [covers: ITEM-17, ITEM-22] file: `src-app/ui/src/modules/knowledge-base/stores/KnowledgeBaseDetail.store.ts` — asserts: documents reducer maps per-doc `index_status`; `sync:knowledge_base_document` patches a single row's status without a full refetch.
- **TEST-28** (tier: unit) [covers: ITEM-26, ITEM-27, ITEM-28] file: `src-app/ui/src/modules/knowledge-base/chat-extension/components/KnowledgeCitationChip.tsx` — asserts: the `structuredContent`→citation mapper yields numbered `{n,fileId,page,charRange,snippet}`; the deep-link builder produces the `displayInRightPanel` payload; the transparency mapper yields chunk rows.
- **TEST-29** (tier: e2e) [covers: ITEM-16, ITEM-18, ITEM-19, ITEM-20] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-list.spec.ts` — asserts: "Knowledge" nav entry → `/knowledge`; empty state; create via drawer; card shows doc-count + status; rename; delete (Confirm).
- **TEST-30** (tier: e2e) [covers: ITEM-21, ITEM-22] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-documents.spec.ts` — asserts: open KB detail; overview card; bulk upload (multi-file + folder input); per-doc index-status badge reaches `indexed`; failed-row retry affordance; cap counter; multi-select bulk remove; empty/loading states render.
- **TEST-31** (tier: e2e) [covers: ITEM-23, ITEM-24] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-attach-chat.spec.ts` — asserts: composer `+` → "Attach knowledge base" picker; empty-picker links to `/knowledge`; a "Knowledge · N" status pill appears with per-KB remove; project-inherited KBs show as read-only chips; the attachment persists across reload (hydrate).
- **TEST-32** (tier: e2e) [covers: ITEM-26, ITEM-27, ITEM-28, ITEM-29] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-citation-flow.spec.ts` — asserts: on a turn with a seeded `search_knowledge` result, numbered citation chips render with hover preview; the transparency panel expands to list chunks; clicking a chip opens the right panel at the cited page **with a highlight box when rects exist** and **falls back to page-only** when empty (portrait + landscape fixture).
- **TEST-33** (tier: e2e) [covers: ITEM-25] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-project-extension.spec.ts` — asserts: the "Knowledge bases" knowledge-kind on a project binds a KB and shows it in the inline preview.
- **TEST-34** (tier: e2e) [covers: ITEM-32] file: `src-app/ui/tests/e2e/14-knowledge-base/reranker-admin.spec.ts` — asserts: mark a model Reranker; on the file-rag admin page select it, enable, set candidate-k (all inside FormField); persists on reload; empty-model hint shown when none tagged.
- **TEST-35** (tier: e2e) [covers: ITEM-35, ITEM-18, ITEM-21, ITEM-22] file: `src-app/ui/tests/e2e/visual/knowledge-base.states.spec.ts` — asserts: gallery renders KB list (loaded/empty/error), KB detail (loaded/not-found/indexing), documents panel (empty/uploading/indexing/failed), picker, transparency panel, PDF highlight overlay (portrait+landscape), reranker section — all with zero runtime HIGH findings (drives `gate:ui` + `check:state-matrix` for `ui` and `desktop/ui`).

## Part X — cross-cutting

- **TEST-36** (tier: integration) [covers: ITEM-33, ITEM-11] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: `openapi::emit_ts::tests::types_ts_parity` green (KB + rerank + text-rects types regenerated) for the server binary; desktop parity via `npm run check (desktop/ui)` (ITEM-34).
- **TEST-37** (tier: unit) [covers: ITEM-34] file: `src-app/desktop/ui/src/modules/knowledge-base/module.tsx` — asserts: the KB module is present and NOT in `CORE_MODULE_BLOCKLIST` (operationally confirmed by `npm run check (desktop/ui): PASS` at phase 8).
- **TEST-38** (tier: unit) [covers: ITEM-36] file: `src-app/server/tests/knowledge_base/mod.rs` — asserts: `CLAUDE.md` contains the "Knowledge Base" header, names `search_knowledge`, and mentions the `rerank` capability.

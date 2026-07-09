# TESTS — knowledge-base-retrieval

Every ITEM is covered by ≥1 TEST. Frontend items carry `tier: e2e`. Real-path,
no cosmetic mocks — only the LLM/provider boundary is mocked where noted;
retrieval runs against real `file_rag` chunks in a real DB.

## Backend — unit (`#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-4, ITEM-2] file: `src-app/server/src/modules/knowledge_base/permissions.rs` — asserts: `KnowledgeBaseUse::PERMISSION == "knowledge_base::use"` and `KnowledgeBaseManage::PERMISSION == "knowledge_base::manage"`, matching the exact strings granted in migration 134.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/knowledge_base/mod.rs` — asserts: `knowledge_base_server_id()` is deterministic (`Uuid::new_v5(NAMESPACE_URL, b"knowledge_base.ziee.internal")`) and stable across calls.
- **TEST-3** (tier: unit) [covers: ITEM-5, ITEM-7] file: `src-app/server/src/modules/knowledge_base/repository.rs` — asserts: `KB_MAX_DOCUMENTS == 2000` and the pure index-status derivation (`chunk_count==0 → pending`, `chunk_count>0 && embedded<chunk_count → indexed(partial)`, all-embedded → indexed, error-marker → failed).
- **TEST-4** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/knowledge_base/tools.rs` — asserts: `tool_list()` contains `search_knowledge` + `list_knowledge_bases` with the documented JSON schema; empty-scope short-circuits to no hits (mirrors `semantic_search` empty-scope guard).
- **TEST-5** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/modules/knowledge_base/chat_extension/knowledge_base.rs` — asserts: the attach decision — flag set + note injected only when tool-capable AND ≥1 KB resolved; no flag when zero KBs or non-tool-capable model; no chunk/context injection ever.
- **TEST-6** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `auto_attach_builtin_ids` pushes `knowledge_base_server_id()` iff the attach flag == "true"; `is_builtin_server_id(knowledge_base_server_id())` is true (approval-bypass).
- **TEST-7** (tier: unit) [covers: ITEM-11] file: `src-app/server/src/modules/sync/event.rs` — asserts: `SyncEntity::KnowledgeBase` / `KnowledgeBaseDocument` serialize to the expected snake_case wire strings (drives the `sync:<entity>` TS keys).

## Backend — integration (`tests/knowledge_base/`, Postgres + spawned server)

- **TEST-8** (tier: integration) [covers: ITEM-1, ITEM-7] file: `src-app/server/tests/knowledge_base/crud_test.rs` — asserts: create/list/get/patch/delete a KB; per-user unique-name conflict; cascade delete removes `knowledge_base_documents` rows.
- **TEST-9** (tier: integration) [covers: ITEM-5, ITEM-6, ITEM-7] file: `src-app/server/tests/knowledge_base/documents_test.rs` — asserts: attach existing files, multipart bulk upload (files land, get ingested, are attached), detach; `document_count` stays consistent; the 2001st document returns **422**.
- **TEST-10** (tier: integration) [covers: ITEM-4, ITEM-6] file: `src-app/server/tests/knowledge_base/permissions_test.rs` — asserts: a user without `knowledge_base::use` gets 403; a foreign user's KB id returns 404 (owner-scope); Users-group member (granted by migration 134) succeeds.
- **TEST-11** (tier: integration) [covers: ITEM-5, ITEM-8] file: `src-app/server/tests/knowledge_base/search_fts_test.rs` — asserts: with NO embedding model configured (airgapped path), `search_knowledge` over a KB of ingested text files returns FTS hits scoped to exactly that KB's files, and returns nothing from files in a *different* KB (scope isolation). Backs the reuse-of-file_rag claim on the real retrieval path.
- **TEST-12** (tier: integration) [covers: ITEM-8, ITEM-5] file: `src-app/server/tests/knowledge_base/search_vector_test.rs` — asserts: with an embedding model configured on `file_rag_admin_settings`, `search_knowledge` returns hybrid hits carrying full provenance (file_id, page_number, char_start/char_end, score). Uses the same embedding-provider seam the file_rag tests use (no paid key).
- **TEST-13** (tier: integration) [covers: ITEM-6, ITEM-9] file: `src-app/server/tests/knowledge_base/attachment_test.rs` — asserts: attach KB to a conversation and to a project; `attached_kb_ids_for_conversation` returns the union (conversation-direct ∪ project read-through); detach removes it; foreign conversation/project → 404.
- **TEST-14** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/knowledge_base/sync_emit_test.rs` — asserts: KB create/update/delete and document attach/detach each emit an owner-scoped `SyncEntity::KnowledgeBase`/`KnowledgeBaseDocument` event (via `SyncProbe`), and never to another user.
- **TEST-15** (tier: integration) [covers: ITEM-8, ITEM-10] file: `src-app/server/tests/knowledge_base/mcp_test.rs` — asserts: JSON-RPC `initialize` + `tools/list` at `/api/knowledge-base/mcp` returns the two tools; `tools/call search_knowledge` gates on `knowledge_base::use` (403 without); the built-in server row is registered (`is_built_in=true`) with the loopback URL.

## Backend — real-LLM integration (gated on provider key / local bridge)

- **TEST-16** (tier: integration) [covers: ITEM-8, ITEM-9, ITEM-10] file: `src-app/server/tests/knowledge_base/agent_retrieval_real_llm_test.rs` — asserts: a tool-capable model in a conversation with a KB attached actually CALLS `search_knowledge` and its answer reflects a fact that exists ONLY in the KB document (not in the prompt). Mirrors `file_rag`'s `semantic-search-chat-real-llm` + `project/injection_test.rs`; requires `capabilities.tools=true` and an embedding model; skips only if no provider key/bridge (never `#[ignore]`-to-green).

## Frontend — unit (store logic)

- **TEST-17** (tier: unit) [covers: ITEM-14] file: `src-app/ui/src/modules/knowledge-base/stores/KnowledgeBases.store.ts` — asserts: create/delete reducers update the list; `sync:knowledge_base` handler self-gates on `knowledge_base::use` (no refetch without perm).
- **TEST-18** (tier: unit) [covers: ITEM-15, ITEM-18] file: `src-app/ui/src/modules/knowledge-base/stores/KnowledgeBaseDetail.store.ts` — asserts: document-list reducer maps per-doc `index_status`; the citation-hit mapper turns a `search_knowledge` `structuredContent` hit into `{fileId, page}` for the viewer deep-link.

## Frontend — e2e (`ui/tests/e2e/14-knowledge-base/`, Playwright)

- **TEST-19** (tier: e2e) [covers: ITEM-13, ITEM-14] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-list.spec.ts` — asserts: user navigates to `/knowledge`, sees the empty state, creates a KB, sees it listed, deletes it.
- **TEST-20** (tier: e2e) [covers: ITEM-15] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-documents.spec.ts` — asserts: open a KB, upload document(s), see them listed with an index-status badge that reaches `indexed`, remove one.
- **TEST-21** (tier: e2e) [covers: ITEM-16, ITEM-18] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-chat-retrieval.spec.ts` — asserts: attach a KB to a chat, send a question, the agent's turn shows a `search_knowledge` call and a citation chip; clicking the chip opens the file viewer at the cited page. (Real-LLM-gated for the answer; the attach + citation-chip render + deep-link are asserted on a seeded tool-result even when the LLM is skipped.)
- **TEST-22** (tier: e2e) [covers: ITEM-17] file: `src-app/ui/tests/e2e/14-knowledge-base/kb-project-extension.spec.ts` — asserts: on a project detail page, the "Knowledge bases" knowledge-kind lets the user bind a KB and see it in the inline preview.
- **TEST-23** (tier: e2e) [covers: ITEM-19, ITEM-20] file: `src-app/ui/tests/e2e/visual/knowledge-base.states.spec.ts` — asserts: gallery renders KB list (loaded/empty/error), KB detail, and the project-extension panel with zero runtime HIGH findings (drives `npm run gate:ui` + `check:state-matrix` coverage for both `ui` and `desktop/ui`).

## Cross-cutting gate lines (recorded in TEST_RESULTS.md at phase 8)

- **TEST-24** (tier: integration) [covers: ITEM-12] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: `openapi::emit_ts::tests::types_ts_parity` is green (regen ran; `types.ts` matches `openapi.json`) for the server binary. (Desktop parity captured by `npm run check (desktop/ui)` per ITEM-20.)
- **TEST-25** (tier: unit) [covers: ITEM-21] file: `src-app/server/tests/knowledge_base/mod.rs` — asserts: a docs-presence check that `CLAUDE.md` contains the "Knowledge Base" section header and names the `search_knowledge` tool (guards against shipping the module undocumented; mirrors the naming-lint spirit).

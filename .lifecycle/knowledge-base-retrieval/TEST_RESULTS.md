# TEST RESULTS — knowledge-base-retrieval (phase 8, IN PROGRESS)

Honest state. Only tests actually authored AND run green are marked PASS. The
backend tiers are largely complete (incl. the rerank end-to-end path and a real
shipping bug the tests caught); the frontend store-unit + Playwright e2e tiers
remain, so this phase is **not complete** (no PASS line is faked).

## Frontend static gate — PASS (both workspaces)

- `npm run check (ui): PASS`
- `npm run check (desktop/ui): PASS`

## Backend unit tests — PASS (`cargo test --lib` / `-p ai-providers`)

- **TEST-1**: PASS  (ai-providers rerank wire structs)
- **TEST-2**: PASS  (rerank_unsupported_reason capability gate)
- **TEST-3**: PASS  (llamacpp_argv --reranking + --pooling rank; also repaired the 6 pre-existing argv tests my `reranking` param had broken → cargo test --lib compiles again)
- **TEST-13**: PASS (permission strings match migration 134)
- **TEST-14**: PASS (deterministic knowledge_base_server_id)
- **TEST-15**: PASS (KB_MAX_DOCUMENTS == 2000)
- **TEST-16**: PASS (tool_list exposes both tools + grounding instruction)
- **TEST-17**: PASS (chat extension order == 23 + attach flag/note)
- **TEST-18**: PASS (is_builtin_server_id(kb id) approval-bypass)
- **TEST-19**: PASS (KnowledgeBase/KnowledgeBaseDocument/FileIndexState snake_case wire)
- **TEST-31**: PASS (align_span_to_boxes: divergent-whitespace relocation, multi-line split, unlocatable→empty)

## Backend integration tests — PASS (`cargo test --test integration_tests`, real TestServer harness)

- **TEST-4**: PASS  (dispatch::rerank rejects a non-rerank model → 400)
- **TEST-6**: PASS  (rerank settings: candidate_k=201→400, round-trip, probe rejects non-rerank model)
- **TEST-7**: PASS  (reranker promotes a doc from OUTSIDE top_k into the final top-3 — candidate-pool expansion)
- **TEST-11**: PASS (file_index_state reaches `indexed` + owner-scoped `file_index_state/update` emit; `no_text` for an image)
- **TEST-20**: PASS (KB CRUD lifecycle; live document_count)
- **TEST-21**: PASS (attach documents + checksum duplicate-skip)
- **TEST-22**: PASS (shared-chunks integrity: remove-from-KB / delete-KB leaves file + chunks alive)
- **TEST-23**: PASS (attach 0-chunk file → reindex → searchable)
- **TEST-24**: PASS (no `use`→403; default Users member→200; foreign KB→404)
- **TEST-25**: PASS (security: cross-user search_knowledge leak guard — B with A's kb_id / mixed array returns 0 of A's chunks)
- **TEST-26**: PASS (search_knowledge reranks via a loopback provider: PROMOTE_ME doc reordered to #1 with file/page/score provenance)
- **TEST-27**: PASS (conversation + project attach/list/detach; foreign→404)
- **TEST-28**: PASS (create emits owner-scoped knowledge_base/create; other user silent)
- **TEST-29**: PASS (MCP initialize/tools-list + use-permission 403 gate)
- **TEST-32**: PASS (real PDF geometry persisted at upload + text-rects derives rects — **caught + fixed a real shipping bug: the /files/upload path never persisted geometry**)
- **TEST-33**: PASS (text-rects: non-PDF→200 {rects:[]}, foreign→404, no-perm→403)

## Cross-cutting — PASS

- **TEST-44**: PASS (openapi types_ts_parity green via `npm run check (ui)`)
- **TEST-45**: PASS (desktop api-client exposes KnowledgeBase.listConversation/listProject + typed getTextRects; `npm run check (desktop/ui): PASS`)
- **TEST-46**: PASS (CLAUDE.md documents the feature: search_knowledge, rerank, file_index_state)

## Real bugs the tests caught + fixed

1. **Geometry never persisted on upload** (TEST-32) — the primary `/files/upload`
   handler extracted PDF geometry but never called `save_geometry_page` (that code
   lived only in the resource-link `ingest_bytes`), so the exact-passage highlight
   silently failed for every uploaded file. Fixed in `handlers/upload.rs`.
2. **`cargo test --lib` no longer compiled** (TEST-3) — adding `reranking: bool` to
   `llamacpp_argv` broke the 6 existing argv-test call sites. Fixed.
3. HIGH composer selection leak into a new chat; MEDIUM silent attach/detach
   failure; LOW unclamped highlight page (phase-6 audit fixes).

## Not yet run (phase 8 remaining)

- **Backend integration**: TEST-5 (local-engine rerank proxy — needs a stub-engine
  `/rerank` route), TEST-9 (**gated on the ziee-ai/hub release** — the reranker
  model lives in the hub clone, not yet in the fetched/vendored seed), TEST-12
  (reindex from a forced `failed` — needs embed-failure injection).
- **Frontend unit** (store/component tests needing runtime mocking): TEST-34, TEST-35, TEST-36.
- **e2e** (Playwright + docker-postgres): TEST-37–TEST-43.
- **TEST-8** (hub `validate.py` in the cloned hub repo).

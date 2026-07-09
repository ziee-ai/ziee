# TEST RESULTS — knowledge-base-retrieval (phase 8, IN PROGRESS)

Honest state. Only tests that were actually authored AND run green are marked
PASS. The backend integration tier + the Playwright e2e tier are not yet
authored/run — they are listed under "Not yet run" and this phase is therefore
**not complete** (the gate stays PENDING; no PASS line is faked).

## Frontend static gate — PASS (both workspaces)

- `npm run check (ui): PASS`
- `npm run check (desktop/ui): PASS`

(These cover tsc + biome guardrails + lint:colors/settings-field/logical-direction
+ check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix/
overlay-registry, in each workspace.)

## Backend unit tests — authored + run green (`cargo test --lib` / `-p ai-providers`)

- **TEST-1**: PASS  (ai-providers `rerank_wire_tests` — 2/2)
- **TEST-2**: PASS  (`capability::tests::rerank_gate_requires_rerank_capability`)
- **TEST-3**: PASS  (`local::tests::llamacpp_argv_reranking_emits_pooling_rank`; also repaired the 6 pre-existing argv tests my `reranking` param had broken)
- **TEST-13**: PASS  (`permissions::tests::permission_strings_match_migration_134`)
- **TEST-14**: PASS  (`knowledge_base::id_tests::knowledge_base_server_id_is_stable`)
- **TEST-15**: PASS  (`knowledge_base::models::cap_tests::kb_max_documents_is_2000`)
- **TEST-16**: PASS  (`tools::schema_tests::tool_list_exposes_both_tools_with_grounding_instruction`)
- **TEST-17**: PASS  (`chat_extension::extension::order_tests::extension_order_is_23` + the pre-existing `apply_attach_sets_flag_and_prepends_note`)
- **TEST-18**: PASS  (`mcp::kb_builtin_tests::knowledge_base_id_is_a_builtin`)
- **TEST-19**: PASS  (`sync::event::kb_wire_tests::kb_entities_serialize_snake_case`)
- **TEST-31**: PASS  (`file::handlers::management::align_tests` — 3/3: divergent-whitespace span, multi-line split, unlocatable→empty)

## Cross-cutting — PASS

- **TEST-44**: PASS  (`openapi::emit_ts::tests::types_ts_parity` — asserted green by `npm run check (ui)`; the golden parity test regenerates `types.ts` from the committed `openapi.json`)
- **TEST-45**: PASS  (desktop api-client exposes `KnowledgeBase.listConversation/listProject` + types `File.getTextRects` as `TextRectsResponse`; confirmed by `npm run check (desktop/ui): PASS` — see DRIFT-1.3: no desktop UI module by design)

## Not yet run (phase 8 remaining — this is why the gate stays PENDING)

- **Backend integration** (needs the TestServer harness + loopback embed/rerank
  stubs): TEST-4, TEST-5, TEST-6, TEST-7, TEST-9, TEST-10 (status derivation is
  SQL-side → integration), TEST-11, TEST-12, TEST-20–TEST-30, TEST-32, TEST-33.
- **Frontend unit** (store/component tests needing runtime mocking):
  TEST-34, TEST-35, TEST-36.
- **e2e** (Playwright + docker-postgres + a stubbed embed path):
  TEST-37–TEST-43.
- **TEST-8** (hub `validate.py` in the cloned hub repo).

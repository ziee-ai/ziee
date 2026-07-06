# TEST_RESULTS — org-migration-hub-ids

Scoped to the change (hub seed + migration + hub tests). Logs under
`/data/pbya/ziee/tmp/lifecycle-logs/`.

- **TEST-1**: PASS — `cargo test --lib -p ziee hub::hub_manager` → 18 passed;
  `seed_manifest_json_round_trips_into_structs` resolves the renamed
  `models/io.github.ziee-ai/...` + `assistants/io.github.ziee-ai/...` seed files
  and asserts each `name == io.github.ziee-ai/*`.
- **TEST-2**: PASS — same run; `is_safe_name_accepts_reverse_dns_rejects_traversal`
  accepts `io.github.ziee-ai/llama-3-1-8b-instruct`.
- **TEST-3**: PASS — `cargo test --test integration_tests hub::catalog_v1::` →
  **15 passed / 0 failed** (was 10/5 before ITEM-10). Covers the rebranded catalog
  index, the `io.github.ziee-ai%2Fllama-3-1-8b-instruct` manifest fetch (name ==
  `io.github.ziee-ai/llama-3-1-8b-instruct`), and a tracked `hub_id =
  io.github.ziee-ai/code-reviewer` install.
- **TEST-4**: PASS — `cargo test --test integration_tests hub::test_create_model_from_hub`
  → 6 passed / 0 failed; the `io.github.ziee-ai/llama-3-1-8b-instruct` model-manifest
  lookups + hub_id insert resolve. (`test_duplicate_download_prevention` also passed
  in the full `hub::` run.)
- **TEST-5**: PASS — `cargo test --test integration_tests hub::migration_test::` →
  **5 passed / 0 failed**: the 3 migration-92 isolation tests + the 2 new
  migration-131 tests (`hub_ids_phibya_rewrite_migrates_and_is_idempotent`,
  `hub_id_slug_composes_through_92_then_131`). The prefix-substring rewrite was
  additionally verified directly against real Postgres (correct + idempotent +
  non-target rows untouched).
- **TEST-6**: PASS — `cargo test --lib -p ziee workflow_mcp::tools` → 22 passed;
  `slug_maps_separators_to_underscore` yields
  `wf_io_github_ziee-ai_research-summarize-write`.
- **TEST-8**: PASS — in the `ziee-ai/hub` clone: `python3 scripts/validate.py` →
  "OK — 28 manifests validated"; `scripts/build-pages.py` → emits
  `assistants/io.github.ziee-ai/*` + `models/io.github.ziee-ai/*`, 12 ziee-ai
  items / 0 phibya, and a built `code-reviewer/1.0.0.json` byte-identical to the
  vendored seed manifest.

## Notes (out of scope / gated)
- `tests/hub/catalog_v1.rs` carried 5 pre-existing 403 failures + stale seed
  counts (ITEM-10) — proven pre-existing (identical failures on a rebrand-free
  base checkout); fixed and now green.
- `tests/hub/catalog_hermetic.rs` has 2 pre-existing failures
  (`install_as_system_mcp_with_replace_existing_succeeds` → `unwrap on None`;
  `replace_existing_preserves_admin_tunable_fields_mcp` → `RESOURCE_CONFLICT`).
  These use a **hermetic mock catalog** (`io.github.test/*`), structurally
  unrelated to the seed rename, and are not in any file this change touches — a
  test-isolation/state-leak issue left to its owner.
- `tests/llm_model/download_test.rs`'s 1-line fixture rename is exercised only by
  the HF-download suite, which is **secret-token-gated** (no `HUGGINGFACE_API_KEY`
  on this host); its correctness (const matches a real seed model name) is covered
  by TEST-1.

# TEST_RESULTS — project-search

Scoped test run per Phase 3 (TESTS.md). Logs under
`/data/pbya/ziee/tmp/lifecycle-logs/`.

## Commands

```bash
# unit + openapi parity (lib)
cargo test --lib -p ziee -- \
  project::handlers::tests::project_list_query_deserializes_search \
  project::handlers::tests::normalize_search_trims_and_blanks_to_none \
  openapi::emit_ts::tests::types_ts_parity
# → test result: ok. 4 passed; 0 failed  (incl. types_ts_parity_desktop)

# integration (real HTTP + real per-test DB)
cargo test --test integration_tests project::search_test -- --test-threads=1
# → test result: ok. 6 passed; 0 failed; finished in 10.37s
```

## Results (every Phase-3 TEST-ID)

- **TEST-1**: PASS — `project_list_query_deserializes_search` (unit)
- **TEST-2**: PASS — `normalize_search_trims_and_blanks_to_none` (unit)
- **TEST-3**: PASS — `search_by_name_case_insensitive` (integration)
- **TEST-4**: PASS — `search_matches_description` (integration)
- **TEST-5**: PASS — `blank_and_absent_return_all` (integration)
- **TEST-6**: PASS — `search_is_ownership_scoped` (integration)
- **TEST-7**: PASS — `openapi::emit_ts::tests::types_ts_parity` (unit; `types_ts_parity_desktop` also green)
- **TEST-8**: PASS — `filtered_total_survives_page_truncation` (integration)
- **TEST-9**: PASS — `multi_match_and_wildcard_metacharacters` (integration)

Totals: unit/parity 4 passed / 0 failed; integration 6 passed / 0 failed.
No `#[ignore]`, no skips.

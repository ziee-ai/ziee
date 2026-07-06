# TESTS — project-search

Every ITEM maps to ≥1 TEST. Tiers mirror the codebase: unit `#[cfg(test)]` in
the module, integration in `tests/project/`, and the openapi golden lib test.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/project/handlers.rs` — asserts: `PaginationQuery` deserializes a `search` field (`{"search":"foo"}` → `Some("foo")`, absent → `None`), proving the field is wired into the extractor.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/project/handlers.rs` — asserts: `normalize_search` maps `None`/`""`/`"   "` → `None` and `"  foo "` → `Some("foo")` (trim-to-None convention).
- **TEST-3** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/project/search_test.rs` — asserts: `GET /projects?search=road` over seeded projects {"Roadmap","Backlog","Design"} returns only "Roadmap"; `total==1`; case-insensitive (`search=ROAD` also matches).
- **TEST-4** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/project/search_test.rs` — asserts: search matches on `description` too — a project named "Alpha" with description "quarterly roadmap" is returned by `search=roadmap`.
- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/project/search_test.rs` — asserts: no-filter baseline — `?search=` (blank) and no `search` param both return ALL of the user's projects with the correct `total`, proving blank normalizes to "no predicate".
- **TEST-6** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/project/search_test.rs` — asserts: ownership scoping — user B owns a "Roadmap" project; user A's `search=road` returns 0 (the predicate never widens the existing `user_id = $1` scope).
- **TEST-7** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden test passes — the committed `openapi.json`/`types.ts` regenerate identically, proving the spec was regenerated in lockstep with the new `search` query param.

## Coverage matrix

| ITEM | Tests |
|------|-------|
| ITEM-1 | TEST-1 |
| ITEM-2 | TEST-2, TEST-5 |
| ITEM-3 | TEST-3, TEST-4, TEST-5, TEST-6 |
| ITEM-4 | TEST-7 |

# DRIFT-2 — implementation vs plan (round 2)

Triggered by the Phase-8 test run surfacing failures in a touched file.

- **DRIFT-2.1** — verdict: impl-wins — Running `hub::catalog_v1::` showed 5 tests
  failing (403 on `/hub/{version,index,manifest}` + a 28-vs-29 seed-count
  mismatch). Investigation proved these are **pre-existing on origin/main**:
  a clean base checkout (bde32e12, still `io.github.phibya`, zero rebrand) fails
  the *identical* 5 tests with the *identical* 403s (10 passed / 5 failed). Cause:
  a hub-permission refactor moved catalog reads behind `HubCatalogRead` but left
  these 5 test fns granting `hub::models::read`, and the seed gained a 10th
  workflow (`io.github.ziee/sr-review`) without the test constants being bumped.
  The original plan did not foresee pre-existing test-debt in a touched file, so
  the plan was AMENDED with **ITEM-10** (fix the 5 grants → `hub::catalog::read`;
  bump `SEED_ITEM_COUNT` 28→29 + `workflows` 9→10). Re-ran Phases 1–3 gates.
- **DRIFT-2.2** — verdict: none — the fix is scope-limited: only the 5
  catalog-read tests + 2 constants changed; the `refresh`/`installed` tests keep
  `hub::models::read` (they gate different endpoints). No production code touched.
- **DRIFT-2.3** — verdict: none — the added hunks live entirely in
  `tests/hub/catalog_v1.rs`, already covered by its wide `AUDIT_COVERAGE.tsv` row
  (tests-quality, api-contract, correctness), so the Phase-6 coverage law still
  holds without a TSV edit; a ledger row records the pre-existing-fix review.

**Unresolved drifts:** 0

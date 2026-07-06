# FIX_ROUND-2 — org-migration-hub-ids

## Confirmed findings addressed
The Phase-8 run surfaced 2 confirmed (pre-existing, base-reproduced) findings in
the touched file `tests/hub/catalog_v1.rs`, both now fixed under ITEM-10:

1. `perms-authz` — 5 catalog-read tests granted `hub::models::read` while the
   `/hub/{version,index,manifest}` handlers require `HubCatalogRead` → 403.
   Fixed: grants → `hub::catalog::read` (matches the handler contract + the
   sibling `catalog_read_cannot_refresh` test). `refresh`/`installed` grants left
   as `hub::models::read` (they gate different endpoints).
2. `tests-quality` — `SEED_ITEM_COUNT` (28) + `workflows` (9) were stale vs the
   actual 29-item / 10-workflow seed. Fixed: 29 / 10.

Result: `hub::catalog_v1::` is now **15 passed / 0 failed** (was 10/5).

## Re-audit pass
Re-reviewed the amended `catalog_v1.rs` hunks:
- The grant edits are limited to the 5 catalog-read tests; a grep confirms the
  only remaining `hub::models::read` grants are the `refresh` + `installed` tests
  (both still green, both intentionally gating non-catalog endpoints).
- The count edits match the live seed (`index.json` has 10 workflows / 29 items),
  and the comment was updated to match.
- No production code changed; no new behavior introduced.

No new confirmed findings surfaced.

**New confirmed findings:** 0

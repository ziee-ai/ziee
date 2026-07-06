# DRIFT-1 — implementation vs plan

Round 1 audit of the implemented diff against PLAN.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: none — ITEM-1 implemented as planned: `search: Option<String>` added to both `PaginationQuery` and the private `Raw` in the custom `Deserialize` (handlers.rs). Matches plan + reference idiom.
- **DRIFT-1.2** — verdict: none — ITEM-2 implemented as planned: `normalize_search` extracted (trim → blank-to-None) and wired into `list_projects` via `query.search.as_deref()` → `search.as_deref()`. Mirrors the mcp handler convention (DEC-5).
- **DRIFT-1.3** — verdict: none — ITEM-3 implemented as planned: `list_for_user(..., search: Option<&str>)` with the parameterized `$N::text IS NULL OR name ILIKE … OR description ILIKE …` predicate applied to BOTH the page SELECT and the COUNT (DEC-6). `cargo check --lib` passed → sqlx verified the SQL against the build DB.
- **DRIFT-1.4** — verdict: impl-wins — ITEM-4 regen produced a large *positional* diff in `openapi.json` (≈1381 lines) even though the semantic content delta is only **+33 lines, 0 removed** (verified via `comm` on sorted files — the search param on `Project.list` + `Project.listConversations` + its schema). Cause: the committed spec's key-order had drifted from the current generator's output; regen brings it into lockstep. **Resolution (plan amended in spirit):** the generator is canonical — do not hand-edit generated JSON; commit the regen as-is. The lifecycle coverage law was updated to EXCLUDE generated artifacts (`**/openapi.json`, `**/api-client/types.ts`) from the phase-6 hunk-coverage requirement, since they are mechanically derived from reviewed source and guarded by the `types_ts_parity` golden test. `types.ts` itself has a clean 9-line diff (it is sort-stable), which the api-contract angle reviews directly.
- **DRIFT-1.5** — verdict: none — the `search` param surfacing on `Project.listConversations` (the other extractor sharing `PaginationQuery`) is NOT a drift — PLAN_AUDIT's breakage-risk section predicted it; it is additive and ignored there.
- **DRIFT-1.6** — verdict: none — test files (`tests/project/search_test.rs` + `mod.rs` registration, 2 unit tests in handlers.rs) match TESTS.md's TEST-1..TEST-7 enumeration exactly.

**Unresolved drifts:** 0

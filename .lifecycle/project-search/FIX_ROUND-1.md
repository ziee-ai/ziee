# FIX_ROUND-1 — project-search

Merged the blind-audit LEDGER (round 1, 3 fresh reviewers across 12 angles),
applied fixes for every confirmed finding, then re-ran a full blind round
(round 2, 2 fresh reviewers across all 12 angles).

## Round-1 confirmed findings → disposition

- **[api-contract] `search` advertised on `Project.listConversations`** (LEDGER
  round-1 #10) → **FIXED (FIX-A).** Introduced a dedicated `ProjectListQuery`
  for `GET /projects`; reverted `PaginationQuery` to page/limit only, so the
  conversations endpoint no longer advertises `search`. Verified in the
  regenerated `openapi.json` (search now only on `Project.list`,
  `McpServer.listAccessible`, `McpServerSystem.list`, `Memory.list`) and
  `types.ts`. Mirrors the per-endpoint query-struct convention
  (`mcp/handlers/user.rs`), so the fix is MORE conformant than the original.

- **[tests-quality] no filtered-pagination consistency test** (LEDGER round-1
  #16) → **FIXED (FIX-B).** Added **TEST-8** `filtered_total_survives_page_truncation`:
  3 matches + 1 decoy, `?search=report&limit=2` ⇒ `total==3` (COUNT ignores
  LIMIT) while page length `==2`.

- **[tests-quality] no multi-match / metacharacter test** (LEDGER round-1 #17)
  → **FIXED (FIX-C).** Added **TEST-9** `multi_match_and_wildcard_metacharacters`:
  a term matching two projects returns both (sorted compare); a bare `%` term
  behaves as an unescaped ILIKE wildcard, documenting DEC-7.

## Round-1 confirmed findings → explicitly DISMISSED (not defects)

- **[security] LIKE metacharacters not escaped** (LEDGER round-1 #3) — accepted
  per **DEC-7**; matches the mcp convention exactly; harmless (scoped to the
  user's own rows, fully parameterized so no injection). A dismissed finding is
  a conscious rejection, not a silent pass.
- **[concurrency] non-transactional SELECT+COUNT** (LEDGER round-1 #8) —
  pre-existing pattern, unchanged by this diff; not a regression.

## Round-2 re-audit (post-fix, blind)

Two fresh reviewers re-examined the full post-fix diff across all 12 angles,
told only that the LIKE-wildcard + non-transactional-COUNT items are already
accepted. They specifically verified: `ProjectListQuery::resolved()` clamps
byte-identically to the original; the new `Deserialize` is correct; removing
`search` from `PaginationQuery` did **not** break its second caller
(`chat_extension` conversations list — checked and cleared, high-severity
hypothesis); and TEST-8/TEST-9 are non-vacuous and deterministic (real decoys,
no ORDER BY tie-break reliance).

Result: **all findings dismissed; no new defect introduced by the fixes.**

**New confirmed findings:** 0

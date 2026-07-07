# FIX_ROUND-6 — re-audit round 6

Two fresh blind reviewers (draft logic now traced sound across new-chat / edit /
regenerate / cancel / A→B switch) → 2 new confirmed, fixed:

- FIX-21 (patterns/correctness, medium): the content-search EXISTS subquery
  joined branches by conversation only, so a conversation matched when the term
  appeared only in a superseded/inactive edit branch — invisible on open and
  inconsistent with the active-branch-only client find bar. Restricted both the
  list + count subqueries to `bm2.branch_id = c.active_branch_id`.
- FIX-22 (state-management, medium): the `sync:conversation` delete handler
  decremented `total` unconditionally, so a cross-device delete of a NON-matching
  conversation desynced the server-side FILTERED total ("Showing 3 of 2" + wrong
  hasMore). Now decrements only when the deleted id was actually in the list.

Compiles clean (server `cargo check --tests`; ui tsc).


**New confirmed findings:** 2

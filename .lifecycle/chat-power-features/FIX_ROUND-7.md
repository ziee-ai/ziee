# FIX_ROUND-7 — re-audit round 7

Two fresh blind reviewers. Reviewer A (backend/logic) confirmed NONE — the
active-branch search subquery (list == count), the sync-delete `wasPresent`
guard, `reloadQueued` coalescing, and the find/collapse/draft/paste paths were
all traced sound. Reviewer B (full sweep) found 1 new confirmed, fixed:

- FIX-23 (state-management, medium): `deleteConversation` decremented `total`
  unconditionally, but it's invoked from surfaces (recent-conversations widget,
  project conversation lists) where the deleted conversation may not be in this
  store's current search-filtered list — desyncing the filtered "Showing X of N"
  + hasMore. Added the same `wasPresent` guard as the sync-delete path.

Compiles clean (ui tsc).

**New confirmed findings:** 1

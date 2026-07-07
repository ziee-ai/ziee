# FIX_ROUND-8 — re-audit round 8

Two fresh blind reviewers. Reviewer B (independent full sweep) confirmed NONE
across all angles. Reviewer A found 1 new confirmed, fixed:

- FIX-24 (state-management, medium): `bulkDelete` decremented `total` by
  `selectedIds.length` rather than the count actually removed, and the
  `sync:conversation` delete handler didn't prune `selectedIds` — so a selected
  row removed by a concurrent cross-device delete (already decremented) would be
  double-counted by a later bulkDelete. Fixed: sync-delete now prunes
  `selectedIds`, and bulkDelete decrements by the actually-removed count.

This closes the `total`-count invariant across every path (created / delete /
bulkDelete / sync-delete / load). Compiles clean (ui tsc).

**New confirmed findings:** 1

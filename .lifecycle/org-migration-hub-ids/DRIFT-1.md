# DRIFT-1 — implementation vs plan

Reviewed the worktree `git status` + `git grep` against PLAN.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: none — ITEM-1/2/3: the 12 seed manifests are renamed via `git mv` (git detects them as renames `R`), index.json + manifests carry `io.github.ziee-ai` (45 occurrences: 24 index + 21 manifest), and a fresh `build-pages.py` run in the hub clone produces a **byte-identical** `code-reviewer/1.0.0.json`, confirming the mirror matches canonical build output.
- **DRIFT-1.2** — verdict: none — ITEM-4: migration `00000000000131` present, prefix-scoped idempotent `UPDATE hub_entities`, mirrors migration 92; touches only `hub_entities.hub_id`.
- **DRIFT-1.3** — verdict: none — ITEM-5/6/8: `hub_manager.rs`, `catalog_v1.rs`, `mod.rs`, `download_test.rs`, `workflow_mcp/tools.rs` fully rebranded (dotted form + the underscore slug form in workflow_mcp); zero residual `io.github.phibya`/`io_github_phibya` in these files.
- **DRIFT-1.4** — verdict: resolved — ITEM-7: `migration_test.rs` was NOT blind-seded (that would have made the migration-92-isolation tests assert ziee-ai while executing only migration 92 → false failure). Instead the existing 92 tests are left asserting `io.github.phibya/*` (still migration 92's true output), and two new tests were added: a direct migration-131 rewrite+idempotency test and a 92→131 composition test. The remaining `io.github.phibya` occurrences in this file are all intentional (92's isolated output + the chain's intermediate state).
- **DRIFT-1.5** — verdict: none — ITEM-9: hub-repo YAMLs rebranded (`contributor: phibya` preserved), `validate.py` green, `build-pages.py` emits the ziee-ai layout; PR opened (ziee-ai/hub#6).
- **DRIFT-1.6** — verdict: none — DEC-8 honored: gallery `crawl.json` (ui + desktop) intentionally still carry `io.github.phibya` (deferred, out of scope); no asserting test depends on them.
- **DRIFT-1.7** — verdict: none — DEC-3 honored: `hub_version` remains `2.0.0` in the seed index (unchanged), preserving the `seed_index_version_matches_const` lockstep with the `SEED_HUB_VERSION` const (untouched).

**Unresolved drifts:** 0

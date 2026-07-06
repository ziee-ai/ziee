# PLAN_AUDIT — org-migration-hub-ids

Audit of PLAN.md against the codebase (read before implementing).

## Breakage risk

- The publisher string `io.github.phibya` is **unique to catalog publisher IDs** —
  a repo-wide grep finds it only in: the seed JSON, the hub `#[cfg(test)]` +
  test files, `workflow_mcp/tools.rs` (a fictional slug-test fixture), and the
  gallery `crawl.json` fixtures. No production runtime code branches on the
  literal string, so a prefix rename cannot break a caller.
- `is_safe_name` (`hub_manager.rs:968`) permits `-` in the namespace bytes
  (`b == b'-'`), so `io.github.ziee-ai/*` is a **valid** safe name (verified) —
  the rename does not trip name validation, catalog fetch, or the MCP server-slug
  regex (which is derived from the *leaf*, unchanged).
- `binaries/hub-seed/` is gitignored (`git check-ignore` confirms) and
  regenerated from `resources/hub-seed/` on every build by
  `build_helper/hub_seed.rs` — editing only `resources/` is correct and
  sufficient; the baked copy follows automatically.

## Pattern conformance

- **ITEM-4** mirrors migration 92 one-for-one (guarded idempotent `UPDATE
  hub_entities` on `hub_id`). Confirmed `hub_entities.hub_id` is the ONLY column
  holding the raw catalog id: migrations 8/69/79/80/92 are the only ones touching
  it, and `repository.rs` / `handlers.rs` insert the reverse-DNS `name` straight
  into `hub_entities.hub_id`. Installed assistants/models/mcp_servers reference
  the catalog only indirectly via `hub_entities.entity_id` (a UUID), so no other
  table needs rewriting.
- **ITEM-1/2/3** conform to the seed layout enforced by
  `seed_index_version_matches_const` and the `HUB_SEED.get_file` tests.
- **ITEM-9** conforms to the hub repo's flat-YAML source shape; the derived Pages
  namespace comes from `name` via `split_name`, and `contributor` is independent
  (the 6 mcp-servers already carry `contributor: phibya` with non-`phibya` names,
  proving decoupling).

## Migration collisions

- `ls migrations/ | sort | tail -1` = `00000000000130_*`. Next free number is
  **131**. No collision. The migration is pure DML (no schema change), so it
  needs no `sqlx::query!` verification of its own and no `openapi` impact.

## OpenAPI regen

- **None.** No Rust request/response type, no schemas file, and no OpenAPI-annotated
  handler changes. The rename touches only seed data (JSON), a SQL migration,
  and test/`#[cfg(test)]` string literals. `types.ts`/`openapi.json` are
  untouched → no `just openapi-regen` in either workspace.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure `git mv`; baked mirror regenerates from `resources/`; renamed names pass `is_safe_name`.
- **ITEM-2** — verdict: PASS — 12 name + 12 manifest_path swaps in index.json; `hub_version` deliberately unchanged (kept at 2.0.0 → lockstep test stays green).
- **ITEM-3** — verdict: PASS — 21 in-manifest occurrences (own `name` + `dependencies[].name`); dependency targets are also renamed in lockstep so cross-refs still resolve.
- **ITEM-4** — verdict: PASS — mirrors migration 92; only `hub_entities`; idempotent prefix guard; no schema/sqlx/openapi impact; number 131 free.
- **ITEM-5** — verdict: PASS — the 6 `#[cfg(test)]` refs are seed-path/name assertions that MUST track the rename; string-swap only.
- **ITEM-6** — verdict: PASS — 12 assertion-literal swaps across 3 test files; the referenced entities are exactly the renamed ones.
- **ITEM-7** — verdict: CONCERN — `migration_test.rs` currently asserts the post-92 value `io.github.phibya/code-reviewer`; because the harness runs ALL migrations, migration 131 will further rewrite it to `io.github.ziee-ai/*`. The existing assertions MUST be updated to the final state, and a dedicated migration-131 assertion added, or the suite goes red. Resolved by ITEM-7's scope (both changes are in-plan).
- **ITEM-8** — verdict: PASS — self-contained slug-transform unit test; input + expected-slug swap only; `slug_for_name` preserves hyphens so `ziee-ai` slugs cleanly.
- **ITEM-10** — verdict: PASS — the 403 + count failures reproduce byte-identically on the rebrand-free base (origin/main content, 10 passed / 5 failed), so they are pre-existing, not rebrand-induced; the grant fix matches the handler's `HubCatalogRead` contract (confirmed by the sibling `catalog_read_cannot_refresh` test) and the count bump matches the actual seed (29 items / 10 workflows). Scope-limited to the 5 catalog-read tests + 2 constants; `refresh`/`installed` grants untouched.
- **ITEM-9** — verdict: PASS — 12 source-YAML `name`/dep swaps in a separate repo; verified by the hub repo's own `validate.py` + `test-pages-build.sh`; `contributor`/README/CONTRIBUTING intentionally untouched.

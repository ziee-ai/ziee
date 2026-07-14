# MIGRATE-squash — CUT / reshape

## Design-gate

This chunk is a **migration-chain RECONSTRUCTION**, not a symbol move. Per
decision **M2** (DECISION_LEDGER convergence pass) it is **EXEMPT from the
move-shaped C-2 checks E5/E6/E7** — there are no `move:` source→dest lines and
no `## Symbols` deletions, because no code symbol relocates. The reshape squashes
the composed 147-migration numeric history (`src-app/server/migrations/` ∪
`sdk/crates/ziee-auth/migrations/`, 137+10) into clean, **module-owned** squashed
baselines named `<YYYYMMDDNNNN>_<module>_<desc>.sql`, and is gated instead by the
EA equivalence anchors (EA-schema fingerprint + EA-seed whole-DB image) plus the
N9 auth-domain-purity grep. Git *code* history is preserved; the *migration
chain* history is deliberately squashed (N8 — no deployed DBs to protect; the
append-only/checksum guard is re-armed from this baseline forward).

## Files (reshape, not move)

- **Deleted:** 137 numeric `src-app/server/migrations/*.sql` + 10 numeric
  `sdk/crates/ziee-auth/migrations/*.sql`.
- **Created:** 91 squashed baselines — 3 in `sdk/crates/ziee-auth/migrations/`
  (auth schema + fkeys + N9-clean seed) and 88 across 30
  `src-app/server/src/modules/<mod>/migrations/` dirs (per-module schema, deferred
  fkeys, seed data, domain permission grants) + one framework bootstrap
  (`app/migrations/202607140001_app_bootstrap.sql` — extensions + shared trigger
  fn, sorts FIRST).
- **Modified (code):** `src-app/server/build.rs` (`compose_merged_migrations()`
  widened to glob `src/modules/*/migrations/ ∪ ../../sdk/crates/*/migrations/`,
  + basename-collision guard); `src-app/server/src/core/database/mod.rs` (doc
  comment); new `src-app/server/src/modules/MIGRATIONS.md` authoring convention.
- **Tooling (committed):** `.extraction/tools/{schema_fp.sql,seed_compare.py,
  apply_migrations.sh}`; baselines `.extraction/baseline/{schema.fp,
  seed.canonical.txt,seed.sql}`.

## Symbols

_None._ No code symbol is moved or renamed by this chunk (reconstruction only).

## Ownership map (H4)

Auth identity tables → `ziee-auth`. Domain tables → their feature module
(`file_chunks`/`file_index_state`→file_rag; join tables→parent module;
`CREATE EXTENSION vector`→framework bootstrap, sorts first; domain permission
grants→feature module, NOT auth per N9). Full table→module map in
`.extraction/MIGRATE-squash/TRANSFORMS.md`.

# Migration authoring convention (module-owned)

Migrations in ziee are **module-owned** (decision N7): every module keeps its
schema next to its routes/permissions/repository, in `<module>/migrations/`.
The framework composes the UNION of all module migration dirs at build time.

## Where a migration lives

- **A ziee server module** → `src-app/server/src/modules/<module>/migrations/`.
- **An SDK crate** (e.g. identity/auth) → `sdk/crates/<crate>/migrations/`.
  `ziee-auth` owns the identity tables (`users`, `groups`, `sessions`,
  `auth_providers`, `refresh_tokens`, `user_auth_links`, `session_settings`, …).

`build.rs::compose_merged_migrations()` globs
`src/modules/*/migrations/ ∪ ../../sdk/crates/*/migrations/`, copies every `.sql`
into the generated `migrations-merged/` (basename-collision-guarded), and both
the build-DB provisioner and the runtime `sqlx::migrate!("./migrations-merged")`
apply that version-sorted set.

## Naming — `<YYYYMMDDNNNN>_<module>_<desc>.sql`

- `YYYYMMDD` date prefix + `NNNN` a **monotonic counter** (NOT wall-clock
  seconds — 100+ files authored in one session would collide). The counter is
  assigned to preserve **FK-topological order**: a referenced table's migration
  must sort before its referrer.
- Rough version bands used by the MIGRATE-squash baseline:
  `0001` framework bootstrap (extensions + shared trigger fn, sorts FIRST) ·
  `0050` auth schema · `0100+` per-module table schema (no inline FKs) ·
  `4000+` deferred foreign-key ALTERs (every table exists by now) ·
  `4500/5000+` seed data · `6000+` domain permission grants.

## Ownership rules

1. **One owner per table.** Others reference it via FK. Join tables belong to
   their parent module.
2. **No domain data in the SDK/auth crate (N9).** `ziee-auth` migrations contain
   ZERO permission strings other than `profile::*` / `*`. Every domain permission
   grant (`chat::`, `files::`, `mcp_servers::`, …) lives in the owning module's
   own `*_grant_permissions.sql`. The auth seed creates the system groups with a
   CLEAN base (`Administrators=['*']`, `Users=['profile::read','profile::edit']`);
   each module appends its perms.
3. **FKs are deferred** to a post-schema band so cross-module table order is free
   and no module needs to know another module's version number.
4. **Append-only / immutable from the squash baseline forward (N8/N3.1):** never
   edit a shipped migration; add a new one. (The one squash suspended this; it is
   re-armed now.)

## Equivalence gate

Schema/seed changes are gated by `.extraction/tools/schema_fp.sql` (catalog
fingerprint) + `.extraction/tools/seed_compare.py` (business-key seed image)
against `.extraction/baseline/`. A fresh-migrated DB must be structurally
identical to the baseline and seed-equivalent.

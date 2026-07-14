# Chunk BA — TRANSFORMS (shim approach + migration-composition design + spike result)

## Symbol transforms (spike scope)

- **T-1** `User`: no source change — the struct + its `#[serde(skip_serializing)]
  #[schemars(skip)] password_hash`, `AuthUser` impl, and `ziee_identity::Principal`
  impl moved **byte-for-byte** into `ziee-auth`. **why:** equivalence-preserving
  move (N2); the impls are orphan-rule-bound to the crate that defines `User`
  (`AuthUser` is a foreign trait; `Principal` is a `ziee-identity` trait), so they
  travel with the struct.
- **T-2** `Group`: byte-for-byte move. **why:** same, N2.
- **T-3** `ziee::modules::user::models`: reduced to `pub use ziee_auth::{Group,
  User};`. **why:** re-export shim keeps `crate::modules::user::{User,Group}`
  (and the `pub use models::*` fan-out) resolving unchanged, and — the load-bearing
  point — keeps the schemars **short-name** identity `User`/`Group`, so the wire
  schema is byte-stable.

## Decision — the shim approach (N2)
**Resolution:** move the *definition* into `ziee-auth`; keep a thin ziee re-export
so neither the ~call sites nor the schemars type-idents move. Verified by regen:
`types.ts` byte-identical + `openapi.json` canonically-equal on ui **and** desktop.
No `GOLDEN_DELTA` needed (the delta is empty) — the strongest N2 outcome.

## Decision — migration composition (N3), DESIGNED, NOT YET APPLIED
**Resolution (design, for the future un-blocked BA):** relocate the auth-table
migration files (`users`/`groups`/`permissions`/`refresh_tokens`/`sessions`/
`session_settings`) into `sdk/crates/ziee-auth/migrations/` **preserving exact
version numbers + byte content** (checksums immutable). `ziee-auth` exports
`pub static AUTH_MIGRATOR: Migrator = sqlx::migrate!("migrations");`. Each app
composes ONE merged migration directory **at build time** (a `build.rs` step
copies `ziee-auth`'s embedded migrations ∪ the app's `migrations/` into
`$OUT_DIR/merged-migrations`, sorted by version) and points BOTH the runtime
`sqlx::migrate!` (today `core/database/mod.rs:318`) and the build-DB provisioner
(today `build.rs:165`) at that merged dir. `ziee-auth`'s own `build.rs`
provisions an **auth-only** build DB (just `AUTH_MIGRATOR`) for its `query!`
verification. Because the moved files keep version+bytes, the version-sorted
merged set reproduces ziee's exact `_sqlx_migrations` history → existing
deployments unaffected; schema-identity provable via `pg_dump --schema-only` ==
`.extraction/baseline/schema.sql`.

**This is NOT applied** because it is only coherent once `ziee-auth` owns the auth
`query!` macros — which is blocked (STOP_REPORT.md). Relocating migrations while
the code stays in ziee inverts the dependency and adds deployment risk for no
gain.

## Spike result
GOLDEN-CLEAN. See SPIKE.md. Zero forbidden markers.

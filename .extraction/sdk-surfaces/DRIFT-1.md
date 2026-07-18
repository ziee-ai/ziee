# Chunk `sdk-surfaces` — DRIFT scan (round 1)

Drift = any place the two moves could diverge from pre-extraction
behavior / surface / output / schema. Each candidate reconciled.

- **DRIFT-1.1** — verdict: none. **Handler logic identity.** The moved
  `tokio::select!` loop, `recheck_interval()` + the `SYNC_RECHECK_TICK_MS` seam,
  the `ConnGuard` drop, the deadline arithmetic, and the connected handshake are
  1:1 with the former `subscribe_sync`; the only substitutions are `R`/`S` for the
  four ziee concretes. ziee's `SyncSurface::recheck` reproduces the three former
  match arms exactly (Refresh/TearDown/Transient).

- **DRIFT-1.2** — verdict: none. **`exp` extraction identity.** ziee's
  `ZieeIdentityResolver::access_token_exp` is byte-identical to the former inline
  block (`Arc<JwtService>` → `extract_token_from_header` → `validate_access_token`
  → `.exp`); default-`None` fallback + the 24h stream fallback are unchanged.

- **DRIFT-1.3** — verdict: none. **Security gate.** Subscribe gate
  (`profile::read`), the 60s re-check (is_active + baseline perm → teardown), the
  exp deadline, and notify-only wire are all preserved (LEDGER sdk-surfaces-03).

- **DRIFT-1.4** — verdict: none. **OpenAPI (Sync.subscribe, BOTH surfaces).** The
  operation metadata (id/tag/summary/description), the `with_permission` extension
  (`profile::read`), and the 200 schema (`Json<S::Wire>` = ziee `SyncSseEvent`,
  keyed by short ident) are unchanged. Regenerated ui+desktop: `types.ts`
  BYTE-IDENTICAL, `openapi.json` canonically-equal; reverted via git checkout.

- **DRIFT-1.5** — verdict: none. **Framework test mock intact.** `SyncSurface` is a
  NEW trait (own file), not a widening of `SyncEntityKind`, so the registry's
  minimal `TestEntity` (`session_signal` only) + its unit tests are untouched.

- **DRIFT-1.6** — verdict: none. **onboarding byte-identity.** `models.rs` moved
  byte-for-byte; `repository.rs` differs by ONLY the `AppError` import (same
  re-exported type); the 3 methods (incl. the atomic cardinality-cap upserts) are
  unchanged. Handlers + the `is_valid_onboarding_id` units are byte-unchanged.

- **DRIFT-1.7** — verdict: none. **Migration integrity + standalone.** The table
  migration moved byte-for-byte (checksum/version preserved, NO FK); the `users`
  FK migration stays ziee-side, sorted after both tables. The merged set is
  version-identical → `schema.fp` UNCHANGED. The crate migration applies alone
  (0 domain FKs); its own build.rs proves the `query!` verify standalone.

- **DRIFT-1.8** — verdict: none. **Shim transparency / build hygiene.**
  `onboarding/mod.rs` + `sync/handlers.rs` keep every call site + `Repos.onboarding`
  resolving; the dropped `ClientConn`/`SYNC_CHANNEL_CAPACITY` re-exports had no
  external consumer. `cargo check` on ziee / ziee-desktop / sdk workspace all
  exit 0; no NEW warnings (the 2 ziee warnings are pre-existing).

**Unresolved drifts: 0**

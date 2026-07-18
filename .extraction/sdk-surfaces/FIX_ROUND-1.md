# Chunk `sdk-surfaces` — FIX round 1

Blind multi-angle audit (`LEDGER.jsonl` — 12 findings across 9 angles incl.
`equivalence`, `security` (the SSE auth-gate), `boundary`, `canonical-equality`,
`migration-integrity`, `standalone-apply`, `cross-crate-resolution`,
`desktop-parity`, `test-fidelity`, `shim-transparency`) reconciled against every
diff hunk (`AUDIT_COVERAGE.tsv` — 15 hunks, ≥2 angles each; the load-bearing
handler + resolver hunks ≥4).

The scope boundary was decided UP FRONT (not a deferred fix):

- **sync_routes** — the schema-bearing wire types (`SyncSseEvent` &c., in the
  OpenAPI/`types.ts` surface) + `publish` + the `registry()` singleton STAY
  concrete in ziee. Only the app-agnostic handler moves, generic over
  `R: IdentityResolver` + a NEW `SyncSurface` trait (deliberately separate from
  `SyncEntityKind` so the framework's registry test mock is untouched). The `exp`
  seam is a minimal default-`None` addition to `IdentityResolver` (the resolver
  owns token verification), NOT a new type param — keeping `sync_routes::<R, S>()`
  at two params.

- **ziee-onboarding** — mirrors `ziee-notification`'s handler/route/sync boundary,
  with the ONE justified divergence that the REPOSITORY + a build DB move too,
  because onboarding's `query!` are self-contained (`user_onboarding` only). The
  domain `user_id → users(id)` FK stays app-side so the crate migration applies on
  a bare DB (standalone-apply gate).

All findings are `verified`: the handler is a faithful 1:1 (loop/recheck/handshake/
exp), the security gate is preserved, models moved byte-for-byte + repository by a
one-line import, the migration split keeps the merged schema (schema.fp UNCHANGED)
while the crate applies standalone, the shims keep every call site + `Repos`
resolving, and golden is `types.ts` byte-identical + `openapi.json` canonically-equal
on BOTH ui + desktop. `cargo check` exit 0 on all three.

Re-audit of the diff surfaced no new issues.

**New confirmed findings: 0**

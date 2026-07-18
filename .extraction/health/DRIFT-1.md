# Chunk `health` — DRIFT scan (round 1)

Drift = any place moving the liveness surface could diverge from pre-extraction
behavior / surface / output. Each candidate reconciled.

- **DRIFT-1.1** — verdict: none. **Byte-identity of all three moved files.**
  `types.rs`/`handlers.rs`/`routes.rs` were 0-domain-dep; copied then
  `diff <(git show 4a2391732:…) sdk/…` is empty (exit 0) for each. No logic/text
  change; the 5 handler tests pass in the crate.

- **DRIFT-1.2** — verdict: none. **`routes()` still resolves.** `mod.rs`'s
  `pub use ziee_health::{handlers, routes, types};` carries the crate-root
  `routes()` fn (value namespace) so `register_routes`' call resolves; no second
  import line (would double-import → E0252). ziee + ziee-desktop compile exit 0.

- **DRIFT-1.3** — verdict: none. **OpenAPI output (E8, BOTH surfaces).** The
  schema key `HealthResponse` (schemars short ident) + operationId `Health.check`
  + the `health` tag are unchanged. Regenerated ui + desktop: `types.ts`
  BYTE-IDENTICAL, `openapi.json` canonically-equal (jq -S) vs baseline. Restored
  via `git checkout`.

- **DRIFT-1.4** — verdict: none. **Unauthenticated liveness preserved.**
  `health_check` still takes no extractor (no `RequirePermissions`), returns a
  static `{"status":"ok"}` 200. No DB, no auth reached the crate. The
  `health_check_returns_ok_200` + serde round-trip tests assert the exact wire
  shape.

- **DRIFT-1.5** — verdict: none. **Boundary / build-hygiene.** ziee-health names
  no app type + no DB; no build.rs, no build DB. `cargo check --workspace` exit 0.
  No new warnings.

**Unresolved drifts: 0**

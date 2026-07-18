# Chunk `server-update` — DRIFT scan (round 1)

Drift = any place moving the server-update wire type + permission key could
diverge from pre-extraction behavior / surface / output. Each candidate
reconciled.

- **DRIFT-1.1** — verdict: none. **types.rs byte-identity.** Copied then `diff
  <(git show 4a2391732:…/types.rs) sdk/…/types.rs` is empty (exit 0).
  `UpdateStatusResponse` (8 fields + `Default` + doc comments) unchanged.

- **DRIFT-1.2** — verdict: none. **checker version behavior preserved.** The
  `env!("CARGO_PKG_VERSION")` sites stay in ziee (checker.rs retained), so
  `/api/server-update/status` still reports ziee's version and the update-available
  semver compare is unchanged. Averted the regression that moving checker would
  cause (crate `0.0.0`).

- **DRIFT-1.3** — verdict: none. **permissions.rs semantic identity.** Only the two
  `PermissionCheck`/`PermissionList` imports changed (`crate::modules::permissions::
  types::…` → `ziee_identity::…`, the same traits ziee re-exports). The
  `server_update::read` string + name + description are unchanged; the moved test
  passes.

- **DRIFT-1.4** — verdict: none. **OpenAPI output (E8, BOTH surfaces).** The schema
  key `UpdateStatusResponse` + operationId `ServerUpdate.getStatus` + the 403
  example (built in the retained handler docs) are unchanged. Regenerated ui +
  desktop: `types.ts` BYTE-IDENTICAL, `openapi.json` canonically-equal (jq -S) vs
  baseline. Restored via `git checkout`.

- **DRIFT-1.5** — verdict: none. **Shim transparency.** `mod.rs`'s
  `pub use ziee_server_update::{permissions, types};` keeps `super::types` (checker +
  handlers) + `super::permissions` (handlers) resolving. ziee + ziee-desktop compile
  exit 0.

- **DRIFT-1.6** — verdict: none. **Boundary / build-hygiene.** ziee-server-update
  names no app type + no DB; no build.rs, no build DB. `cargo check --workspace`
  exit 0. No new warnings.

**Unresolved drifts: 0**

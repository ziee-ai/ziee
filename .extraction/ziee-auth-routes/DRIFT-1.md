# Chunk ziee-auth-routes — DRIFT-1 (convergence)

Drift-convergence loop over the implementation vs the CUT/TRANSFORMS plan.

## Rounds
- **round 1** — initial move (handlers/routes/jwt_extractor/session_settings/
  permissions → `ziee-auth/src/auth/http` + `auth/permissions.rs` + `user/permissions.rs`);
  import rewrites; genericity over `R: IdentityResolver<User=User, Group=Group>`;
  Cargo `routes` feature; app-side thin-consumer shims.
- gate sequence, all green in one pass:
  - `cargo check -p ziee-auth` (default features, build-DB) = 0
  - `cargo check --workspace` (sdk) = 0
  - `cargo check -p skeleton-server` (framework-only, NO DATABASE_URL) = 0
  - `cargo check -p ziee` = 0 (no new warnings vs baseline)
  - `cargo check -p ziee-desktop` = 0
  - golden both surfaces: types.ts BYTE-IDENTICAL ×2, openapi CANONICALLY-EQUAL ×2

## Equivalence spot-check
`diff HEAD:handlers.rs  sdk/…/http/handlers.rs` = 84 changed lines, 100% mechanical
(import paths + `pub(crate)`→`pub` + `super::`→`crate::auth::` + `<R>` generics +
`#[debug_handler]` removal on generic handlers). Zero logic/string/response-shape
changes.

Unresolved drifts: 0

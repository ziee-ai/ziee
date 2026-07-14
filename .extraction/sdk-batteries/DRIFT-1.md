# Chunk sdk-batteries — DRIFT-1 (convergence)

Drift-convergence loop over the implementation vs the CUT/TRANSFORMS plan.

## Rounds
- **round 1** — created `ziee-build-support` (worktree_db verbatim move + compose +
  build_db); framework re-export `build_support` + `serve`; embedded_pg P0-b fix;
  ziee-auth turnkey (DefaultIdentityResolver + mount_auth) + noop sinks; ziee-core
  config loader; CORS downgrade; wired ziee's server/desktop build.rs + harness to the
  build-dep; deleted `build_helper/worktree_db.rs`.
- one drift found + fixed in-loop:
  - **mount_auth smoke test** used `#[test]` but `PgPool::connect_lazy` needs a Tokio
    context → panicked "requires a Tokio context". Fixed → `#[tokio::test]`. Re-ran green.

## Gate sequence (all green)
- `cargo check -p ziee-build-support -p ziee-framework` = 0
- `cargo check -p ziee-auth` (default features, auth build-DB) = 0
- `cargo check -p ziee-core` = 0
- `cargo check --workspace` (sdk, incl. skeleton-server = E11) = 0
- `cargo check -p ziee` = 0 (build.rs ran: composed migrations-merged + provisioned build DB)
- `cargo check -p ziee-desktop` = 0
- unit tests: build-support 5/5, embedded_pg resolve_* 2/2, ziee-core load_from 5/5,
  ziee-auth turnkey 2/2 + noop 2/2 — all pass.

## Equivalence spot-check
- migrations-merged: `sha256(sorted-concat) == 93b6f632…` == the source-union sha; 91==91
  files, name sets identical → byte-identical composition.
- embedded_pg: ziee fills installation_dir/data_dir before boot → default branch never
  fires → identical.

Unresolved drifts: 0

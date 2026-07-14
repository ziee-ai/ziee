# Chunk `health` — TRANSFORMS

Every transform applied moving `modules/health` into `ziee-health`, each with its
decision + resolution. Zero TBD.

## T-1 — `types.rs` / `handlers.rs` / `routes.rs` moved BYTE-FOR-BYTE

### Decision — is any edit needed to the moved files?

`types.rs` (`HealthResponse`) names only `serde` + `schemars`. `handlers.rs`
(`health_check` + docs + 5 tests) names only `aide::transform::TransformOperation`,
`axum::{Json, debug_handler, http::StatusCode}`, `super::types::HealthResponse`,
and `serde_json` (in a test). `routes.rs` (`routes()`) names `aide::axum` +
`super::handlers::*`. None reference `crate::`, a DB, or an app type — the module
was already 0-domain-dep.

**Resolution:** copied via `cp`, no edits. Inside the crate, `super::types` (from
`handlers`) and `super::handlers` (from `routes`) resolve to the sibling modules
declared in `lib.rs`. `diff` vs the git-HEAD originals is empty for all three.

## T-2 — `health/mod.rs` re-export shim (registration stays, body re-exported)

### Decision — how `routes()` keeps resolving after the body moves

The `HealthModule::register_routes` calls `routes()` (formerly `pub use
routes::routes;` over the local `mod routes`). After the move those modules no
longer exist under `health/`.

**Resolution:** `mod.rs` does `pub use ziee_health::{handlers, routes, types};`.
Re-exporting `ziee_health::routes` brings the name `routes` from BOTH namespaces
it occupies in the crate — the module (`pub mod routes`) AND the crate-root
function (`pub use routes::routes` in the crate's `lib.rs`) — so `routes()`
resolves in the value namespace exactly as the old `pub use routes::routes;` did.
No second `pub use …::routes::routes;` line is added (that would double-import the
function → E0252). `handlers`/`types` are re-exported so any
`crate::modules::health::{handlers,types}::…` path stays valid (none exist outside
the module today, but parity is preserved).

## T-3 — `ziee-health` deps

### Decision — minimal dep set for a DB-free liveness crate

**Resolution:** `serde`/`serde_json`/`schemars` (the wire type + a serde test),
`axum` (`Json`/`debug_handler`/`StatusCode`), `aide` (the route builder +
`TransformOperation`) — versions matching the ziee server crate so the single
`src-app/Cargo.lock` unifies them (no duplicate build). `tokio` is a dev-dep only
(the 5 `#[tokio::test]`/`#[test]` handler units). No `sqlx`, no build.rs, no build
DB.

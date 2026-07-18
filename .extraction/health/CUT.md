# Chunk `health` — `ziee-health` DB-free liveness surface (CUT manifest)

Lift ziee's `modules/health` — a 0-domain-dep, DB-free liveness/readiness
endpoint — into a new `ziee-health` SDK crate (N7: module = liftable crate). The
whole module BODY moves; only the `AppModule`/`MODULE_ENTRIES` registration
(which names ziee's `module_api`) stays app-side.

## Design-gate — the liveness surface

`health` exposes one unauthenticated endpoint `GET /api/health` returning
`{"status":"ok"}` — a liveness probe for load balancers/orchestrators. It has NO
DB, NO auth, NO app-concrete type: `HealthResponse` is a `schemars`/`serde`
struct, `health_check` is a pure async handler, `routes()` mounts it via aide.
The only tie to ziee is the module-registration glue, which every module keeps
app-side because `MODULE_ENTRIES` is ziee's `linkme` distributed slice. So the
crate is a genuinely reusable, build-DB-free health module.

## Files move: INTO `ziee-health` (submodule `sdk/`, sha 35a6e7f1)

- new: `crates/ziee-health/src/types.rs` — `HealthResponse` (moved BYTE-FOR-BYTE).
- new: `crates/ziee-health/src/handlers.rs` — `health_check` + `health_check_docs`
  + the 5 `#[cfg(test)]` units (moved BYTE-FOR-BYTE).
- new: `crates/ziee-health/src/routes.rs` — `routes()` (moved BYTE-FOR-BYTE).
- new: `crates/ziee-health/src/lib.rs` — `pub mod types/handlers/routes;` +
  `pub use routes::routes;`.
- new: `crates/ziee-health/Cargo.toml` — serde/serde_json/schemars/axum/aide;
  tokio dev-dep (the handler tests). Build-DB-free.

## Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- del: `server/src/modules/health/{types,handlers,routes}.rs` (moved).
- edit: `server/src/modules/health/mod.rs` — replaces `mod handlers/routes/types;`
  + `pub use routes::routes;` with `pub use ziee_health::{handlers, routes, types};`
  (the `routes` re-export carries both the module and the crate-root `routes()` fn
  in the value namespace, so `AppModule::register_routes`' `routes()` call resolves).
- edit: `server/Cargo.toml` — add the `ziee-health` path dep.

## Symbols

- Moved: `HealthResponse`, `health_check`, `health_check_docs`, `routes`.
- Schemars key preserved: `HealthResponse` (short ident) → OpenAPI schema
  `HealthResponse` byte-identical.
- OperationId preserved: `Health.check` (set in `health_check_docs`, unchanged).

## Stays app-side

`health/mod.rs` — the `HealthModule` `AppModule` impl + the
`#[distributed_slice(MODULE_ENTRIES)]` registration (name "health", order 85).
Names `crate::module_api`, so it stays in ziee.

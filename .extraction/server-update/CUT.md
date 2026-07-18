# Chunk `server-update` — `ziee-server-update` wire type + perm key (CUT manifest)

Lift the DB-free, app-decoupled surface of ziee's `modules/server_update` — the
`UpdateStatusResponse` wire type + the `ServerUpdateRead` permission key — into a
new `ziee-server-update` SDK crate (N7). The daily-poll `checker` is DELIBERATELY
retained in ziee (behavioral-equivalence constraint, see Design-gate); the
aide/axum handlers/routes + the `AppModule` registration also stay app-side.

## Design-gate — why `checker` STAYS (the load-bearing decision)

`server_update` is a notification-only module: a daily background task polls
GitHub `releases/latest` for `ziee-ai/ziee`, semver-compares, and caches the
result served by admin `GET /api/server-update/status`. Two files are DB-free and
app-decoupled — `types.rs` (`UpdateStatusResponse`, serde/schemars) and
`permissions.rs` (`ServerUpdateRead`, `ziee_identity::PermissionCheck`) — so they
move. `checker.rs` is the substantive logic but is NOT equivalence-preservingly
liftable: it embeds `env!("CARGO_PKG_VERSION")` in TWO places (the cache seed's
`current_version` + the `is_newer` comparison), which the compiler resolves to the
CRATE it is compiled in — moving it would report `ziee-server-update`'s `0.0.0`
instead of ziee's version, a runtime behavior change observable at
`/api/server-update/status`. It also has a `#[cfg(test)]` naming
`crate::core::config::UpdateCheckConfig` (ziee's config type). So `checker` stays
in ziee. `handlers.rs`/`routes.rs` name `RequirePermissions`/`with_permission`
(ziee's resolver alias), so they stay too.

## Files move: INTO `ziee-server-update` (submodule `sdk/`, sha 35a6e7f1)

- new: `crates/ziee-server-update/src/types.rs` — `UpdateStatusResponse` (moved
  BYTE-FOR-BYTE).
- new: `crates/ziee-server-update/src/permissions.rs` — `ServerUpdateRead` + its 1
  `#[cfg(test)]`. Edits: the two `crate::modules::permissions::types::{…}` imports
  → `ziee_identity::{…}` (TRANSFORMS T-1); rest byte-for-byte.
- new: `crates/ziee-server-update/src/lib.rs` — `pub mod permissions; pub mod
  types;`.
- new: `crates/ziee-server-update/Cargo.toml` — ziee-identity + serde/schemars.
  Build-DB-free.

## Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- del: `server/src/modules/server_update/{types,permissions}.rs`.
- edit: `server/src/modules/server_update/mod.rs` — keeps `mod checker/handlers/
  routes;` + registration; replaces `pub mod permissions; mod types;` with
  `pub use ziee_server_update::{permissions, types};` so `super::types::
  UpdateStatusResponse` (in `checker.rs` + `handlers.rs`) + `super::permissions::
  ServerUpdateRead` (in `handlers.rs`) resolve.
- edit: `server/Cargo.toml` — add the `ziee-server-update` path dep.

## Symbols

- Moved: `UpdateStatusResponse`, `ServerUpdateRead`.
- Schemars key preserved: `UpdateStatusResponse` → OpenAPI schema byte-identical.
- Permission string preserved: `server_update::read` → the 403 example (built in
  the retained `handlers.rs` docs via `with_permission::<(ServerUpdateRead,)>`)
  that feeds the UI `Permissions` enum is byte-identical.

## Stays app-side

`server_update/checker.rs` (env!CARGO_PKG_VERSION + the config test),
`handlers.rs`/`routes.rs` (RequirePermissions/with_permission), `mod.rs`
registration (name "server_update", order 92, + the daily-poll spawn +
`checker::set_enabled`).

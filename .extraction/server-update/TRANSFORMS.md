# Chunk `server-update` — TRANSFORMS

Every transform applied moving the server-update wire type + permission key into
`ziee-server-update`, each with its decision + resolution. Zero TBD.

## T-1 — `permissions.rs`: `PermissionCheck`/`PermissionList` imports re-pointed

### Decision — the permission traits live in `ziee-identity`

`ServerUpdateRead` implements `PermissionCheck`; its `#[cfg(test)]` also names
`PermissionList`. Both are re-exported by ziee from `ziee-identity`
(`crate::modules::permissions::types::{…}` is a B1b shim). The `impl` + the const
values (`server_update::read`, name, description) + the test's assertions are
identical.

**Resolution:** changed the production `use crate::modules::permissions::types::
PermissionCheck;` → `use ziee_identity::PermissionCheck;` and the test's `use
crate::modules::permissions::types::{PermissionCheck, PermissionList};` →
`use ziee_identity::{PermissionCheck, PermissionList};`. The permission string is
unchanged, so the OpenAPI 403 example (built in the retained `handlers.rs` docs)
that scrapes the UI `Permissions` enum is byte-identical. Only the two `use` lines
changed; the test moved with the file and passes.

## T-2 — `types.rs` moved BYTE-FOR-BYTE

`UpdateStatusResponse` names only `serde` + `schemars`. No `crate::` reference.

**Resolution:** copied via `cp`; `diff` vs the git-HEAD original is empty (exit 0).
The `#[derive(… Serialize, JsonSchema, Default)]` + all 8 fields (incl. the doc
comments that flow through as JSDoc) are unchanged → schema byte-identical.

## Decision — `checker.rs` is retained (NOT moved): the equivalence constraint

`checker.rs` is the module's substantive logic (GitHub poll + cache + semver), but
moving it would BREAK behavioral equivalence:

- It calls `env!("CARGO_PKG_VERSION")` at the cache seed (`current_version`) and in
  `check_once`'s `is_newer(&latest, env!("CARGO_PKG_VERSION"))`. `env!` is resolved
  at compile time to the crate being compiled. In ziee it yields ziee's version;
  moved to `ziee-server-update` it would yield that crate's `0.0.0`, changing the
  value the `/api/server-update/status` endpoint returns AND the update-available
  computation — an observable runtime regression.
- Its `#[cfg(test)] config_default_enabled_true` names
  `crate::core::config::UpdateCheckConfig` (ziee's config type).

Passing ziee's version into the crate (constructor/const-generic) would be a
REWRITE, not an equivalence-preserving move. So `checker.rs` STAYS in ziee. It
reaches the moved wire type via `super::types::UpdateStatusResponse` (resolved by
the `mod.rs` re-export). This is the honest DB-free-decoupled boundary: only the
two files that carry NO app/version coupling move.

## T-3 — `server_update/mod.rs` re-export shim (checker/handlers/routes stay)

### Decision — how `super::types`/`super::permissions` keep resolving

`checker.rs` uses `super::types::UpdateStatusResponse`; `handlers.rs` uses
`super::types::UpdateStatusResponse` + `super::permissions::ServerUpdateRead` +
`super::checker`.

**Resolution:** `mod.rs` keeps `mod checker; mod handlers; mod routes;` and
replaces `pub mod permissions; mod types;` with `pub use ziee_server_update::
{permissions, types};`. The `pub use` of the two modules aliases them so every
`super::types`/`super::permissions` path resolves. Zero call-site edits outside
`mod.rs` + `Cargo.toml`.

## T-4 — `ziee-server-update` deps

**Resolution:** `ziee-identity` (`PermissionCheck`/`PermissionList`) + `serde` +
`schemars` (the wire type). Versions match the ziee server catalog so the single
`src-app/Cargo.lock` unifies them. No `sqlx`, no build.rs, no build DB.

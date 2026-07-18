# Chunk `notification` — TRANSFORMS

Every transform applied moving the notification types + permission key +
migrations into `ziee-notification`, each with its decision + resolution. Zero TBD.

## T-1 — `permissions.rs`: `PermissionCheck` import re-pointed

### Decision — the permission trait lives in `ziee-identity`

`NotificationsRead` implements `PermissionCheck`, which ziee re-exports from
`ziee-identity`. The `impl` + the const values (`notifications::read`, name,
description) are identical.

**Resolution:** changed `use crate::modules::permissions::types::PermissionCheck;`
→ `use ziee_identity::PermissionCheck;`. The permission string is unchanged; the
grant (`notifications::read` → Users group) lives in the `scheduler` module's
`scheduler_grant_permissions.sql` migration, which is UNTOUCHED. Only the one
`use` line changed.

## T-2 — `models.rs` moved BYTE-FOR-BYTE

`Notification`/`NewNotification`/`NotificationPage`/`UnreadCount` name only
`chrono`/`schemars`/`serde`/`uuid`/`sqlx::FromRow`. `Notification` derives
`sqlx::FromRow` but the file issues NO `query!` (the compile-time-verified queries
are all in `repository.rs`, which stays app-side). No `crate::` reference.

**Resolution:** copied via `cp`; `diff` vs the git-HEAD original is empty (exit 0).
The schemars keys (`Notification`/`NotificationPage`/`UnreadCount`) are unchanged →
schemas byte-identical. `FromRow` derive needs no DB.

## T-3 — migrations moved to the crate's `migrations/` (N7, checksums preserved)

### Decision — carry the module's schema with the crate; the app globs it

The module owns `202607140190_notification_schema.sql` +
`202607144180_notification_fkeys.sql`. The app's `build.rs::
compose_merged_migrations()` globs `src/modules/*/migrations/ ∪
../../sdk/crates/*/migrations/` into a version-sorted merged set (basename-
collision-guarded) for both the build-DB provisioner and the runtime `migrate`.

**Resolution:** moved both `.sql` verbatim to
`crates/ziee-notification/migrations/` — ORIGINAL filenames + byte content, so
their sqlx checksums + version numbers are preserved and the merged set reproduces
ziee's exact `_sqlx_migrations` history. The app-side `migrations/` dir is deleted,
so each file is found EXACTLY ONCE (via the `sdk/crates/*` glob). The fkeys still
reference other modules' tables — that composes correctly in the FULL merged set
(the deferred-FK band `4180` sorts after every referenced table's schema), exactly
as before. Proven by the golden regen: ziee's build.rs provisioned the merged build
DB (incl. these two files from the crate) and the 10 `query_as!` in the retained
`repository.rs` verified against it (`cargo check -p ziee` exit 0).

## Decision — `repository.rs`/`events.rs` are retained (NOT moved)

The two schema-/sync-bound files stay in ziee (this is why the crate is thin +
build-DB-free):

- `repository.rs` uses `sqlx::query_as!` (×10, compile-time-verified). The
  notification fkeys reference `users`/`conversations`/`scheduled_tasks`/
  `workflow_runs` (other modules), so a STANDALONE notification build DB (auth-style,
  crate-owned) would fail those FKs at migration time — unlike `ziee-auth`, which is
  self-contained. So the queries verify only against the app's FULL merged build DB;
  `repository.rs` stays in ziee, and `ziee-notification` needs NO build.rs / build
  DB. It reaches the moved types via `super::models` (the shim).
- `events.rs` (`create_and_emit`) names the CONCRETE `crate::modules::sync::
  {Audience, SyncAction, SyncEntity, publish}` — the app-owned sync enums with the
  `SyncEntity::Notification` variant. Moving it would require the ziee-auth-style
  seam (a rewrite), so it stays.

## T-4 — `notification/mod.rs` re-export shim

### Decision — how `super::models`/`super::permissions` keep resolving

`repository.rs`/`events.rs`/`handlers.rs` use `super::models::{…}`; `handlers.rs`
uses `super::permissions::NotificationsRead`; `scheduler/dispatch.rs` names
`crate::modules::notification::models::NewNotification`.

**Resolution:** `mod.rs` keeps `pub mod events/handlers/prune/repository/routes;` +
registration and replaces `pub mod models; pub mod permissions;` with `pub use
ziee_notification::{models, permissions};`. The `pub use` aliases the two modules so
every `super::models`/`super::permissions` path + the cross-module
`notification::models::NewNotification` resolve. Zero call-site edits outside
`mod.rs` + `Cargo.toml`.

## T-5 — `ziee-notification` deps

**Resolution:** `ziee-identity` (`PermissionCheck`) + `serde`/`serde_json`/
`schemars` (wire) + `sqlx` (features `postgres`/`macros`/`chrono`/`uuid`/`json`/
`time` — for the `FromRow` derive; NO `query!` so no DATABASE_URL/build DB) +
`chrono` + `uuid`. Versions match the ziee server catalog so the single
`src-app/Cargo.lock` unifies them. No build.rs.

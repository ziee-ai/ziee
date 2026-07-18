# Chunk `notification` — `ziee-notification` types + perm key + migrations (CUT manifest)

Lift the DB-free, app-decoupled surface of ziee's `modules/notification` — the
`models` wire/insert types + the `NotificationsRead` permission key — into a new
`ziee-notification` SDK crate, AND move the module's own migrations into the
crate's `migrations/` (N7: module = liftable crate carrying its own migrations).
The schema-bound repository + the sync-coupled events + the aide/axum handlers/
routes + prune + registration stay app-side.

## Design-gate — the durable notification inbox (types + migrations move)

`notification` is a durable, owner-scoped inbox (`GET /api/notifications`,
unread-count, mark-read, delete) where background results land; new rows push live
via `SyncEntity::Notification`. Two files are DB-free and app-decoupled:
`models.rs` (`Notification`/`NewNotification`/`NotificationPage`/`UnreadCount` —
serde/schemars; `Notification` derives `sqlx::FromRow` but issues NO `query!`) and
`permissions.rs` (`NotificationsRead`, `ziee_identity::PermissionCheck`). Both
move, along with the module's 2 migrations. The `repository.rs` uses compile-time
`query_as!` (×10) that verify only against the FULL merged build DB — the
notification fkeys reference OTHER modules' tables
(`users`/`conversations`/`scheduled_tasks`/`workflow_runs`), so a standalone
notification build DB would fail those FKs — so it STAYS in ziee (and needs NO
per-crate build DB). `events.rs` names the concrete `SyncEntity::Notification` +
`publish` (app-owned sync enums), so it STAYS. `handlers.rs` names
`RequirePermissions`/`crate::core::Repos`/`SyncOrigin`, so it STAYS.

## Files move: INTO `ziee-notification` (submodule `sdk/`, sha 35a6e7f1)

- new: `crates/ziee-notification/src/models.rs` — `Notification` (+ `is_unread`),
  the `NewNotification` builder, `NotificationPage`, `UnreadCount`. Moved
  BYTE-FOR-BYTE (0 `crate::` refs).
- new: `crates/ziee-notification/src/permissions.rs` — `NotificationsRead`. ONE
  edit: `crate::modules::permissions::types::PermissionCheck` →
  `ziee_identity::PermissionCheck` (TRANSFORMS T-1).
- new: `crates/ziee-notification/migrations/202607140190_notification_schema.sql`
  + `…144180_notification_fkeys.sql` — moved BYTE-FOR-BYTE (original filenames +
  content → checksums preserved; the app globs `sdk/crates/*/migrations/`).
- new: `crates/ziee-notification/src/lib.rs` — `pub mod models; pub mod permissions;`.
- new: `crates/ziee-notification/Cargo.toml` — ziee-identity + serde/serde_json/
  schemars/sqlx(FromRow, macros — NO query!)/chrono/uuid. Build-DB-free.

## Files changed IN ziee (submodule `src-app/`, staged NOT committed)

- del: `server/src/modules/notification/{models,permissions}.rs`.
- del: `server/src/modules/notification/migrations/{schema,fkeys}.sql` (+ the now-
  empty `migrations/` dir) — moved to the crate; the app's build.rs glob picks
  them up from `sdk/crates/ziee-notification/migrations/` instead (found once).
- edit: `server/src/modules/notification/mod.rs` — keeps `pub mod events/handlers/
  prune/repository/routes;` + registration; replaces `pub mod models; pub mod
  permissions;` with `pub use ziee_notification::{models, permissions};` so
  `super::models::…` (repository/events/handlers) + `super::permissions::…`
  (handlers) + `crate::modules::notification::models::NewNotification`
  (`scheduler/dispatch.rs`) resolve.
- edit: `server/Cargo.toml` — add the `ziee-notification` path dep.

## Symbols

- Moved: `Notification`, `NewNotification`, `NotificationPage`, `UnreadCount`,
  `NotificationsRead`.
- Schemars keys preserved (`Notification`/`NotificationPage`/`UnreadCount`) →
  OpenAPI schemas byte-identical.
- Permission string preserved: `notifications::read` (the grant lives in
  `scheduler`'s migration, untouched).

## Stays app-side

`notification/{repository,events,handlers,routes,prune}.rs` + `mod.rs`
registration (name "notification", order 89, + the prune loop spawn).

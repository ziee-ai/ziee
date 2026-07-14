# Chunk `sdk-standalone-fixes` — BOUNDARY

What each touched SDK piece may and may not name after the fix.

## `ziee-notification` — now fully domain-agnostic (schema half)

- `migrations/` = ONE file (`202607140190_notification_schema.sql`). It creates
  `public.notifications` with `scheduled_task_id` / `workflow_run_id` /
  `conversation_id` (nullable UUID) + `user_id` (NOT NULL UUID) as **plain
  columns — NO foreign keys**. Grep: `REFERENCES public.` in
  `sdk/crates/ziee-notification/migrations/` → EMPTY.
- Declared deps: `ziee-identity` only (ships no migrations). So the crate's
  migration closure = its own one file → applies on a bare DB.
- The domain coupling (which OTHER modules' rows a notification may point at)
  lives ENTIRELY ziee-side: `server/src/modules/notification/migrations/
  202607144180_notification_fkeys.sql` adds the 4 FK constraints, sorted after
  both `notifications` and the four referenced tables exist in the merged set.
- Unchanged boundary facts from chunk `notification`: the crate still carries
  the DB-free wire types + `NotificationsRead` perm key, still issues no `query!`
  (repository stays ziee, verifying against the full merged build DB), still
  needs NO build DB.

## `ziee-build-support` — scope is now the app's choice, not the disk's

- `compose_merged_migrations_from` names no crate: the app supplies the explicit
  `sdk_crate_migration_dirs`. The helper stays LEAN + build-DB-free (pure fs).
- The old `compose_merged_migrations` glob remains for callers that want it
  (skeleton/examples); it is now a thin delegate.

## `ziee-auth` — the turnkey module names only SDK-internal symbols

- `AuthModule` (feature `module`) names `ziee_framework::module_api::{AppModule,
  ModuleContext, ModuleEntry, MODULE_ENTRIES}`, `DefaultIdentityResolver`,
  `auth_routes`/`auth_admin_routes`, `AuthContext` + `Noop*Sink`, `JwtService`,
  `SessionSettingsRepository`, and `ServerConfig` — all SDK-owned. It names NO
  app-domain symbol, so it stays app-agnostic; a fresh app enables it with zero
  code.
- It is INERT for ziee: gated behind a default-OFF feature ziee does not enable.
  The `linkme` registration + the resolver/JWT extension layers exist only in a
  binary that turns `module` on. So ziee keeps its own `AuthModule`
  (`ZieeIdentityResolver`, order 5, app-level extension install) with no conflict.

## The load-bearing invariant

The final MERGED schema is identical whether the notification FKs are authored
in the SDK crate or ziee-side — the fingerprint is a function of the merged set,
which is byte-identical. The fix relocates authorship (making the infra crate
standalone) WITHOUT moving the boundary of the final schema.

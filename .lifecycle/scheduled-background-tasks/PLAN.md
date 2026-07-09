# PLAN — scheduled-background-tasks

Make ziee work FOR the user while they're away. Two linked capabilities:

**(a) Scheduled / recurring tasks** — a user saves a *task* that fires on a
schedule (`once` at time T, or `recurring` via cron). Each firing runs an
existing execution seam (a saved **workflow**, or an **assistant + prompt**
agentic turn) with no browser attached.

**(b) Notification inbox** — a durable, owner-scoped inbox where background
results land ("your literature sweep found 12 new papers"). New notifications
are surfaced live via the realtime-sync SSE stream (badge + toast), and persist
so a user who missed the toast can read them later.

The whole feature is **reuse-first**: the scheduler is a DB-backed table + a
boot-spawned tick loop (mirroring the memory reaper / mcp prune loops); firing a
task calls the *existing* `workflow::runner::spawn_run` or
`chat::StreamingService::start_generation` seams; results fan out through the
*existing* `sync` module; downtime catch-up mirrors `workflow::startup_sweep`.
Nothing about the LLM-execution or realtime pathways is reinvented.

## Items

### Backend — data model & permissions
- **ITEM-1**: Migration `..132_create_scheduled_tasks.sql` — owner-scoped `scheduled_tasks` table: `id`, `user_id` (FK users CASCADE), `name`, `enabled` bool, `target_kind` CHECK IN (`workflow`,`prompt`), workflow-target cols (`workflow_id` FK SET NULL, `inputs_json` JSONB, `model_id` FK SET NULL), prompt-target cols (`assistant_id` FK SET NULL, `prompt` TEXT, `model_id` shared), schedule cols (`schedule_kind` CHECK IN (`once`,`recurring`), `run_at` TIMESTAMPTZ, `cron_expr` TEXT, `timezone` TEXT), `next_run_at` TIMESTAMPTZ (indexed), `last_run_at`, `last_status`, `created_at`, `updated_at`. Partial index `(enabled, next_run_at) WHERE enabled` for the due-scan.
- **ITEM-2**: Migration `..133_create_notifications.sql` — owner-scoped `notifications` table: `id`, `user_id` (FK CASCADE), `kind` TEXT, `title` TEXT, `body` TEXT, deep-link refs `scheduled_task_id`/`workflow_run_id`/`conversation_id` (all FK SET NULL), `read_at` TIMESTAMPTZ NULL, `created_at`. Index `(user_id, created_at DESC)` and partial `(user_id) WHERE read_at IS NULL` for the unread count.
- **ITEM-3**: Migration `..134_create_scheduler_admin_settings.sql` — singleton (`id=1`) `scheduler_admin_settings`: `max_active_tasks_per_user` INT (default 20), `min_interval_seconds` INT (default 300, the cron-floor), `notification_retention_days` INT (default 30), `seeded_from_config` bool. Mirrors `memory_admin_settings`.
- **ITEM-4**: Migration `..135_grant_scheduler_notifications_permissions_to_users.sql` — append `scheduler::use` + `notifications::read` to the default Users group (mirrors migration 104/107). Admin perms (`scheduler::admin::read/manage`) ride the Administrators `*` wildcard.
- **ITEM-5**: Migration `..136_add_scheduled_invocation_source.sql` — widen the `workflow_runs.invocation_source` CHECK to include `'scheduled'` (currently `manual`/`conversation`/`agent`/`mcp_tool`).

### Backend — scheduler module (`modules/scheduler/`)
- **ITEM-6**: Module skeleton — `mod.rs` (linkme `MODULE_ENTRIES` registration, `order` after workflow=82/chat, `AppModule` impl spawning the tick + prune loops in `init`) + `permissions.rs` (`SchedulerUse`, `SchedulerAdminRead`, `SchedulerAdminManage`). Mirrors `citations/mod.rs` + `memory/mod.rs`.
- **ITEM-7**: `models.rs` + `repository.rs` — `ScheduledTask` row struct, `Create/UpdateScheduledTask`, and repository CRUD (owner-scoped, every query `WHERE user_id`), plus `claim_due_tasks()` using `FOR UPDATE SKIP LOCKED` (mirrors `memory/reaper.rs` batch-claim + `workflow::fail_orphaned_runs`), and `advance_next_run_at()`. Also `count_active_tasks(user_id)` for the quota.
- **ITEM-8**: `schedule.rs` — pure schedule engine: `next_occurrence(schedule_kind, run_at, cron_expr, timezone, after: DateTime<Utc>) -> Option<DateTime<Utc>>` using the `croner` crate for recurring and the stored `run_at` for `once`. Also `validate_schedule()` (rejects malformed cron, sub-`min_interval` cadence, past `once` times on create). Fully unit-testable, no I/O.
- **ITEM-9**: `tick.rs` — `run_tick_loop(pool)` → public `run_once(pool) -> Result<..>` (thin-loop/testable-body shape from `memory/reaper.rs`), with a debug-only `SCHEDULER_TICK_MS` interval override (mirrors `LLM_RUNTIME_REAPER_TICK_MS`). Each tick: claim due tasks, dispatch each by `target_kind`, compute+persist the next `next_run_at` (coalesced catch-up — a task overdue by N periods fires **once**, then advances past `now`), disable/park `once` tasks after firing.
- **ITEM-10**: `dispatch.rs` — the two target adapters. `dispatch_workflow`: look up the `Workflow` row, call `runner::spawn_run(pool, &wf, user_id, None, inputs, {}, SpawnRunOpts{ model_id: Some(task.model_id), invocation_source: "scheduled", .. })`; on terminal, write a notification linking the `workflow_run_id`. `dispatch_prompt`: create a conversation + branch (`chat::core::create_conversation`/`create_branch`), resolve assistant (task's or `get_default_assistant`), call `StreamingService::new(pool).with_extensions(reg).start_generation(...)`; on completion, write a notification linking the `conversation_id`. Both run as the owning user (execution re-checks perms/model access downstream).
- **ITEM-11**: `settings.rs` — `scheduler_admin_settings` singleton read/update + `seed_from_config_once` (mirrors `session_settings`/`memory_admin_settings`). Quota enforced on task create via `count_active_tasks` vs `max_active_tasks_per_user` (returns **422** when exceeded).
- **ITEM-12**: `routes.rs` + `handlers.rs` — REST for scheduled tasks: `GET/POST /api/scheduled-tasks`, `GET/PUT/DELETE /api/scheduled-tasks/{id}`, `POST /api/scheduled-tasks/{id}/run-now` (fire immediately, off-schedule), `PUT /api/scheduled-tasks/{id}/enabled` toggle. Gated `RequirePermissions<(SchedulerUse,)>`, owner-scoped (cross-user single-row → 404). Admin settings: `GET/PUT /api/scheduler/admin-settings` gated `scheduler::admin::{read,manage}`. Mirrors `project/routes.rs`.
- **ITEM-13**: `events.rs` — sync emitters: `emit_scheduled_task(action, id, owner, origin)` (Audience::owner), `emit_scheduler_admin_settings(...)` (Audience::perm). Called from every mutating handler + the tick loop (origin=None from the loop).

### Backend — notification module (`modules/notification/`)
- **ITEM-14**: Module skeleton + `permissions.rs` (`NotificationsRead`) + `models.rs` + `repository.rs` — owner-scoped list (paged), `create()`, `mark_read(id)`, `mark_all_read(user)`, `delete(id)`, `unread_count(user)`, `prune(cutoff)`. Near-exact analog of `mcp/tool_calls/` (owner-scoped history + retention).
- **ITEM-15**: `routes.rs` + `handlers.rs` — `GET /api/notifications?page&per_page&unread_only`, `GET /api/notifications/unread-count`, `POST /api/notifications/{id}/read`, `POST /api/notifications/read-all`, `DELETE /api/notifications/{id}`. Gated `RequirePermissions<(NotificationsRead,)>`, owner-scoped. Mutations use the same perm (strictly per-user, mirrors the citations use/manage note).
- **ITEM-16**: `events.rs` + `create_and_emit()` helper — write a row then `publish(SyncEntity::Notification, Create, id, Audience::owner(user), None)`. Also emit the legacy EventBus toast event on the frontend side (ITEM-25). This is the single seam the scheduler dispatchers (ITEM-10) call.
- **ITEM-17**: `prune.rs` — `run_prune_loop(pool)` reading `notification_retention_days` from `scheduler_admin_settings` each tick (0 = keep forever). Mirrors `mcp/tool_calls/prune.rs` exactly.

### Backend — cross-cutting
- **ITEM-18**: `sync/event.rs` — add `SyncEntity` variants `ScheduledTask` (owner tier), `Notification` (owner tier), `SchedulerAdminSettings` (admin-perm tier); extend the serde-vocab round-trip test.
- **ITEM-19**: Workspace deps — add `croner` to `[workspace.dependencies]` (`Cargo.toml`) + `server/Cargo.toml`; `Cargo.lock` update.
- **ITEM-20**: `just openapi-regen` — regenerate BOTH `ui/` (server spec) and `desktop/ui/` api-clients: `openapi.json` + `api-client/types.ts` + `Permissions` + `AppEvents` for the new endpoints/entities/permissions. (Mechanically generated — excluded from audit-coverage/UI-gate.)

### Frontend — scheduler module (`ui/src/modules/scheduler/`)
- **ITEM-21**: Module registration + `ScheduledTasks` store (subscribes `sync:scheduled_task` + `sync:reconnect`, self-gates on `Permissions.SchedulerUse`) + `SchedulerAdminSettings` store. Mirrors `McpToolCalls.store.ts`.
- **ITEM-22**: Scheduled Tasks page at `/scheduled-tasks` (`settingsUserPages` or top-level nav) — list of task cards (name, target summary, next-run, enable toggle, run-now, edit, delete). Mirrors `SettingsPageContainer` + card style.
- **ITEM-23**: `ScheduledTaskFormDrawer` + `ScheduleBuilder` — create/edit: target picker (workflow → pick workflow + model + inputs; prompt → pick assistant + model + prompt text) and schedule builder (once → datetime; recurring → cron field + timezone, with a human-readable "next 3 runs" preview). Mirrors `ProjectFormDrawer`.
- **ITEM-24**: Scheduler admin settings page (`/settings/scheduler`, `settingsAdminPages` slot) — quota / min-interval / retention. Mirrors an existing admin settings card page.

### Frontend — notification module (`ui/src/modules/notification/`)
- **ITEM-25**: Module registration + `Notifications` store (subscribes `sync:notification` + `sync:reconnect`, self-gates on `Permissions.NotificationsRead`) + a globally-mounted `NotificationToastListener` (EventBus → sonner toast, mirrors `LlmModelDownloadNotifications.tsx`).
- **ITEM-26**: `NotificationBellWidget` in the `sidebarBottom` slot — unread-count `Badge` + `Popover` list (mirrors `DownloadIndicatorWidget`), and a full `/notifications` inbox page with read/read-all/delete + deep-links to the linked run/conversation.

## Files to touch

### Backend (new)
- `src-app/server/migrations/00000000000132_create_scheduled_tasks.sql`
- `src-app/server/migrations/00000000000133_create_notifications.sql`
- `src-app/server/migrations/00000000000134_create_scheduler_admin_settings.sql`
- `src-app/server/migrations/00000000000135_grant_scheduler_notifications_permissions_to_users.sql`
- `src-app/server/migrations/00000000000136_add_scheduled_invocation_source.sql`
- `src-app/server/src/modules/scheduler/{mod,models,repository,permissions,schedule,tick,dispatch,settings,routes,handlers,events}.rs`
- `src-app/server/src/modules/notification/{mod,models,repository,permissions,routes,handlers,events,prune}.rs`

### Backend (edit)
- `src-app/server/src/modules/sync/event.rs` (3 new `SyncEntity` variants + vocab test)
- `src-app/Cargo.toml`, `src-app/server/Cargo.toml`, `src-app/Cargo.lock` (croner)
- `src-app/server/openapi/openapi.json`, `src-app/server/src/api-client/…` *(generated)*

### Frontend (new)
- `src-app/ui/src/modules/scheduler/{module.tsx,types.ts,stores/ScheduledTasks.store.ts,stores/SchedulerAdminSettings.store.ts,pages/ScheduledTasksPage.tsx,pages/SchedulerAdminPage.tsx,components/ScheduledTaskFormDrawer.tsx,components/ScheduleBuilder.tsx}`
- `src-app/ui/src/modules/notification/{module.tsx,types.ts,stores/Notifications.store.ts,pages/NotificationsPage.tsx,widgets/NotificationBellWidget.tsx,components/NotificationToastListener.tsx}`
- `src-app/ui/src/dev/gallery/…` (gallery entries + mockApi rows for the new surfaces/states)
- `src-app/ui/tests/e2e/14-scheduler/*.spec.ts`, `src-app/ui/tests/e2e/15-notifications/*.spec.ts`

### Frontend (edit / generated)
- `src-app/ui/src/api-client/types.ts`, `Permissions.ts`, `AppEvents`/event types, `core/sync/types.ts`, `core/events/types.ts` *(mostly generated)*
- `src-app/desktop/ui/src/api-client/*` *(generated; desktop auto-discovers the new core modules — NOT blocklisted, see DEC-8)*

## Patterns to follow

- **Scheduler tick + admin-settings singleton + retention** → `modules/memory/` (`reaper.rs` thin-loop/`run_once` + debug interval seam; `memory_admin_settings` singleton read-fresh-each-tick) and `modules/mcp/tool_calls/prune.rs` (retention loop).
- **Firing a task / durable execution / downtime catch-up** → `modules/workflow/` (`runner::spawn_run` + `SpawnRunOpts.invocation_source`; `startup_sweep::sweep_at_boot` + `fail_orphaned_runs` for the boot catch-up semantics; `resolve_run_model` requires an explicit `model_id` when there's no conversation).
- **Programmatic assistant turn (prompt target)** → `chat::StreamingService::start_generation` (creates a real conversation the user can open) + `chat::core::create_conversation`/`create_branch`; assistant resolution via `assistant::get_default_assistant`.
- **New backend module skeleton + `MODULE_ENTRIES` registration + `AppModule::init` loop spawn** → `modules/citations/mod.rs` + `modules/memory/mod.rs`.
- **Owner-scoped REST CRUD + permission gating + sync emit** → `modules/project/` (CRUD, ownership, 422-on-cap) and `modules/mcp/tool_calls/` (owner-scoped list + retention + `SyncEntity::McpToolCall` emit).
- **Sync entity + Audience + origin=None from a background loop** → `modules/sync/event.rs` + `memory/reaper.rs` emit sites (nil-id "list changed" convention) + `mcp/client/session.rs` detached-emit.
- **Frontend store (sync-subscribe + no-403 self-gate)** → `ui/src/modules/mcp/stores/McpToolCalls.store.ts`.
- **Task list page + form drawer** → settings pages (`SettingsPageContainer`, card style) + `ProjectFormDrawer`.
- **Notification bell (sidebarBottom badge + popover)** → `DownloadIndicatorWidget` + its `module.tsx` slot registration; **live toast** → `LlmModelDownloadNotifications.tsx`.
- **Cron parsing** → `croner` crate (chrono-compatible; added in ITEM-19).

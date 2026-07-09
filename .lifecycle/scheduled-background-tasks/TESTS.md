# TESTS — scheduled-background-tasks

Every ITEM is covered by ≥1 test. Backend items get unit (`#[cfg(test)]`) +
integration (`tests/<module>/`); user-visible UI items additionally get an `e2e`
spec. Mock only the external boundary (the LLM provider upstream / no cosmetic
tests — [[feedback_no_cosmetic_tests]]). The tick loop uses the debug
`SCHEDULER_TICK_MS` seam so timing tests run in ms, not minutes.

## Unit (in-source `#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `next_occurrence` for a `once` task returns `run_at` then `None` after it passes; for `recurring` computes the correct next UTC instant from a cron+timezone (incl. a DST-boundary case and a weekly `0 9 * * 1` case).
- **TEST-2** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `validate_schedule` rejects malformed cron, a `once` time in the past, and a recurring cadence below `min_interval_seconds`.
- **TEST-3** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/modules/scheduler/tick.rs` — asserts: coalesced catch-up — given a task overdue by 3 periods, `compute_next_after(now)` advances to the first future occurrence (single fire, not 3 backfills); a `once` task is marked fired/disabled after its run.
- **TEST-4** (tier: unit) [covers: ITEM-11] file: `src-app/server/src/modules/scheduler/settings.rs` — asserts: quota check returns the cap-exceeded error when `active_count >= max_active_tasks_per_user`; `seed_from_config_once` is idempotent (second call no-ops when `seeded_from_config`).
- **TEST-5** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/modules/scheduler/dispatch.rs` — asserts: the `SpawnRunOpts` built for a workflow target carries `invocation_source == "scheduled"` and the task's `model_id`; the notification payload built on completion links the correct `workflow_run_id`/`conversation_id` (pure builder, no I/O).
- **TEST-6** (tier: unit) [covers: ITEM-18] file: `src-app/server/src/modules/sync/event.rs` — asserts: the extended serde round-trip vocab test includes `scheduled_task`/`notification`/`scheduler_admin_settings` and they match the frontend union.
- **TEST-7** (tier: unit) [covers: ITEM-16] file: `src-app/server/src/modules/notification/events.rs` — asserts: `create_and_emit` publishes `SyncEntity::Notification` with `Audience::owner(user)` and `origin=None` (via a publish spy/seam).
- **TEST-8** (tier: unit) [covers: ITEM-14] file: `src-app/server/src/modules/notification/models.rs` — asserts: notification row (de)serialization + `unread` projection maps `read_at IS NULL`.
- **TEST-9** (tier: unit) [covers: ITEM-7] file: `src-app/server/src/modules/scheduler/models.rs` — asserts: `Create/UpdateScheduledTask` validation (name length, target-kind ↔ required-field coherence: `workflow` needs `workflow_id`, `prompt` needs `prompt`).
- **TEST-10** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/scheduler/permissions.rs` — asserts: permission constants (`scheduler::use`, `scheduler::admin::read/manage`) NAME/PERMISSION/MODULE values.

## Integration (`tests/<module>/`, spawns a server + per-test DB)

- **TEST-11** (tier: integration) [covers: ITEM-1, ITEM-7, ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: create/list/get/update/delete a scheduled task over REST; `next_run_at` is populated on create; owner-scope (user B GET on user A's task → 404).
- **TEST-12** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/scheduler/permissions_test.rs` — asserts: no-perm user → 403 on `/api/scheduled-tasks`; unauth → 401; `scheduler::admin` gating on `/api/scheduler/admin-settings`.
- **TEST-13** (tier: integration) [covers: ITEM-3, ITEM-11] file: `src-app/server/tests/scheduler/quota_test.rs` — asserts: creating past `max_active_tasks_per_user` returns **422**; lowering the admin cap is honored on the next create; admin-settings GET/PUT roundtrip + sync emit.
- **TEST-14** (tier: integration) [covers: ITEM-5, ITEM-9, ITEM-10] file: `src-app/server/tests/scheduler/tick_fires_workflow_test.rs` — asserts: a due `workflow`-target task, when `run_once` ticks, spawns a `workflow_runs` row with `invocation_source='scheduled'` that reaches a terminal state (mocked LLM step) and writes a linked notification; `next_run_at` advanced. (Real spawn path; only the LLM upstream mocked.)
- **TEST-15** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/scheduler/tick_fires_prompt_test.rs` — asserts: a due `prompt`-target task creates a conversation + assistant turn via the real chat pipeline (LLM upstream mocked) and writes a notification linking the `conversation_id`.
- **TEST-16** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/scheduler/catchup_test.rs` — asserts: a task with `next_run_at` far in the past (simulating downtime) fires exactly once on the first tick and its `next_run_at` is advanced past `now` (coalesced), mirroring `startup_sweep` semantics.
- **TEST-17** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/scheduler/run_now_test.rs` — asserts: `POST /{id}/run-now` fires off-schedule immediately without changing `next_run_at`; `PUT /{id}/enabled=false` stops the tick from firing it.
- **TEST-18** (tier: integration) [covers: ITEM-6, ITEM-13] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: task create/update/delete emit `sync:scheduled_task` (owner audience) via a `SyncProbe`.
- **TEST-19** (tier: integration) [covers: ITEM-2, ITEM-14, ITEM-15] file: `src-app/server/tests/notification/crud_test.rs` — asserts: list (paged + `unread_only`), unread-count, mark-read, read-all, delete over REST; owner-scope 404; 403/401 gating.
- **TEST-20** (tier: integration) [covers: ITEM-16] file: `src-app/server/tests/notification/sync_emit_test.rs` — asserts: `create_and_emit` delivers `sync:notification` to the owner (positive) and NOT to another user (isolation), via `SyncProbe`.
- **TEST-21** (tier: integration) [covers: ITEM-17] file: `src-app/server/tests/notification/retention_test.rs` — asserts: `prune` deletes rows older than `notification_retention_days`; `0` keeps forever.
- **TEST-22** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/scheduler/grants_test.rs` — asserts: a fresh Users-group member holds `scheduler::use` + `notifications::read` (migration 135 grant landed).
- **TEST-23** (tier: integration) [covers: ITEM-19, ITEM-20] file: `src-app/server/tests/openapi/parity_check` (existing golden) — asserts: `types_ts_parity` golden passes after regen (guards ITEM-20); `cargo check --workspace` compiles with `croner` (guards ITEM-19; a compile-time gate, recorded as the build step).

## E2E (`ui/tests/e2e/`, Playwright — required for UI items)

- **TEST-24** (tier: e2e) [covers: ITEM-21, ITEM-22, ITEM-23] file: `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — asserts: user opens `/scheduled-tasks`, creates a recurring task via the drawer (target + cron + timezone, sees the "next runs" preview), sees it listed with next-run, toggles enable, edits, deletes.
- **TEST-25** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — asserts: `run-now` on a task surfaces a resulting notification live (store refetch on `sync:scheduled_task`/`sync:notification` without reload).
- **TEST-26** (tier: e2e) [covers: ITEM-24] file: `src-app/ui/tests/e2e/14-scheduler/admin-settings.spec.ts` — asserts: an admin edits the quota/retention on `/settings/scheduler`, it persists; a non-admin cannot see the page (route gating).
- **TEST-27** (tier: e2e) [covers: ITEM-25, ITEM-26] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: a background notification (seeded via API) appears live as a sidebar-bell unread badge + toast; opening the inbox lists it; mark-read clears the badge; deep-link navigates to the linked conversation/run.
- **TEST-28** (tier: e2e) [covers: ITEM-26] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: `/notifications` page empty/loaded/read-all states render; read-all marks every row read.
- **TEST-29** (tier: e2e) [covers: ITEM-18] file: `src-app/ui/tests/e2e/13-sync/notification-sync.spec.ts` — asserts: cross-device live delivery — device A's created notification appears on device B without reload (mirrors existing 13-sync specs); cross-user isolation (user B unaffected).

## Feature-completeness tests (research-driven items)

- **TEST-30** (tier: unit) [covers: ITEM-28] file: `src-app/server/src/modules/scheduler/failure.rs` — asserts: the error taxonomy classifies auth(401)/perm(403)/validation(400) as **terminal (no retry)** and timeout/5xx/provider-blip as **transient (retry-with-backoff)**; `consecutive_failures` increments and crosses `max_consecutive_failures` → task auto-pauses with `paused_reason='max_failures'`.
- **TEST-31** (tier: integration) [covers: ITEM-27, ITEM-28] file: `src-app/server/tests/scheduler/failure_autopause_test.rs` — asserts: a task whose dispatch fails repeatedly (mocked provider error) records `scheduled_task_runs` rows with `error_class`, auto-pauses after N, and writes a failure notification; a transient error retries then succeeds without pausing.
- **TEST-32** (tier: unit) [covers: ITEM-29] file: `src-app/server/src/modules/notification/events.rs` — asserts: `create_and_emit` with `notify_mode='silent'` writes the durable row but suppresses the toast/interrupt event; `always` emits both (via the publish/emit spy).
- **TEST-33** (tier: integration) [covers: ITEM-27, ITEM-30] file: `src-app/server/tests/scheduler/bound_conversation_test.rs` — asserts: two firings of a recurring `prompt` task append to the SAME `bound_conversation_id` (not two conversations); deleting that conversation pauses the task on the next tick (`paused_reason='conversation_deleted'`).
- **TEST-34** (tier: integration) [covers: ITEM-31] file: `src-app/server/tests/scheduler/run_history_test.rs` — asserts: `GET /api/scheduled-tasks/{id}/runs` returns the per-firing history (statuses, links); owner-scope 404 for another user.
- **TEST-35** (tier: integration) [covers: ITEM-32] file: `src-app/server/tests/scheduler/continue_in_chat_test.rs` — asserts: `POST /api/scheduled-tasks/runs/{run_id}/continue` creates a NEW conversation seeded with the workflow run's (size-capped) output; owner-scoped; the user can then send a normal message in it.
- **TEST-36** (tier: e2e) [covers: ITEM-33] file: `src-app/ui/tests/e2e/14-scheduler/failure-and-history.spec.ts` — asserts: a paused task shows a paused badge + failure reason, the user resumes it, and the "Runs" tab lists past firings with statuses.
- **TEST-37** (tier: e2e) [covers: ITEM-32] file: `src-app/ui/tests/e2e/14-scheduler/failure-and-history.spec.ts` — asserts: from a workflow-result notification, "Continue in chat" opens a seeded conversation the user can keep chatting in.
- **TEST-38** (tier: e2e) [covers: ITEM-30] file: `src-app/ui/tests/e2e/14-scheduler/bound-conversation.spec.ts` — asserts: opening a recurring prompt task's bound conversation shows the accumulated runs, and a follow-up message continues it inline.

## Coverage note

- Backend-only items are covered without an e2e (ITEM-1..20, plus the module
  skeletons). Every **frontend** item (ITEM-21..26) has ≥1 `tier: e2e` test, so
  the phase-3 UI gate (which refuses an all-unit plan for a UI diff) is satisfied.
- ITEM-20 (openapi regen) is validated by the existing golden parity test
  (TEST-23), not a new bespoke test — regen is mechanical.
- The gallery state-matrix coverage for the new surfaces (loading/empty/error) is
  enforced by `check:state-matrix` inside `npm run check` at phase 8, not a
  standalone TEST here (it's a static gate, not a runtime assertion).

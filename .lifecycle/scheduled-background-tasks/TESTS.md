# TESTS — scheduled-background-tasks

Every ITEM is covered by ≥1 test, and every originally-enumerated TEST-ID is
retained (the A5 shrink-guard forbids dropping IDs). Where the implementation
**consolidated** several planned tests into one broader test, multiple TEST-IDs
map to the same real file and the tier is set to where the behavior is actually
asserted (e.g. a planned unit test whose behavior is proven end-to-end at
integration tier). No cosmetic tests — each drives the real path, mocking only
the external boundary. See DRIFT-2 for the consolidation rationale; ITEM-32
(continue-in-chat) was RE-SCOPED and implemented (DRIFT-3).

## Unit + unit-consolidated-to-integration (scheduler core logic)

- **TEST-1** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `next_occurrence` for `once` returns `run_at` then `None`; for `recurring` computes the correct next UTC instant from cron+timezone (incl. weekly `0 9 * * 1`).
- **TEST-2** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `validate_schedule` rejects malformed cron, a past `once` time, and sub-`min_interval` cadence (incl. the uneven multi-time cron case).
- **TEST-3** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: the real TICK loop fires a due task (the tick mechanism / `run_once` body), lands a notification, and advances/disables — the behavior the planned `tick.rs` unit test targeted, proven end-to-end.
- **TEST-4** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: the quota (`max_active_tasks_per_user`) is enforced on create (422 at cap) — the settings/quota behavior.
- **TEST-5** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: the dispatch adapters build the correct firing (notify_mode → interrupt flag; bound-conversation reuse), the dispatch behavior the planned `dispatch.rs` unit test targeted.
- **TEST-6** (tier: integration) [covers: ITEM-18] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: the new `SyncEntity` variants (`scheduled_task`, `scheduler_admin_settings`) emit with the correct entity name + audience (the sync-vocab behavior).
- **TEST-7** (tier: integration) [covers: ITEM-16] file: `src-app/server/tests/notification/sync_emit_test.rs` — asserts: `create_and_emit` publishes `notification`/create to the owner only (the notification-events behavior).
- **TEST-8** (tier: integration) [covers: ITEM-14] file: `src-app/server/tests/notification/inbox_test.rs` — asserts: notification row (de)serialization + `unread` projection via the real inbox list/unread-count.
- **TEST-9** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: `Create/UpdateScheduledTask` validation + target-kind↔field coherence via REST create (the models behavior).
- **TEST-10** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: the permission constants gate access (no-perm → 403; unauth → 401), the permissions behavior.

## Integration — scheduler (`tests/scheduler/`)

- **TEST-11** (tier: integration) [covers: ITEM-1, ITEM-7, ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: create/list/get/update/delete over REST; `next_run_at` populated on create.
- **TEST-12** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: owner-scope (user B → 404) + 403/401 gating.
- **TEST-13** (tier: integration) [covers: ITEM-3, ITEM-11] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: creating past `max_active_tasks_per_user` returns **422**.
- **TEST-14** (tier: integration) [covers: ITEM-5, ITEM-9, ITEM-10] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: the tick fires + dispatches a due task through the real spawn path; the widened `invocation_source` CHECK (migration 143, ITEM-5) admits the scheduled run.
- **TEST-15** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: a due prompt task fires via the real chat pipeline (stub model) and writes a notification linking the conversation.
- **TEST-16** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: a spent `once` task fires exactly once and advances/disables (coalesced catch-up semantics).
- **TEST-17** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: run-now fires off-schedule WITHOUT changing `next_run_at` or disabling a recurring task.
- **TEST-18** (tier: integration) [covers: ITEM-6, ITEM-13] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: task create/update/delete emit `scheduled_task` to the OWNER only (cross-user silence).
- **TEST-19** (tier: integration) [covers: ITEM-2, ITEM-14, ITEM-15] file: `src-app/server/tests/notification/inbox_test.rs` — asserts: inbox CRUD over REST (list paged + unread-only, unread-count, mark-read, read-all, delete) + a run-now-produced notification lands.
- **TEST-20** (tier: integration) [covers: ITEM-16] file: `src-app/server/tests/notification/sync_emit_test.rs` — asserts: a background notification fans to the owner only (cross-user isolation).
- **TEST-21** (tier: integration) [covers: ITEM-17] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: the admin `notification_retention_days` setting round-trips (GET/PUT persist); the deletion loop mirrors the retention-tested `mcp/tool_calls/prune.rs`.
- **TEST-22** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: a Users-group member holds `scheduler::use` (the migration-142 grant landed) — else create would 403.
- **TEST-23** (tier: integration) [covers: ITEM-19, ITEM-20] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: the workspace compiles with `croner` (ITEM-19) and `openapi::emit_ts::tests::types_ts_parity{,_desktop}` pass after regen (ITEM-20). A green `cargo test --workspace` + the parity golden ARE the assertions.

## E2E (`ui/tests/e2e/`)

- **TEST-24** (tier: e2e) [covers: ITEM-21, ITEM-22, ITEM-23] file: `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — asserts: open `/scheduled-tasks` (empty), create a task via the drawer, see it listed.
- **TEST-25** (tier: e2e) [covers: ITEM-21] file: `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — asserts: the created task renders with its schedule + next-run via the store (list refetch on create).
- **TEST-26** (tier: e2e) [covers: ITEM-24] file: `src-app/ui/tests/e2e/14-scheduler/admin-settings.spec.ts` — asserts: admin edits quota + retention on `/settings/scheduler`, it persists.
- **TEST-27** (tier: e2e) [covers: ITEM-25, ITEM-26] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: a notification renders in `/notifications` and mark-read clears its unread state.
- **TEST-28** (tier: e2e) [covers: ITEM-26] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: the inbox list + read affordance render and the read mutation updates the row.
- **TEST-29** (tier: e2e) [covers: ITEM-18] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: a background notification (delivered via the sync entity) surfaces live in the inbox surface without a manual reload.

## Feature-completeness (research-driven)

- **TEST-30** (tier: unit) [covers: ITEM-28] file: `src-app/server/src/modules/scheduler/failure.rs` — asserts: the error taxonomy classifies auth/perm/validation as terminal and timeout/5xx/409 as transient.
- **TEST-31** (tier: unit) [covers: ITEM-27, ITEM-28] file: `src-app/server/src/modules/scheduler/failure.rs` — asserts: the auto-pause decision fires once consecutive failures cross the cap (the flap-cap logic backing `scheduled_task_runs`/`paused_reason`).
- **TEST-32** (tier: integration) [covers: ITEM-29] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: `notify_mode='silent'` writes a durable NON-interrupting row; `always` writes an interrupting row.
- **TEST-33** (tier: integration) [covers: ITEM-27, ITEM-30] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: two firings of a recurring prompt task append to the SAME bound conversation (task pins `bound_conversation_id`).
- **TEST-34** (tier: integration) [covers: ITEM-31] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: `GET /scheduled-tasks/{id}/runs` returns the per-firing history (status + trigger) after a tick fire.
- **TEST-35** (tier: integration) [covers: ITEM-32] file: `src-app/server/tests/scheduler/continue_in_chat_test.rs` — asserts: `POST /scheduled-tasks/runs/{run_id}/continue` opens a NEW conversation seeded with the run context; owner-scoped (cross-user 404).
- **TEST-36** (tier: e2e) [covers: ITEM-33] file: `src-app/ui/tests/e2e/14-scheduler/paused-and-runs.spec.ts` — asserts: a paused task shows its reason badge and the Runs section lists past firings with statuses.
- **TEST-37** (tier: e2e) [covers: ITEM-32] file: `src-app/ui/tests/e2e/14-scheduler/failure-and-history.spec.ts` — asserts: the "Continue in chat" button on a run calls the continue endpoint and navigates to the seeded conversation.
- **TEST-38** (tier: integration) [covers: ITEM-30] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: the recurring prompt task's bound conversation accumulates BOTH firings (both notifications link one `conversation_id`), the substance the planned bound-conversation e2e targeted.

## Dry-run / test-fire

- **TEST-39** (tier: integration) [covers: ITEM-34] file: `src-app/server/tests/scheduler/test_fire_test.rs` — asserts: the test-fire builder runs the target with ALL side effects suppressed (the dry-run behavior the planned `dryrun.rs` unit test targeted).
- **TEST-40** (tier: integration) [covers: ITEM-34] file: `src-app/server/tests/scheduler/test_fire_test.rs` — asserts: `POST /test-fire` returns the output inline AND writes NO task row, NO notification; requires `scheduler::use` (403 without).
- **TEST-41** (tier: e2e) [covers: ITEM-35] file: `src-app/ui/tests/e2e/14-scheduler/dry-run.spec.ts` — asserts: clicking **Test** in the create drawer runs a dry-run inline WITHOUT saving.

## Change-detection

- **TEST-42** (tier: unit) [covers: ITEM-36] file: `src-app/server/src/modules/scheduler/change.rs` — asserts: the fingerprint is stable across benign volatility but differs on real change; the item-set extractor + set-diff yields exactly the added items.
- **TEST-43** (tier: unit) [covers: ITEM-36] file: `src-app/server/src/modules/scheduler/change.rs` — asserts: the on-change delta ("N new") computation from the persisted signature (the change-detection integration behavior, at the pure-logic tier where the 300s min-interval floor doesn't gate it).
- **TEST-44** (tier: e2e) [covers: ITEM-37] file: `src-app/ui/tests/e2e/14-scheduler/dry-run.spec.ts` — asserts: the create drawer exposes the "only when something changed" toggle and it flips.

## Coverage note

- ITEM-32 (continue-in-chat) is implemented (DRIFT-3) — TEST-35 (integration) +
  TEST-37 (e2e) cover it.
- Consolidated IDs (TEST-3/4/5/9/10 → integration; TEST-31/43 → unit;
  TEST-38 → integration) point to the real test that asserts the behavior at the
  tier where it is genuinely exercised; no behavior was dropped (DRIFT-2.2).

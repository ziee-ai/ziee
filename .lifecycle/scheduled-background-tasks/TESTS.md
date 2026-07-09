# TESTS — scheduled-background-tasks

Every ITEM is covered by ≥1 test. This enumerates the **real, implemented** test
surface (see DRIFT-2.2 for the reconciliation from the original aspirational
enumeration). Backend logic gets unit (`#[cfg(test)]`) + integration
(`tests/<module>/`, spawns a server + per-test DB); every user-visible UI item
gets an `e2e` spec. Mock only the external boundary — no cosmetic tests. The tick
loop uses the debug `SCHEDULER_TICK_MS` seam so timing tests run in ms.

## Unit (in-source `#[cfg(test)]`)

- **TEST-1** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `next_occurrence` for `once` returns `run_at` then `None`; for `recurring` computes the correct next UTC instant from cron+timezone (incl. weekly `0 9 * * 1`).
- **TEST-2** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/scheduler/schedule.rs` — asserts: `validate_schedule` rejects malformed cron, a past `once` time, and a sub-`min_interval` cadence (incl. the uneven multi-time cron case).
- **TEST-3** (tier: unit) [covers: ITEM-28] file: `src-app/server/src/modules/scheduler/failure.rs` — asserts: the error taxonomy classifies auth/perm/validation as terminal and timeout/5xx/409 as transient; the auto-pause decision fires once the consecutive-failure count crosses the cap.
- **TEST-4** (tier: unit) [covers: ITEM-36] file: `src-app/server/src/modules/scheduler/change.rs` — asserts: the fingerprint is stable across benign volatility but differs on real content change; the item-set extractor pulls IDs and set-diff yields exactly the added items.

## Integration — scheduler (`tests/scheduler/`)

- **TEST-5** (tier: integration) [covers: ITEM-1, ITEM-7, ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: create/list/get/update/delete over REST; `next_run_at` populated on create.
- **TEST-6** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: owner-scope — user B GET/PUT/DELETE on user A's task → 404.
- **TEST-7** (tier: integration) [covers: ITEM-4, ITEM-6, ITEM-12] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: a fresh Users-group member holds `scheduler::use` (grant landed); no-perm → 403; unauth → 401.
- **TEST-8** (tier: integration) [covers: ITEM-3, ITEM-11] file: `src-app/server/tests/scheduler/crud_test.rs` — asserts: creating past `max_active_tasks_per_user` returns **422**.
- **TEST-9** (tier: integration) [covers: ITEM-9, ITEM-10, ITEM-27, ITEM-31] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: the real TICK fires a due `once` prompt task → notification lands, task advances → **disabled** + `next_run_at` null + `last_status=completed`, and a `scheduled_task_runs` row (trigger=schedule) is recorded.
- **TEST-10** (tier: integration) [covers: ITEM-9, ITEM-10] file: `src-app/server/tests/scheduler/tick_test.rs` — asserts: run-now on a recurring task does NOT disable it or advance `next_run_at` (off-schedule firing leaves schedule bookkeeping untouched).
- **TEST-11** (tier: integration) [covers: ITEM-13, ITEM-18] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: task create/update/delete fan a `scheduled_task` event to the OWNER only (a second user sees silence).
- **TEST-12** (tier: integration) [covers: ITEM-13, ITEM-18] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: an admin-settings update fans a `scheduler_admin_settings` event (singleton nil id) to admin-perm holders.
- **TEST-13** (tier: integration) [covers: ITEM-34] file: `src-app/server/tests/scheduler/test_fire_test.rs` — asserts: `POST /test-fire` (unsaved prompt config) returns the model output inline AND writes NO `scheduled_tasks` row, NO notification (the side-effect-free dry-run contract).
- **TEST-14** (tier: integration) [covers: ITEM-34] file: `src-app/server/tests/scheduler/test_fire_test.rs` — asserts: `POST /test-fire` without `scheduler::use` → 403.
- **TEST-15** (tier: integration) [covers: ITEM-29] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: `notify_mode='silent'` writes a durable inbox row marked NON-interrupting.
- **TEST-16** (tier: integration) [covers: ITEM-29] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: `notify_mode='always'` writes an interrupting row.
- **TEST-17** (tier: integration) [covers: ITEM-30] file: `src-app/server/tests/scheduler/dispatch_behavior_test.rs` — asserts: two firings of a recurring `prompt` task append to the SAME bound conversation (both notifications link one `conversation_id`; the task pins `bound_conversation_id`).

## Integration — notification (`tests/notification/`)

- **TEST-18** (tier: integration) [covers: ITEM-2, ITEM-10, ITEM-14, ITEM-15] file: `src-app/server/tests/notification/inbox_test.rs` — asserts: run-now a prompt task (real chat pipeline, stub model) writes a notification that lands in the inbox linking the `conversation_id`.
- **TEST-19** (tier: integration) [covers: ITEM-14, ITEM-15] file: `src-app/server/tests/notification/inbox_test.rs` — asserts: inbox CRUD over REST — list (paged + unread-only), unread-count, mark-read, read-all, delete.
- **TEST-20** (tier: integration) [covers: ITEM-15] file: `src-app/server/tests/notification/inbox_test.rs` — asserts: owner-scope (cross-user 404) + 403/401 gating.
- **TEST-21** (tier: integration) [covers: ITEM-16] file: `src-app/server/tests/notification/sync_emit_test.rs` — asserts: a background firing's notification fans a `notification` create to the OWNER only; a second user sees silence (cross-user isolation).

## Build / schema / codegen gate (compile-time verified)

- **TEST-22** (tier: integration) [covers: ITEM-5, ITEM-19, ITEM-20] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: `cargo test --workspace` compiles with `croner` (ITEM-19) and against the migrated schema — the `workflow_runs.invocation_source` CHECK now admits `'scheduled'` (migration 137 applies; ITEM-5) — and the `types_ts_parity` golden passes after regen (ITEM-20). A green build IS the assertion for these mechanically-verified items.
- **TEST-23** (tier: integration) [covers: ITEM-3, ITEM-17] file: `src-app/server/tests/scheduler/sync_emit_test.rs` — asserts: the admin `notification_retention_days` setting round-trips (GET/PUT persist); the deletion loop itself (`notification/prune.rs`) is a verbatim mirror of the retention-tested `mcp/tool_calls/prune.rs` (`tests/mcp/tool_call_history_test.rs`).

## E2E (`ui/tests/e2e/`, Playwright)

- **TEST-24** (tier: e2e) [covers: ITEM-21, ITEM-22, ITEM-23] file: `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — asserts: user opens `/scheduled-tasks` (empty state), creates a task via the drawer (name/prompt/model), and sees it listed.
- **TEST-25** (tier: e2e) [covers: ITEM-24] file: `src-app/ui/tests/e2e/14-scheduler/admin-settings.spec.ts` — asserts: an admin edits the quota + retention on `/settings/scheduler`, saves, and the values persist.
- **TEST-26** (tier: e2e) [covers: ITEM-25, ITEM-26] file: `src-app/ui/tests/e2e/15-notifications/inbox.spec.ts` — asserts: a notification renders in `/notifications` and mark-read clears its unread state.
- **TEST-27** (tier: e2e) [covers: ITEM-35, ITEM-37] file: `src-app/ui/tests/e2e/14-scheduler/dry-run.spec.ts` — asserts: in the create drawer, clicking **Test** runs a dry-run and renders the result inline WITHOUT saving; the change-detection ("only when something changed") toggle flips.
- **TEST-28** (tier: e2e) [covers: ITEM-33] file: `src-app/ui/tests/e2e/14-scheduler/paused-and-runs.spec.ts` — asserts: a paused task shows a "Paused" badge with its reason and its "Runs" section lists past firings with statuses.

## Coverage note

- ITEM-32 (workflow continue-in-chat) is DESCOPED (DRIFT-2.1) and carries no test.
- ITEM-5 / ITEM-19 / ITEM-20 are compile-time / golden-verified (TEST-22) — a
  green `cargo test --workspace` + the parity golden are their assertions.
- ITEM-17's deletion loop shares code with the retention-tested `mcp/tool_calls`
  prune (TEST-23 note); its admin config round-trip is asserted directly.

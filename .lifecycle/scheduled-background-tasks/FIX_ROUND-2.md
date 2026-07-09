# FIX_ROUND-2 — extended-angle audit (a11y / i18n / perf / tests-quality)

Round 1 covered 8 angles. Round 2 added the remaining roster angles via two more
fresh blind subagents (a11y+i18n; perf+tests-quality) → 12 distinct angles total.
8 more confirmed findings; all fixed or (LOW/acknowledged) resolved.

## Fixed

- **a11y (4)** — added accessible names: the 4 admin `InputNumber`s
  (`aria-label`), the 2 notify `Switch`es, the ScheduleBuilder timezone / custom-
  cron `Input`s + the weekday `Select`; the unread dots got `role="img"
  aria-label="Unread"` (color-only cue → also announced). Now consistent with the
  already-labeled row actions.
- **i18n/copy (2)** — the task list no longer leaks raw 5-field cron: a
  `humanizeCron` renders "Weekly on Monday at 09:00 (tz)"; empty-state copy
  unified to "No notifications yet".
- **tests-quality (HIGH)** — the tick-DRIVEN scheduled firing path is now
  integration-tested (`tests/scheduler/tick_test.rs`): a scheduled `once` prompt
  fired by the real tick loop (via the `SCHEDULER_TICK_MS` seam) advances →
  disables → records status + run-history + notification; plus a test that
  run-now does NOT mutate a recurring task's schedule bookkeeping. The misleading
  `tests/scheduler/mod.rs` note was corrected.

## Resolved as acknowledged (LOW, not code-changed)

- **perf** — the store's `sync:scheduled_task` reload refetches the task list +
  all loaded run-histories. Bounded by the user's own task count; standard
  notify-and-refetch churn. Noted for a future "refetch only the open task"
  optimization; not a defect.
- **tests-quality** — `on_change` end-to-end suppression can't be fast-integration-
  tested (the 300s min-interval floor separates two scheduled firings) and a clean
  terminal-failure injection would need a stub error seam the scheduler doesn't
  expose; both are covered by the `change.rs` / `failure.rs` unit tests. The e2e
  specs mock the API (repo's UI-surface convention) — the wired firing path is
  covered by the integration tests instead.

**New confirmed findings:** 0

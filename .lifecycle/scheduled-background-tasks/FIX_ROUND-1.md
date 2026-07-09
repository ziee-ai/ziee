# FIX_ROUND-1 — blind-audit fixes

Three fresh blind subagents reviewed `git diff main...HEAD` (correctness;
security+concurrency; frontend). 13 confirmed findings (2 HIGH), all fixed.

## Backend

- **HIGH — workflow IDOR** (`dispatch.rs`, `dryrun.rs`, `handlers.rs`): added
  `workflow::repository::user_can_access(pool, user_id, workflow_id)` re-checks at
  create-time, fire-time (dispatch), and test-fire. A workflow the user can't run
  is now rejected/paused (404) everywhere.
- **auto-pause on terminal failure** (`tick.rs`): `fire_task` now pauses the task
  immediately with the failure class as `paused_reason` for TERMINAL classes
  (target_missing/auth/permission/validation); transient failures pause only at
  the consecutive-failure cap. Wires the previously-dead `failure` taxonomy.
- **starvation** (`tick.rs`): `run_once` advances `next_run_at` synchronously then
  `tokio::spawn`s the dispatch, so a slow/hung task no longer blocks the batch.
- **run-now no bookkeeping mutation** (`tick.rs`): `record_outcome` is now gated on
  `trigger != "run_now"` — a manual run no longer touches the failure counter,
  change-detection signature, or auto-pause.
- **min-interval multi-gap** (`schedule.rs`): `validate_schedule` walks up to 24
  consecutive occurrences and rejects the minimum gap below the floor (catches
  uneven multi-time crons regardless of where `now` lands).
- **409 → transient** (`failure.rs`): a bound-conversation generation collision is
  retryable, not a hard failure.
- **assistant ownership** (`handlers.rs`, `dispatch.rs`, `dryrun.rs`): `assistant_id`
  is validated via `Repos.assistant.get_for_user` at create/dispatch/test-fire.
- **notify_mode/notify_on validation** (`handlers.rs`): 400 instead of a DB-CHECK 500.
- **misleading claim comment** (`repository.rs`): corrected to describe the actual
  single-process advance-before-spawn model (see DRIFT-1.3).

## Frontend

- **HIGH — ScheduleBuilder preset lock** (`ScheduleBuilder.tsx`): preset is now
  LOCAL state (not re-derived from cron each render), and `*` fields are sanitized
  to real defaults — switching Daily↔Weekly↔Monthly↔Custom works.
- **once run_at UTC/local drift** (`ScheduleBuilder.tsx`): the datetime-local input
  now converts stored-UTC↔local-wall-clock correctly (no offset drift on edit).
- **once run_at validation + empty-date crash** (`ScheduledTaskFormDrawer.tsx` +
  `ScheduleBuilder.tsx`): validate rejects an empty `run_at`; the onChange guards
  an empty/invalid date.
- **bell popover coupling** (`NotificationsPage.tsx`): leaving the inbox resets the
  shared store to page-1/all so the sidebar bell shows the latest, not the
  inbox's paginated slice.

**New confirmed findings:** 0

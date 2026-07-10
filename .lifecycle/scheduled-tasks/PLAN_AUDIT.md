# PLAN_AUDIT — scheduled-tasks

Audit of PLAN.md against the actual codebase (read pre-implementation).

## Breakage risk
- **ITEM-1/3/4 (drawer rewrite):** the drawer is self-contained (`ScheduledTaskFormDrawer.tsx`), driven by `Stores.SchedulerDrawer` + `Stores.ScheduledTasks`. The request body shape (`CreateScheduledTask` with `assistant_id`/`workflow_id`/`model_id`) is UNCHANGED — only the input widgets change — so the store/API/backend contract is untouched. Existing e2e specs in `14-scheduler/` select by `data-testid` (`task-form-assistant`, `task-form-workflow`, `task-form-model`); swapping `<Input>` for a Combobox/Select changes the interaction, so the create-flow spec MUST be updated (budgeted in TESTS). Risk: contained to the scheduler UI + its e2e.
- **ITEM-2 (timezone auto-detect in ScheduleBuilder):** removes the tz input; `value.timezone` is set from `browserTz()` automatically and still sent on the body. `ScheduleBuilder` is used only by the drawer — no other consumer. Existing e2e that typed into `schedule-timezone` must be updated to assert the read-only detected zone instead.
- **ITEM-12 (multi-day weekly):** changes the weekly control from a single-day `<Select>` to a multi-toggle; the emitted cron gains a comma dow. Server accepts any valid 5-field cron (`schedule.rs` parses via `cron`/`chrono_tz`), so no backend change. Risk: `humanizeCron` must handle the comma list or it degrades to `Cron: <expr>` (still correct, just less pretty) — covered by a unit test.
- **ITEM-5/6 (create/update validation):** ADDS pre-insert checks that currently don't exist → turns today's 500 (bad model FK) / silent-auto-pause into a clean 4xx. This is stricter: a client that today succeeds with an inaccessible model will now 403 at create. That is the intended fix and matches create-time assistant/workflow gating already present. No existing valid caller breaks (a valid model still passes).
- **ITEM-7 (pre-emptive pause):** changes dispatch/tick behavior for the NULL-referent edge only; the happy path (referent present) is unchanged. Must not pause a legitimate first-run prompt task (bound_conversation_id NULL + last_status NULL) — the DEC pins the exact discriminator (last_status non-NULL). Covered by a dispatch test.
- **ITEM-8 (run prune):** additive boot loop; deletes only rows older than retention. Mirrors notification prune; no read-path change.
- **ITEM-9 (transient retry):** changes a transient failure from "count toward cap immediately" to "retry N then count". Bounded attempts prevent a hang; the failure-cap semantics for non-transient classes are unchanged.
- **ITEM-10 (`completed` reason):** `paused_reason='completed'` is a NEW string value the frontend must recognize; an old frontend would render it as a generic paused badge (graceful). `is_active()` already returns false for any non-NULL paused_reason, so scheduling logic is unaffected.
- **ITEM-11 (log notif errors):** pure observability; no behavior change.

## Pattern conformance
- **ITEM-1** — mirrors `AssistantSelector.tsx` (Combobox over `Stores.AssistantPicker`) + `WorkflowRunDialog.tsx`/`ModelSelector.tsx` (grouped model `<Select>` from `Stores.ModelPicker`). Conforms.
- **ITEM-3** — `FormField` is the mandated form primitive (DESIGN_SYSTEM.md "Field-not-raw-flex-gap"); `ProjectFormDrawer.tsx` is the exemplar. Conforms; the current hand-rolled `<label>` blocks are the divergence being corrected.
- **ITEM-4** — reuses `parseWorkflowIr` (exported, `workflow/components/workflowIr.ts:32`) exactly as `WorkflowRunDialog` does. Conforms.
- **ITEM-5/6** — mirrors `workflow/runner.rs` `user_has_access_to_provider` for model access and the existing `scheduler/handlers.rs:104` assistant-ownership check. Conforms.
- **ITEM-8** — mirrors `notification/prune.rs` boot loop. Conforms.
- **ITEM-9** — reuses in-module `failure.rs` helpers (currently test-only). Conforms.
- **ITEM-2** — auto-detect via `Intl.DateTimeFormat().resolvedOptions().timeZone`, the same API the drawer's `browserTz()` already uses; read-only helper text is a plain `<Text>`. Conforms.
- **ITEM-12** — multi-day toggle uses the kit multi-select toggle-group primitive over Sun–Sat; cron dow becomes a sorted comma list. No exact existing multi-day picker to mirror, but the toggle-group primitive is standard kit usage.

## Migration collisions
None. This branch adds NO migration (BASE.md). New `paused_reason` values (`completed`,
`conversation_deleted`) require no schema change (free TEXT). Run-history retention reuses the
existing `scheduler_admin_settings.notification_retention_days` column. Highest main migration
is 145; unaffected.

## OpenAPI regen
Not required. No request/response type changes (all three picker fields already exist on
`CreateScheduledTask`; `completed` reuses `paused_reason`). `openapi.json` /
`api-client/types.ts` are untouched → the diff stays cleanly split (backend fixes + UI fixes),
and merge-gate C3 is a no-op.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — reuses AssistantSelector/ModelSelector/Workflow-list patterns; body shape unchanged; e2e update budgeted.
- **ITEM-2** — verdict: PASS — removing the timezone input and auto-detecting from `browserTz()` (already computed in the drawer) shrinks the form; `timezone` is still sent on the body (unchanged contract), just derived automatically. Read-only helper text keeps it transparent.
- **ITEM-3** — verdict: PASS — adopts the mandated `FormField` layout per ProjectFormDrawer; corrects the FB-1 divergence.
- **ITEM-4** — verdict: PASS — reuses exported `parseWorkflowIr`; JSON fallback preserves current capability.
- **ITEM-5** — verdict: PASS — adds the create-time model validation the migration comment already promises; mirrors workflow access check. Turns a 500 into a 4xx (intended).
- **ITEM-6** — verdict: CONCERN — quota-on-re-enable needs a repository count that excludes the task being updated (avoid off-by-one); pin the exact count predicate in DECISIONS and test both re-enable-under-cap and re-enable-over-cap.
- **ITEM-7** — verdict: CONCERN — the "conversation deleted vs first run" discriminator (bound_conversation_id NULL) is ambiguous without a signal; resolved by DEC (use `last_status IS NOT NULL`). Must have a dispatch test proving a genuine first-run is NOT paused.
- **ITEM-8** — verdict: PASS — additive prune mirroring notification/prune.rs; reuses existing retention setting.
- **ITEM-9** — verdict: CONCERN — retry attempts/backoff is an operational tunable → needs the configurable-vs-fixed DEC (Phase 4). Bounded attempts required so a persistent transient error can't loop forever within one firing.
- **ITEM-10** — verdict: PASS — `paused_reason='completed'` is backward-compatible (old UI shows generic paused badge); is_active() unaffected.
- **ITEM-11** — verdict: PASS — pure logging; no behavior change.
- **ITEM-12** — verdict: CONCERN — multi-day cron must round-trip: emit a SORTED comma dow (`1,3,5`), and `humanizeCron` (currently only matches `/^\d$/`) must be extended to parse a comma list AND still fall back to `Cron: <expr>` for arbitrary expressions. The min-interval floor in `schedule.rs` already samples a 24-occurrence window so multi-day crons are validated server-side (no backend change). Pin the encoding + parse-back in DECISIONS + a unit test both directions.

No `BLOCKED` verdicts. The three `CONCERN`s (ITEM-6/7/9) are all resolved by explicit DECISIONS
(Phase 4) + targeted tests (Phase 3), not plan changes.

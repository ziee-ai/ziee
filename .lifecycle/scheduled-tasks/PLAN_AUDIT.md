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
One migration added: `153` (`scheduled_task_unattended_tools`), next free vs main current max 152 — no
collision at cut time; merge-gate C2 re-checks if main advances. The bug-fix/FB items add no
schema (new `paused_reason` values are free TEXT; run-prune reuses `notification_retention_days`).

## OpenAPI regen
Required for ITEM-15/17 only: `allowed_unattended_tools` on `Create/UpdateScheduledTask` +
`skipped_tools` on `ScheduledTaskRun`. Run `just openapi-regen` (BOTH `ui/` + `desktop/ui/`);
merge-gate C3 re-checks parity for both. The `unattended` chat-request flag is threaded
in-process — keep it OFF the public OpenAPI request type where possible to keep the regen delta
minimal and avoid making a backend-internal flag part of the client contract.

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
- **ITEM-13** — verdict: CONCERN — threading an `unattended` flag through `SendMessageRequest`→`StreamContext.metadata`→`mcp.rs after_llm_call` touches the SHARED chat pipeline. Must be ADDITIVE + default-false so interactive chat is byte-for-byte unchanged (B3: never route the shared harness around this feature). The deny-not-pause branch must still persist approval-exempt built-in results (mirror the existing pause block) so the turn stays protocol-valid. High blast radius → needs the fresh-agent audit (phase 6) to focus here + a test proving interactive approval is unchanged.
- **ITEM-14** — verdict: PASS — reuses `is_builtin_server_id`/`is_side_effect_tool`/`mcp_config` (all existing); constrains, never widens, the attach set. The empty allow-list default (read-only-only) is the safe floor.
- **ITEM-15** — verdict: CONCERN — migration 153 + new request fields → OpenAPI regen (both workspaces). Allow-list validation must resolve the user's accessible servers at create/update (reuse `get_all_accessible_config`) and reject entries the user can't access — else the field could be a privilege-escalation vector. Default `'[]'` keeps existing rows valid.
- **ITEM-16** — verdict: CONCERN — reuse the existing MCP server/tool selection surface rather than inventing a picker (FB-1). Needs a read of how MCP server selection is presented elsewhere (conversation mcp settings / project mcp defaults) to mirror it. e2e budgeted.
- **ITEM-17** — verdict: PASS — additive run column + notification text; mirrors existing run-status reporting. Backward-compatible (`'[]'`).
- **ITEM-18** — verdict: PASS — reuses `parseWorkflowIr` (ITEM-4) to detect `elicit` steps at create; disabled-servers pass-through mirrors the existing conversation-scoped filter, just sourced from the user's defaults for scheduled runs.

No `BLOCKED` verdicts. The three `CONCERN`s (ITEM-6/7/9) are all resolved by explicit DECISIONS
(Phase 4) + targeted tests (Phase 3), not plan changes.

---

# ROUND 2 AUDIT — Follow-up & Series (ITEM-40..48)

## Breakage risk (Round 2)
- **ITEM-41 + ITEM-46 (run-list pagination) — COORDINATED CHANGE.** `list_task_runs` today returns `Vec<ScheduledTaskRun>`; moving to a paged envelope (mirroring `mcp/tool_calls`) is a **breaking response-shape change**. The ONLY consumer is the scheduler store (`ScheduledTasks.store.ts:101` `listRuns` → `runsByTask: Record<string, ScheduledTaskRun[]>`), so blast radius is contained, but the store + the `sync:scheduled_task` reload loop (`store.ts:123` refetches every loaded task) MUST adapt in the SAME change or the panel breaks. OpenAPI regen (both workspaces) required. Mitigation: land backend + store + UI together; keep the envelope newest-first so page 1 is the default view.
- **ITEM-42 (assistant-turn seed) — isolated to `continue_chat.rs`.** Changes the seeded message role `user`→`assistant` (DEC-23) and, for workflow runs, adds output + artifacts. The current empty-workflow-seed is the defect being fixed. `create_message(branch_id, "assistant", model_id)` + `append_content` is exactly the shape the current user-seed uses, so no chat-core change. `file/repository.rs:216` (`SELECT id FROM files WHERE workflow_run_id = $1`) exists for the cascade — reuse it to resolve artifacts, but re-scope the resolved files to `task.user_id` ownership before attaching (it is run-id-scoped, not user-scoped). No shared-pipeline edit → no B3 concern.
- **ITEM-40 (run preview/change columns) — additive + nullable.** Two `NULL`-default columns on `scheduled_task_runs`; written only in `finalize_success`, which already holds `raw.text` + the `change::diff` outcome. Existing rows stay valid; an old client ignores the new fields.
- **ITEM-43/47 (series follow-up) — additive.** New route + store action; no existing path changes. Depends on ITEM-40's fields being populated (ordering note below).
- **ITEM-44/45/48 (row timeline / Open-thread / responsive) — presentation.** Read new run fields + the already-exposed `bound_conversation_id`; no contract change. Open-thread navigation reuses the existing `continueRun`/`window.location` pattern in the page.

## Pattern conformance (Round 2)
- **ITEM-40/41** — mirror `mcp/tool_calls` (`repository.rs` page/per_page clamp+offset, `models.rs` page envelope, `handlers.rs` `default_per_page`). Conforms.
- **ITEM-42** — reuses `summarize_workflow_output` (already `pub` in `dispatch.rs`), `file/repository.rs` workflow-run file query, and the `chat/extensions/file/processor.rs::process_file_blocks` ContentBlock path. Conforms.
- **ITEM-44** — extract change→badge + preview mappers into `runTimeline.ts` (pure, unit-testable), semantic color tokens per DESIGN_SYSTEM.md. Conforms.
- **ITEM-46** — reuses `components/common/ListPagination.tsx`. Conforms.
- **ITEM-45/47** — reuse the store-action + router-nav idiom already in `ScheduledTasksPage`. Conforms.

## Migration collisions (Round 2)
One migration added: `155` (`scheduled_task_run_result_preview`), next free vs CURRENT main max `154`
(`add_voice_streaming_settings`). No collision; merge-gate C2 re-checks if main advances before merge.

## OpenAPI regen (Round 2)
Required — `ScheduledTaskRun` gains `result_preview` + `change_summary_json` (ITEM-40), the run-list
response becomes a paged envelope (ITEM-41), and `continue-series` adds a route + response (ITEM-43).
`just openapi-regen` (BOTH `ui/` + `desktop/ui/`); merge-gate C3 re-checks parity for both.

## Ordering dependency
ITEM-40 (run fields) must land before/with ITEM-43/44/47, which read `result_preview`/`change_summary_json`.
ITEM-41 (envelope) and ITEM-46 (ListPagination) must land together (shared response shape).

## Per-item verdicts (Round 2)
- **ITEM-40** — verdict: PASS — additive nullable columns written in `finalize_success` (which already has the text + `change::diff` outcome); mirrors run-status reporting; backward-compatible.
- **ITEM-41** — verdict: CONCERN — breaking run-list response-shape change; contained (sole consumer is the scheduler store) but MUST land with ITEM-46 + OpenAPI regen. Resolved by coordinated implementation, not a plan change.
- **ITEM-42** — verdict: CONCERN — role change + artifact resolution; reuses existing `create_message`/`summarize_workflow_output`/file-by-run query, but the resolved artifacts must be re-scoped to `task.user_id` before attaching (the run-id query is not user-scoped). Resolved by the ownership re-scope + TEST-44/45.
- **ITEM-43** — verdict: PASS — additive owner-scoped route reusing the continue scaffolding; 404 on a foreign task is the A9 deny path (TEST-46). Depends on ITEM-40.
- **ITEM-44** — verdict: PASS — reads ITEM-40 fields; pure mappers in `runTimeline.ts`; semantic tokens.
- **ITEM-45** — verdict: PASS — `bound_conversation_id` already on the task model + response; navigation reuses the existing pattern; the prompt/workflow action resolver is pure + unit-tested.
- **ITEM-46** — verdict: CONCERN — paired with ITEM-41 (paged store shape + sync-reload loop must refetch the current page, not all). Resolved by landing with ITEM-41; reuses `ListPagination`.
- **ITEM-47** — verdict: PASS — new `continueSeries` store action → the ITEM-43 endpoint → navigate; chooser is a kit select. Depends on ITEM-43.
- **ITEM-48** — verdict: PASS — presentation only; gallery narrow-viewport + new-state coverage enforced by `gate:ui`/`check:state-matrix`.

No `BLOCKED` verdicts. The three Round-2 `CONCERN`s (ITEM-41/42/46) resolve by coordinated
implementation + the ownership re-scope + explicit DECISIONS (DEC-20/23) + tests — not plan changes.

# PLAN — scheduled-tasks (improve the merged Scheduled Tasks feature)

Improve the already-merged `scheduler` feature: fix concrete backend bugs found by a
code survey, and fix the two seeded human-feedback items (raw-ID inputs → pickers;
reuse existing drawer/form layouts). No new migration, no OpenAPI regen (all fixes use
existing columns/types; `paused_reason` is free TEXT).

## Items

### Frontend — human-feedback driven (FB-1, FB-2)
- **ITEM-1**: Replace the three raw-ID text `<Input>`s in `ScheduledTaskFormDrawer` with proper pickers — Assistant (searchable Combobox from the assistant list), Workflow (searchable Combobox from the workflow list), Model (grouped `<Select>` from `Stores.ModelPicker` with an empty-state CTA) — reusing the existing selector components/stores. (FB-2)
- **ITEM-2**: REMOVE the timezone input entirely — auto-detect the timezone from the client (`Intl.DateTimeFormat().resolvedOptions().timeZone`) and use it automatically; surface it only as read-only helper text ("Times are in your timezone: <zone>"). No timezone field for the user to fill. (FB-3 — revises the earlier "timezone picker" idea; the user does not want to input/pick a timezone at all)
- **ITEM-3**: Rebuild the drawer form on the design-system `FormField` layout (mirror `ProjectFormDrawer` / `WorkflowRunDialog`) instead of hand-rolled `<label><Text/><Input/></label>` blocks. (FB-1)
- **ITEM-4**: For a workflow target, render a FORM generated from the selected workflow's declared inputs via the existing `parseWorkflowIr` (one typed field per input, reusing `WorkflowRunDialog`'s typed-input rendering), with the raw JSON editor kept ONLY as a last-resort fallback when the workflow declares no inputs. Replaces the always-raw "Inputs (JSON)" textarea. (FB-1 + FB-5 — "inputs based on the workflow, not just json; the UX is horrible" + mitigates backend Bug 10)
- **ITEM-12**: In `ScheduleBuilder` weekly mode, let the user select MULTIPLE days of the week (a Mon–Sun multi-toggle), emitting the cron day-of-week as a comma list (e.g. `0 9 * * 1,3,5`); update `humanizeCron` (page + any summary) to render multi-day recurrences ("Weekly on Mon, Wed, Fri at 09:00"). (FB-4)

### Backend — bug fixes
- **ITEM-5**: Validate `model_id` (existence + the user's provider access) in `create_task` and return a clean 404/403 BEFORE the insert — today a bad id hits the `llm_models` FK → HTTP 500, and an inaccessible-but-existing id creates a task that silently auto-pauses on first fire. Mirror `workflow/runner.rs` `user_has_access_to_provider`. (Bug 1, HIGH)
- **ITEM-6**: In `update_task`, re-validate `assistant_id` ownership and `model_id` access when they change, and re-enforce the active-task quota when a task is re-enabled (`enabled: true`) — today update re-checks nothing, allowing a foreign assistant / inaccessible model to be written and the per-user quota to be bypassed via disable→create→re-enable. (Bug 2, HIGH)
- **ITEM-7**: Pre-emptively pause a task whose referenced entity was deleted, instead of firing wastefully: a prompt task whose bound conversation was deleted (`bound_conversation_id` went NULL after it had already run) pauses with `paused_reason='conversation_deleted'` rather than spawning a fresh throwaway conversation each tick; a workflow/prompt task whose `workflow_id`/`assistant_id` went NULL pauses `target_missing` at claim time rather than firing → failing → emitting a spurious failure notification. (Bugs 3 + 11, MEDIUM)
- **ITEM-8**: Prune `scheduled_task_runs` by retention in the existing boot prune loop (reuse `notification_retention_days`), fulfilling migration 144's documented-but-unimplemented intent — today run history grows unbounded. (Bug 4, MEDIUM)
- **ITEM-9**: Wire the existing (test-only) `failure::is_retryable` / `retry_backoff_ms` into `dispatch` so a `transient` failure is retried a bounded number of times with backoff WITHIN the firing before it counts toward the consecutive-failure cap — today a transient blip counts straight toward auto-pause and `retry_backoff_ms` is dead code. (Bug 5, MEDIUM)
- **ITEM-10**: Mark a spent `once` task as completed (`paused_reason='completed'`) instead of leaving `enabled=false, paused_reason=NULL` (indistinguishable from a user pause), and surface it as "Completed" in the UI; keep benign edits from 400-ing purely because a completed once-task's `run_at` is in the past. (Bug 8, LOW)
- **ITEM-11**: Log (structured `warn`) notification-creation failures in `finalize_success`/`finalize_failure` instead of silently `.ok()`-dropping them, so a broken task that also fails to notify is at least diagnosable. (Bug 9, LOW)

### Unattended tool-call + approval policy (human decision DEC-17: "safe default + per-task allow-list")
- **ITEM-13**: Thread an `unattended` signal (source=`scheduled`) from `dispatch_prompt` through `SendMessageRequest` → `StreamContext.metadata` → the MCP `after_llm_call` approval decision. In unattended mode, a tool that would require approval and is NOT allow-listed gets a synthetic denial `tool_result` (turn continues to a coherent answer) instead of writing orphaned `pending` approval rows + truncating. No pending-approval rows created for unattended runs. (DEC-17.1/2 — fixes the silent-skip-reports-success gap)
- **ITEM-14**: Constrain the unattended run's tool surface: attach only built-in read-only servers (`is_builtin_server_id`) + the task's allow-listed servers via `mcp_config`; deny any side-effecting tool (`is_side_effect_tool`) not in the allow-list, incl. Always-mode pre-execution. (DEC-17.3 — closes the unattended-side-effects risk)
- **ITEM-15**: Per-task allow-list (migration 146): `scheduled_tasks.allowed_unattended_tools JSONB`; add to `CreateScheduledTask`/`UpdateScheduledTask` with validation that every entry ⊆ the user's currently-accessible servers; at fire time populate the run's approval-bypass set from it. (DEC-17.4, DEC-19)
- **ITEM-16**: Drawer UI — a "Tools this task may use unattended" multi-select picker (reuse the MCP server/tool selection surface), in the `FormField` layout; empty = read-only-only (safe default), with a one-line explainer. (DEC-17.4 frontend)
- **ITEM-17**: Honest reporting — record `scheduled_task_runs.skipped_tools` (migration 146) and surface it in the completion notification ("Ran — N tools skipped (not permitted unattended)") + the Runs UI, so a truncated result is never reported as a clean success. (DEC-17.5)
- **ITEM-18**: Workflow target under the policy — reject at create/update a workflow whose IR contains an `elicit` step ("needs interactive input; can't be scheduled") instead of the 15-min-timeout false-fail; and apply the user's default `disabled_servers` to scheduled workflow tool-steps (today skipped because the filter is conversation-scoped). (DEC-18)

### Deferred (surveyed, intentionally out of scope this iteration — see PLAN_AUDIT)
Bug 6 (run-now/test-fire rate-limit + concurrent-fire guard — needs a rate-policy DEC),
Bug 7 (claim→mark_fired atomicity — architectural, single-process safe today),
Bug 10 (server-side `inputs_json` schema validation — partially mitigated by ITEM-4's typed inputs).

## Files to touch

### Frontend
- `src-app/ui/src/modules/scheduler/components/ScheduledTaskFormDrawer.tsx` — pickers, FormField layout, typed workflow inputs (ITEM-1/3/4)
- `src-app/ui/src/modules/scheduler/components/ScheduleBuilder.tsx` — timezone Combobox (ITEM-2)
- `src-app/ui/src/modules/scheduler/components/TaskTargetPickers.tsx` — NEW: small form-bound Assistant/Workflow/Model select wrappers (ITEM-1) — only if the existing selectors can't be used controlled; else reuse directly
- `src-app/ui/src/modules/scheduler/pages/ScheduledTasksPage.tsx` — "Completed" badge for spent once-tasks (ITEM-10 UI half)
- `src-app/ui/src/modules/scheduler/stores/ScheduledTasks.store.ts` — trigger picker-list loads on drawer open if needed (ITEM-1)

### Backend
- `src-app/server/src/modules/scheduler/handlers.rs` — model_id validation (create), update re-validation + quota (ITEM-5/6)
- `src-app/server/src/modules/scheduler/repository.rs` — quota re-check on re-enable, run-history prune query, mark_fired `completed` reason (ITEM-6/8/10)
- `src-app/server/src/modules/scheduler/dispatch.rs` — transient retry loop, `conversation_deleted` detection, notif-error logging (ITEM-7/9/11)
- `src-app/server/src/modules/scheduler/tick.rs` — pre-emptive referent-missing pause at claim (ITEM-7)
- `src-app/server/src/modules/scheduler/failure.rs` — expose/adjust retry knobs if needed (ITEM-9)
- `src-app/server/src/modules/scheduler/mod.rs` — spawn/extend the boot prune loop for run history (ITEM-8)
- `src-app/server/migrations/00000000000146_scheduled_task_unattended_tools.sql` — NEW: `allowed_unattended_tools` + `skipped_tools` columns (ITEM-15/17)
- `src-app/server/src/modules/scheduler/models.rs` + `handlers.rs` + `repository.rs` — allow-list field, validation, persistence, run skipped-tools (ITEM-15/17)
- `src-app/server/src/modules/chat/**` (`SendMessageRequest`, `StreamContext`/metadata) — carry the `unattended` signal (ITEM-13) — MINIMAL, additive optional field
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (+ `approval/`) — branch the approval decision on `unattended` + allow-list; deny-not-pause; constrain servers (ITEM-13/14)
- `src-app/server/src/modules/workflow/dispatch.rs` (+ scheduler dispatch) — scheduled disabled-servers on tool-steps; elicit-step detection for create-time reject (ITEM-18)
- OpenAPI: `just openapi-regen` (BOTH ui + desktop) after the new request/response fields (ITEM-15/17)

### Tests
- `src-app/server/tests/scheduler/{crud_test,dispatch_behavior_test,tick_test}.rs` + a new `validation_test.rs` / `prune_test.rs`
- `src-app/ui/tests/e2e/14-scheduler/scheduled-tasks.spec.ts` — picker-based create flow

## Patterns to follow
- **Drawer form layout** → `src-app/ui/src/modules/projects/components/ProjectFormDrawer.tsx` (`FormField`) and `src-app/ui/src/modules/workflow/components/WorkflowRunDialog.tsx` (model `<Select>` + `parseWorkflowIr` typed inputs). Do NOT invent a new form layout ([[feedback_match_existing_patterns]], FB-1).
- **Assistant picker** → `src-app/ui/src/modules/assistant/chat-extension/components/AssistantSelector.tsx` (Combobox over `Stores.AssistantPicker.availableAssistants`).
- **Model picker** → `src-app/ui/src/modules/chat/components/ModelSelector.tsx` + `Stores.ModelPicker` grouped provider→model options.
- **Workflow list** → `Stores.Workflow.workflows` (`ApiClient.Workflow.list`), same shape as the assistant list.
- **Backend create/update access validation** → `src-app/server/src/modules/project/handlers.rs` ownership checks + `src-app/server/src/modules/workflow/runner.rs` `user_has_access_to_provider` (model access); assistant ownership already at `scheduler/handlers.rs:104`.
- **Boot prune loop** → `src-app/server/src/modules/notification/prune.rs`.
- **Transient retry** → reuse `src-app/server/src/modules/scheduler/failure.rs` `is_retryable` / `retry_backoff_ms`.
- **Multi-day-of-week toggle (ITEM-12)** → a kit multi-select toggle group (e.g. the `ToggleGroup`/multi-`Segmented` primitive already used in the design system) over Sun–Sat, emitting a sorted comma cron dow. Timezone auto-detect (ITEM-2) reuses the drawer's existing `browserTz()` helper.

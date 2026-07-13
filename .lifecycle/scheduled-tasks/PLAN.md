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
- **ITEM-15**: Per-task allow-list (migration 153): `scheduled_tasks.allowed_unattended_tools JSONB`; add to `CreateScheduledTask`/`UpdateScheduledTask` with validation that every entry ⊆ the user's currently-accessible servers; at fire time populate the run's approval-bypass set from it. (DEC-17.4, DEC-19)
- **ITEM-16**: Drawer UI — a "Tools this task may use unattended" multi-select picker (reuse the MCP server/tool selection surface), in the `FormField` layout; empty = read-only-only (safe default), with a one-line explainer. (DEC-17.4 frontend)
- **ITEM-17**: Honest reporting — record `scheduled_task_runs.skipped_tools` (migration 153) and surface it in the completion notification ("Ran — N tools skipped (not permitted unattended)") + the Runs UI, so a truncated result is never reported as a clean success. (DEC-17.5)
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
- `src-app/server/migrations/00000000000153_scheduled_task_unattended_tools.sql` — NEW: `allowed_unattended_tools` + `skipped_tools` columns (ITEM-15/17)
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

---

# ROUND 2 — Follow-up & Series (FB-7 / FB-8)

Implements the human-approved `UX_DESIGN_followup_series.md` (all four capabilities:
ongoing thread, inline results + what-changed timeline, series-level follow-up, real-
content fidelity). Locked decisions: DEC-20 fixed UX constants · DEC-21 Open-thread +
keep fork · DEC-22 series chooser {5,10,all-loaded}/default 5 · DEC-23 synthesized
assistant-turn seed. No new permission is introduced — the new `continue-series`
route reuses the existing scheduler-use permission and migration 155 grants nothing
(it only adds two nullable run columns). A10 is therefore N/A this round.

## Round 2 — Items

### Backend
- **ITEM-40**: Migration `155` — add `scheduled_task_runs.result_preview TEXT NULL` (capped `PREVIEW_CHARS≈280`) + `change_summary_json JSONB NULL` (`{changed:bool, new_count:int, new_items?:[…]}`); populate both in `dispatch::finalize_success` from the result text + the existing `change::diff` outcome; expose both on the `ScheduledTaskRun` model (+ OpenAPI). Powers the inline timeline (J1) + the series seed's deltas. FULL result for a single-run follow-up is pulled LIVE at continue time, not duplicated here. (DEC-20)
- **ITEM-41**: Paginate run history — `list_task_runs` gains `page`/`per_page` (default `RUNS_PAGE_SIZE=10`) mirroring the existing paginated list handlers; `repository::list_runs_for_task` returns a page + total count; response becomes a paginated envelope (OpenAPI regen). Bounds the panel's initial load (scale/cardinality checklist). (DEC-20)
- **ITEM-42**: Real-content follow-up fidelity — rework `continue_run_in_chat` to seed a synthesized **assistant** message carrying the REAL result (DEC-23): prompt run → the actual last assistant text; **workflow** run → `summarize_workflow_output(final_output_json)` **+ the run's persisted artifacts** (files linked via `files.workflow_run_id`) attached as provider-routed file ContentBlocks on that assistant message. Fixes today's empty workflow seed (J4).
- **ITEM-43**: Series follow-up endpoint — `POST /scheduled-tasks/{id}/continue-series` (`limit`, server-clamped) aggregating the last N runs' `result_preview` + `change_summary_json` into ONE seeded assistant summary message, returning the new conversation id; owner-scoped (foreign task → 404). (J5, DEC-22)

### Frontend
- **ITEM-44**: Run-row timeline redesign — a what-changed badge (`NEW ×N` / `changed` / `no change` / `failed`, semantic tokens) + a one-line `result_preview`, click-to-expand the fuller result inline (+ failure message on a failed run). Read a result without forking (J1/J2).
- **ITEM-45**: "Open thread" affordance — task row/header primary action for a prompt task with `bound_conversation_id` → navigate to `/conversations/{id}` (disabled + tooltip before first fire); per-run prompt action = "Open thread" primary + existing fork as secondary "New side chat"; a workflow run keeps "Continue in chat" (now real-content). (DEC-21, J3)
- **ITEM-46**: Runs-panel bounding — replace flat render-all with `ListPagination` (page size 10, "Showing N of M"); the store's `loadRuns(taskId, page)` holds the paged slice + total. (scale/cardinality)
- **ITEM-47**: "Discuss recent runs ▾" control — a chooser {5, 10, all-loaded} (default 5) in the runs-panel header → calls `continue-series` → navigates to the seeded conversation. (DEC-22, J5)
- **ITEM-48**: Responsive + states — at ~390px the runs panel stacks and per-run actions collapse to an overflow menu, preview clamps to one line, no horizontal page scroll; never-fired ("Open thread" disabled) / failed (error on expand) / loading states; gallery narrow-viewport + new-state coverage.

## Round 2 — Files to touch (additive)
- `src-app/server/migrations/00000000000155_scheduled_task_run_result_preview.sql` — NEW (ITEM-40)
- `src-app/server/src/modules/scheduler/{models,repository,dispatch}.rs` — run preview/change columns + persist (ITEM-40); paged run query (ITEM-41)
- `src-app/server/src/modules/scheduler/{routes,handlers}.rs` — paged `list_task_runs` (ITEM-41); `continue-series` route+handler (ITEM-43)
- `src-app/server/src/modules/scheduler/continue_chat.rs` — assistant-turn seed + workflow output/artifacts (ITEM-42); series aggregation (ITEM-43)
- `src-app/ui/src/modules/scheduler/pages/ScheduledTasksPage.tsx` — run-row timeline, Open-thread, pagination, Discuss-recent-runs, responsive (ITEM-44..48)
- `src-app/ui/src/modules/scheduler/components/runTimeline.ts` — NEW: change-badge + preview mappers (ITEM-44)
- `src-app/ui/src/modules/scheduler/stores/ScheduledTasks.store.ts` — paged `loadRuns`, `continueSeries` action (ITEM-46/47)
- `src-app/ui/src/modules/scheduler/types.ts` — run fields (preview/change), paginated envelope
- OpenAPI: `just openapi-regen` (BOTH ui + desktop) after ITEM-40/41/43 (new run fields + paged envelope + continue-series response)

## Round 2 — Patterns to follow
- **Run preview + change persist** → mirror `dispatch::finalize_success`' existing `change::diff` use; cap via a named const like `NOTIF_BODY_CAP`.
- **Run pagination** → the existing paginated list handlers (e.g. `mcp/tool_calls` `GET …?page&per_page` + `ListPagination` on the client) — same envelope + idiom.
- **Assistant-turn seed + artifact attach** → the file→ContentBlock path in `chat/extensions/file/processor.rs` (`process_file_blocks`) for workflow artifacts; `continue_run_in_chat`'s existing conversation-create scaffolding.
- **Open thread / continue-series navigation** → the existing `continueRun` store action + `window.location`/router nav already in `ScheduledTasksPage`.
- **Change badges** → semantic color tokens (`text-…`, success/warning/destructive/muted), never raw hex (DESIGN_SYSTEM.md).

## Round 3 (FB-9 precedent audit) — Items

Delta scope from the FB-9 precedent audit: bring the top-level Scheduled Tasks
page/list/card + the form drawer into line with the closest sibling surfaces
(knowledge-base, projects, chat, settings). Approved-descoped items are marked.

- **ITEM-49**: `/scheduled-tasks` route gains `layout: AppLayoutDef` so the page renders inside the app shell (left sidebar + `<main id="main-content">`), matching every top-level nav route (chat ×5, projects ×3, knowledge-base ×2). Today it has NO `layout:` → renders bare (no sidebar, no header). (A1, FB-9 core)
- **ITEM-50**: Rewrite `ScheduledTasksPage` root to the top-level-page idiom — `useNativeScroll(true)` + `Stores.AppLayout.nativeScroll`, root `<div flex-col>` opening with `<HeaderBarContainer>` (`<Title level={4}>` "Scheduled Tasks" left + icon-only `Plus` "New task" `Button` right, gated `<Can permission={SchedulerUse}>`), then a scrollable body — replacing `SettingsPageContainer`. Mirrors `KnowledgeBasesListPage`. (B1/B2/B3)
- **ITEM-51**: Client-side Load-More paging for the task list — `PAGE_SIZE=12`, `visibleCount` state, `Showing N of M` counter (`Text type="secondary" aria-live="polite" role="status"`) + "Load More" button. Mirrors `KnowledgeBasesListPage`/`ProjectsListPage`. (C1, scale)
- **ITEM-52**: Extract the single-task row into a dedicated `ScheduledTaskCard.tsx` component (single-column retained per DEC-24), mirroring `KnowledgeBaseCard`/`ProjectCard` file structure. (C3, reuse)
- **ITEM-53**: Task title typography → `<Title level={5} className="!m-0 !font-normal !text-sm line-clamp-2">` (the canonical list-item title token), replacing `<Text className="truncate font-medium">`. (C4, precedent-fidelity)
- **ITEM-54**: Card actions → `outline size="icon"` buttons wrapped in `Tooltip`, hover/focus-revealed (`opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100`), pinned-visible while a `Confirm` is open; the enable/disable `Switch` stays always-visible (it is STATE, not an action). Mirrors `ProjectCard`. (C6, DEC-25)
- **ITEM-55**: Empty state → `CalendarClock size-16` icon + `<Title level={3} className="text-muted-foreground">No scheduled tasks yet` + secondary description + `<Can permission={SchedulerUse}>`-gated "New task" create button, replacing the bare `<Empty description>`. Mirrors `KnowledgeBasesListPage`/`ProjectsListPage`. (D1)
- **ITEM-56**: Drawer adopts `zodResolver(schema)` on `useForm` and wraps the target-kind `Segmented` + the `Schedule` block in labelled `FormField`s, matching `ProjectFormDrawer`'s form idiom. (E1/E2/E7, DEC-26)
- **ITEM-57**: The two notify toggles → the `Field`/`FieldContent`/`FieldTitle` horizontal pattern (matching the same module's `SchedulerAdminPage`) instead of bare `Flex`+`Text` rows. (E3)
- **ITEM-58**: `ScheduleBuilder` internal control rows (Run-at / Time / Day-of-month) → `Field`/`FieldTitle` labelled rows instead of hand-rolled `<label><Text/>…</label>`. (E4)
- **ITEM-59**: Narrow-viewport (~390px) fidelity of the reworked header + card action cluster (wrap/overflow, no horizontal page scroll), + gallery narrow-viewport coverage for the reworked page. (F1)
- **ITEM-60**: [DESCOPED] 2-column responsive card grid for the task list (C2) — human chose single-column expandable rows (DEC-24).
- **ITEM-61**: [DESCOPED] per-task detail page at `/scheduled-tasks/:id` (C5) — human directed this to a separate future lifecycle (DEC-27).

## Round 3 — Files to touch
- `src-app/ui/src/modules/scheduler/module.tsx` — `layout: AppLayoutDef` on `/scheduled-tasks` (ITEM-49)
- `src-app/ui/src/modules/scheduler/pages/ScheduledTasksPage.tsx` — top-level idiom rewrite: HeaderBarContainer + nativeScroll + Load-More + empty state; delegates the row to the new card (ITEM-50/51/55)
- `src-app/ui/src/modules/scheduler/components/ScheduledTaskCard.tsx` — NEW: extracted single-task card (ITEM-52/53/54)
- `src-app/ui/src/modules/scheduler/components/ScheduledTaskFormDrawer.tsx` — zodResolver + FormField wrapping + notify Field rows (ITEM-56/57)
- `src-app/ui/src/modules/scheduler/components/ScheduleBuilder.tsx` — Field-row labels (ITEM-58)
- Gallery: scheduler page gallery entry(s) incl. a narrow-viewport + empty state (ITEM-55/59)
- `src-app/ui/tests/e2e/14-scheduler/*` — new specs (see TESTS Round 3)

## Round 3 — Patterns to follow
- **Top-level page shell** → `KnowledgeBasesListPage` (the exact twin: `useNativeScroll(true)`, `HeaderBarContainer`, Load-More, `Empty`, `ErrorState`) — copy its structure verbatim, then keep the scheduler-specific expandable-runs body inside a single-column list.
- **Single-task card** → `KnowledgeBaseCard`/`ProjectCard` (title `!font-normal !text-sm` level 5, hover-revealed `outline size="icon"` + `Tooltip` actions, `Confirm` delete).
- **Header create action** → the icon-only `Plus` `Button size="icon"` gated by `<Can>` in `HeaderBarContainer`.
- **Form idiom** → `ProjectFormDrawer` (`useForm` + `zodResolver`, `FormField` per control) + the same module's `SchedulerAdminPage` `Field/FieldGroup` for the horizontal toggle/limit rows.

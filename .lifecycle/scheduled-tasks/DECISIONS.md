# DECISIONS — scheduled-tasks

All inputs the implementation needs, resolved up front so Phase 5 runs nonstop.

### DEC-1: How is "the user can access this model" checked (ITEM-5/6)?
**Resolution:** Reuse `workflow::runner`'s access path — `user_group_llm_provider::user_has_access_to_provider(user_id, provider_id_of(model))` after loading the model row. Missing model row → NotFound; provider not accessible → Forbidden.
**Basis:** codebase — this is exactly how `workflow/runner.rs` gates a run's model; keeps one access model.

### DEC-2: What HTTP status for an invalid model at create (ITEM-5)?
**Resolution:** non-existent `model_id` → 404; existing-but-inaccessible → 403. (Replaces today's raw FK-violation 500 and the silent create-then-auto-pause.)
**Basis:** convention — mirrors the create-time workflow-access (404/403) and assistant-ownership checks already in `scheduler/handlers.rs`.

### DEC-3: What does `update_task` re-validate, and when (ITEM-6)?
**Resolution:** Re-check `assistant_id` ownership and `model_id` access ONLY when the field is present in the update body (changed); re-check the active-task quota ONLY when the update sets `enabled: true`. Unchanged fields are not re-validated.
**Basis:** convention — matches create-time gating; avoids re-charging validation for no-op edits.

### DEC-4: Quota count predicate on re-enable (ITEM-6, resolves PLAN_AUDIT CONCERN)?
**Resolution:** Count the user's `enabled = true` tasks EXCLUDING the row being updated; reject when that count `>= max_active_tasks_per_user` (i.e. re-enabling would make it exceed). This avoids an off-by-one when the target row is already enabled.
**Basis:** codebase — mirrors `repository.rs` create-time quota count (which counts enabled tasks), with the `id <> $updating` exclusion added.

### DEC-5: "Bound conversation deleted" vs "first run" discriminator (ITEM-7, resolves CONCERN)?
**Resolution:** For a prompt task, `bound_conversation_id IS NULL AND last_status IS NOT NULL` ⇒ the conversation was deleted after a prior run ⇒ pause `conversation_deleted`. `bound_conversation_id IS NULL AND last_status IS NULL` ⇒ genuine first run ⇒ create the bound conversation as today.
**Basis:** codebase — `last_status` (models.rs:44) is set on the first fire; the FK is `ON DELETE SET NULL`, so this pair is an unambiguous "had a conversation, now gone" signal.

### DEC-6: Which NULL-referent cases pre-emptively pause (ITEM-7)?
**Resolution:** Pause pre-emptively ONLY for: (a) workflow-kind with `workflow_id IS NULL` → `target_missing`; (b) prompt-kind with a deleted bound conversation (DEC-5) → `conversation_deleted`. A prompt task with `assistant_id IS NULL` is NOT paused — NULL assistant legitimately means "use the user's default assistant" at fire time.
**Basis:** codebase — `workflow_id` is required for a workflow task; `assistant_id` is nullable-by-design (default).

### DEC-7 (CONFIGURABLE-SETTINGS): Run-history retention — fixed or admin-configurable (ITEM-8)?
**Resolution:** ADMIN-CONFIGURABLE via the EXISTING `scheduler_admin_settings.notification_retention_days` (already exposed at `GET/PUT /scheduler/admin-settings`, gated `scheduler::admin::manage`, with bounds validation). Run pruning reuses that same value — no new settings column, no new migration, no new permission.
**Basis:** migration 144 explicitly states runs are "time-pruned alongside notifications"; reusing the existing admin-tunable honors that intent and satisfies the configurable-settings rule without inventing a second retention knob.

### DEC-8 (CONFIGURABLE-SETTINGS): Transient-retry attempts + backoff — fixed or admin-configurable (ITEM-9, resolves CONCERN)?
**Resolution:** FIXED constants in a `RetryLimits` struct in `failure.rs`: `MAX_IN_RUN_ATTEMPTS = 2` retries (3 total tries), exponential backoff via the existing `retry_backoff_ms(attempt)`. NOT an admin setting.
**Basis:** it is a short in-firing transient-blip absorber, not an operational policy; `failure.rs` already hardcodes `retry_backoff_ms`, and the REAL operational policy — the consecutive-failure cap that triggers auto-pause — is ALREADY admin-configurable (`max_consecutive_failures`). Structured as a struct (not inline magic numbers) so it can be promoted to configurable later without a rewrite, per the rule's escape clause.

### DEC-9: Does the in-run retry sleep block the tick loop (ITEM-9)?
**Resolution:** The bounded retry (`tokio::time::sleep` between ≤2 attempts) runs inside `fire_task`, which the tick loop already SPAWNS off-thread (`tick.rs` spawns `fire_task`); the loop does not await it. Total added latency is bounded by the backoff sum (< a few seconds).
**Basis:** codebase — `tick.rs` dispatches `fire_task` on a spawned task, so a bounded sleep there cannot stall claiming.

### DEC-10: Completed once-task re-enable behavior (ITEM-10)?
**Resolution:** No change to `validate_schedule` — re-enabling a `once` task whose `run_at` is in the past still 400s (`RunAtInPast`); that is correct (a spent one-shot needs a NEW future time to run again). The fix is purely the `paused_reason='completed'` marker + the UI "Completed" badge, so the state is legible and the user is prompted to pick a new time rather than being silently blocked.
**Basis:** convention — a fired one-shot is done; re-running requires an explicit new schedule.

### DEC-11: Timezone — auto-detect vs picker (ITEM-2, per FB-3)?
**Resolution:** AUTO-DETECT only — no timezone control. On drawer/builder mount, set `timezone` from `Intl.DateTimeFormat().resolvedOptions().timeZone` and display it as read-only helper text ("Times are in your timezone: <zone>"). The user never inputs or picks a timezone. On edit of an existing task whose stored zone differs from the browser, show the stored zone read-only (do not silently rewrite it).
**Basis:** user (FB-3) — "who would try to input timezone? we need to auto detect timezone from user client." The drawer already computes `browserTz()`; this removes the field entirely.

### DEC-16: Multi-day-of-week cron encoding + humanization (ITEM-12, resolves CONCERN)?
**Resolution:** The weekly multi-toggle emits the cron day-of-week field as a SORTED, comma-separated numeric list `d1,d2,…` (0=Sun..6=Sat), e.g. Mon+Wed+Fri → `0 9 * * 1,3,5`. At least one day must be selected (validation: empty selection → "Pick at least one day"). `humanizeCron` is extended to parse a comma dow list into "Weekly on Mon, Wed, Fri at HH:MM"; any expression it can't classify still falls back to `Cron: <expr>`. Editing a task loads the selected days by splitting the stored dow on commas.
**Basis:** codebase — `schedule.rs` parses arbitrary valid 5-field crons via `cron`/`chrono_tz` and its min-interval floor samples a 24-occurrence window, so a comma dow needs NO backend change; only the frontend builder + `humanizeCron` change.

### DEC-12: Assistant picker "use my default" affordance (ITEM-1)?
**Resolution:** The Assistant picker offers a "Default assistant" option (value = empty ⇒ `assistant_id` omitted) plus the user's assistants. Selecting it sends no `assistant_id`, preserving the NULL⇒default semantics.
**Basis:** codebase — `assistant_id` is optional; NULL resolves to the user default at fire time.

### DEC-13: Workflow inputs when the IR declares none (ITEM-4)?
**Resolution:** Fall back to the raw JSON textarea (current behavior) when `parseWorkflowIr(workflow).inputs` is empty; render typed fields when it has inputs.
**Basis:** codebase — this is exactly `WorkflowRunDialog`'s structured-vs-JSON fallback.

### DEC-14: Reuse `AssistantSelector`/`ModelSelector` directly, or build controlled wrappers (ITEM-1)?
**Resolution:** Build small CONTROLLED wrappers in `TaskTargetPickers.tsx` (a `<Combobox>` for assistant + workflow, a grouped `<Select>` for model) bound to the form state, populated from the existing list stores (`AssistantPicker.availableAssistants`, `Workflow.workflows`, `ModelPicker.providers`). Reuse the DATA stores + kit primitives, not the global-selection-coupled selector components.
**Basis:** codebase — `AssistantSelector`/chat `ModelSelector` mutate GLOBAL picker state (`selectAssistant`, chat model snapshot), which is wrong for an isolated form; the reusable part is the list stores + the Combobox/Select primitives.

### DEC-15: run-now / test-fire rate-limit + concurrent-fire guard (surveyed Bug 6) — in scope?
**Resolution:** OUT of scope this iteration. Recorded in PLAN.md "Deferred". Not implemented here.
**Basis:** it needs its own rate-policy decision (per-user vs per-task, fixed vs admin-configurable) and would widen the diff; the HIGH/MEDIUM correctness bugs (ITEM-5..11) are the priority. Flagged for a follow-up.

### DEC-17 (UNATTENDED TOOL POLICY — the big one, per the human decision): how do tools + approval behave in a scheduled run?
**Resolution:** "Safe default + per-task allow-list" (human-selected). Concretely:
1. **Thread an `unattended` signal into the chat pipeline.** Extend the existing `invocation_source` (already carried to the workflow runner) into the `prompt` path: `dispatch_prompt` sets a request/`StreamContext.metadata` flag `unattended=true` (source=`scheduled`). The MCP `after_llm_call` approval decision (`mcp.rs:2043-2116`) branches on it.
2. **Auto-DENY, never silent-pause.** In unattended mode a tool that would need approval AND is not in the task's allow-list gets a synthetic denial `tool_result` ("`<tool>` requires approval; not permitted for this scheduled task") so the turn CONTINUES to a coherent answer — instead of writing orphaned `pending` approval rows and truncating. No pending-approval rows are created for unattended runs.
3. **No unattended side-effects outside the allow-list.** Constrain the attached servers via `mcp_config` to built-in read-only (`is_builtin_server_id`) + the task's allow-listed servers; a side-effecting tool (`is_side_effect_tool`) not in the allow-list is denied (never pre-executed, incl. Always-mode).
4. **Per-task allow-list (explicit up-front consent).** A new per-task field lists the MCP servers/tools the creator pre-authorizes to run unattended for THIS task; those bypass the per-call approval speed-bump at fire time (populated into the run's approval-bypass set). The allow-list is validated ⊆ the user's currently-accessible servers at create/update.
5. **Honest reporting.** Each run records which tools were skipped/denied; the completion notification says so ("Ran — 1 tool skipped (not permitted unattended)"), so a truncated result is never reported as a clean success.
**Basis:** user decision (AskUserQuestion) + reuses existing primitives (`invocation_source`, approval-mode chain, `is_builtin_server_id`/`is_side_effect_tool`, `mcp_config`, `auto_approved_tools`). Permissions are already enforced by owner `user_id` group-gating (agent-confirmed) — the allow-list narrows, never widens, what the user can already reach; so NO new permission is introduced.

### DEC-18: Workflow target under the unattended policy?
**Resolution:** (a) At task CREATE/UPDATE time, reject a `workflow` target whose compiled IR contains an `elicit` step with a clear error ("this workflow needs interactive input and can't be scheduled") — instead of letting it hang to the 15-min timeout and falsely record `failed`. (b) For a scheduled workflow run, apply the user's default `disabled_servers` to tool-steps (today the disabled-server filter is conversation-scoped and a scheduled run has none, so it is skipped) — pass the user's disabled set through `invocation_source=scheduled`. Workflow tool-steps otherwise remain author-pre-approved (no per-call approval), which matches the workflow design.
**Basis:** agent-confirmed gaps (workflow elicit hang; disabled-server filter conversation-scoped); reuses the IR parser already used for typed inputs (ITEM-4) to detect elicit steps.

### DEC-19: Storage + API shape for the per-task allow-list + skipped-tools report (migration 153)?
**Resolution:** Add migration `153` (next after main's `145`): `scheduled_tasks.allowed_unattended_tools JSONB NOT NULL DEFAULT '[]'` (list of `{server_id, tool_name?}` — tool_name NULL ⇒ whole server allowed), and `scheduled_task_runs.skipped_tools JSONB NOT NULL DEFAULT '[]'` (list of `{tool_name, reason}`). `CreateScheduledTask`/`UpdateScheduledTask` gain `allowed_unattended_tools`; `ScheduledTaskRun` exposes `skipped_tools`. This is a new type surface → OpenAPI regen (BOTH ui + desktop) is required.
**Basis:** convention — JSONB matches the existing `inputs_json` pattern; the run-report column mirrors the existing run-status columns. Default `'[]'` keeps existing rows valid (empty allow-list = read-only-only, the safe default).

---

## ROUND 2 — Follow-up & Series (human-chosen via option pickers, per FB-8)

### DEC-20 (CONFIGURABLE-SETTINGS): result-preview length / series-default-N / runs-page-size — fixed or admin-configurable (ITEM-40/41/47)?
**Resolution:** FIXED named constants — `PREVIEW_CHARS ≈ 280`, `SERIES_DEFAULT_N = 5`, `RUNS_PAGE_SIZE = 10`. NOT admin settings. Retention (the real operational policy) stays the existing `notification_retention_days` admin knob (DEC-7).
**Basis:** user pick (DEC-A picker) — these are presentation defaults, not operational policy; structured as named consts (per the config-rule escape clause) so any one can be promoted to configurable later without a rewrite.

### DEC-21: prompt-task primary follow-up action (ITEM-45)?
**Resolution:** Primary "Open thread" resumes the existing `bound_conversation_id` (the whole series, reply inline); the existing per-run fork survives as a secondary "New side chat". A never-fired prompt task shows "Open thread" disabled + tooltip. Workflow tasks (no thread) keep "Continue in chat".
**Basis:** user pick (DEC-B picker) — the series already lives in the bound conversation; surfacing it is the highest-leverage move, and keeping the fork preserves the throwaway-chat use case.

### DEC-22: series follow-up scope (ITEM-47/43)?
**Resolution:** A "Discuss recent runs ▾" chooser offering {5, 10, all-loaded}, default 5; the server clamps `limit` to the available run count.
**Basis:** user pick (DEC-C picker) — flexible without clutter; default 5 bounds the seed size for a long-running task.

### DEC-23: how the follow-up conversation is seeded with the real result (ITEM-42/43)?
**Resolution:** Synthesized ASSISTANT-turn seed — `continue_run_in_chat` (and the series path) create an assistant message carrying the REAL result (prompt run → the actual last assistant text; workflow run → `summarize_workflow_output` + artifacts as file ContentBlocks; series → one assistant summary of the last N runs + deltas); the user then replies. NOT a user-message embed.
**Basis:** user pick (DEC-D picker) — reads as if the assistant reported the result. For a prompt run the assistant text is the REAL turn (not fabricated); for workflow/series it is a synthesized-but-truthful assistant framing of the actual output.

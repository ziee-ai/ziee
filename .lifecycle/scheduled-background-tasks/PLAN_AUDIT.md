# PLAN_AUDIT — scheduled-background-tasks

Plan audited against the live codebase (workflow, sync, memory, chat, project,
mcp/tool_calls, citations modules) before any code.

## Breakage risk

- **`invocation_source` CHECK widening (ITEM-5)** is additive — migration 106
  already reserves `agent`/`mcp_tool` as future values, so adding `scheduled`
  follows the established forward-compat pattern and breaks no existing caller.
  `SpawnRunOpts.invocation_source` is a `&'static str`, so passing `"scheduled"`
  is a pure caller change; no signature change to `spawn_run`.
- **No existing caller touched.** The scheduler *consumes* `spawn_run` and
  `StreamingService::start_generation` as-is; it adds no parameters to either.
  Risk is one-directional (we depend on them), so their contracts are pinned by
  their own tests.
- **`resolve_run_model` requires an explicit model when `conversation_id` is
  `None`** (returns `WORKFLOW_NO_MODEL_SOURCE`). The plan makes `model_id`
  **required** for both target kinds (schema NOT NULL for `prompt`; required for
  `workflow` when the workflow has LLM steps). Enforced at create-time
  validation → no runtime surprise. Captured in DEC-4.
- **`dispatch_prompt` creates a real conversation per run.** Left unbounded this
  could accumulate conversations. Mitigated: notification links the conversation
  so it's discoverable, and the per-user active-task quota (ITEM-11) bounds the
  fan-out. Retention of run-created conversations is DEC-9 (out-of-scope: they
  are normal user conversations, user-deletable).

## Pattern conformance

- **ITEM-6/7 (module skeleton + repo)** mirror `citations/mod.rs` (linkme
  `ModuleEntry`, `AppModule::init` spawning loops) and `project/repository.rs`
  (owner-scoped CRUD). Conforms.
- **ITEM-9 tick loop** mirrors `memory/reaper.rs` (thin `run_loop` → testable
  `run_once`) + the `LLM_RUNTIME_REAPER_TICK_MS` debug-interval seam. Conforms.
- **ITEM-14/17 notification module** is a near-exact structural copy of
  `mcp/tool_calls/` (owner-scoped history + `prune.rs` retention loop reading
  admin settings each tick). Conforms.
- **ITEM-11 quota-422** mirrors `project` handlers returning 422 on the 100-file
  cap. Conforms.
- **ITEM-18 sync entity** follows the `event.rs` "add a variant, pick Audience at
  every emit site" contract; the 3 tiers chosen (owner/owner/admin-perm) match
  the read-perm each refetch endpoint enforces (no-403 rule). Conforms.
- **ITEM-21/25 stores** mirror `McpToolCalls.store.ts` (sync-subscribe +
  `sync:reconnect` + `hasPermissionNow` self-gate). Conforms.
- **ITEM-26 bell** mirrors `DownloadIndicatorWidget` `sidebarBottom` slot +
  `Badge`/`Popover`. Conforms.
- **DEC risk:** the scheduler introduces the *first* boot-persistent background
  loop that WRITES cross-user rows on a timer. It's single-process/in-memory like
  every existing loop; multi-instance HA is explicitly out of scope (DEC-10),
  consistent with `registry.rs`'s documented single-process assumption.

## Migration collisions

- Latest committed migration is `..131_rewrite_hub_ids_phibya_to_ziee_ai.sql`.
  The plan claims `132–136` — **no collision** (`ls migrations/ | tail` confirms
  131 is highest). Numbers are contiguous and correctly ordered (tables before
  the grant that references the Users group; grant before nothing depends on it).
- `scheduled_tasks.workflow_id`/`assistant_id`/`model_id` FKs reference existing
  tables (`workflows`, `assistants`, `llm_models`) — all present. `ON DELETE SET
  NULL` chosen so deleting a referenced workflow/model disables (not deletes) the
  task; the tick loop treats a NULL target as a hard error → notification +
  auto-disable (DEC-6).

## OpenAPI regen

- **Required — `just openapi-regen` for BOTH binaries (ITEM-20).** New response/
  request types (`ScheduledTask`, `Notification`, admin settings), new endpoints,
  3 new `SyncEntity` values, and 3 new permission strings all flow into
  `openapi.json` + `api-client/types.ts` + `Permissions` + the `sync:<entity>` TS
  union in BOTH `ui/` and `desktop/ui/`. The golden `types_ts_parity` test fails
  if not regenerated. Desktop has its own `types.ts`/`Permissions`/`AppEvents`, so
  the regen must run for the desktop binary too ([[project_openapi_regen_both_binaries]]).
- Generated files are excluded from the phase-6 audit-coverage law and do NOT
  make this a "UI diff" for the phase-3/8 frontend gates — but the *hand-written*
  frontend modules (ITEM-21..26) DO, so an e2e tier is mandatory (see TESTS.md).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new table; no collision (132 > 131); FKs exist.
- **ITEM-2** — verdict: PASS — new table; mirrors `mcp_tool_calls` shape.
- **ITEM-3** — verdict: PASS — singleton mirrors `memory_admin_settings`/`session_settings`.
- **ITEM-4** — verdict: PASS — grant mirrors migration 104/107; targets the `Users` system-default group.
- **ITEM-5** — verdict: CONCERN — CHECK widening is additive/safe, but must land as its own migration AND `cargo clean` is needed so build.rs re-runs migrations (per CLAUDE.md); noted for phase 5.
- **ITEM-6** — verdict: PASS — mirrors `citations`/`memory` module skeleton + `MODULE_ENTRIES`.
- **ITEM-7** — verdict: PASS — owner-scoped repo mirrors `project`; `FOR UPDATE SKIP LOCKED` claim mirrors `memory/reaper.rs`.
- **ITEM-8** — verdict: CONCERN — depends on the `croner` crate (ITEM-19) not yet in the tree; resolve the dep first. Timezone handling must be explicit (DEC-3).
- **ITEM-9** — verdict: PASS — thin-loop/testable-body + debug interval seam mirror `memory/reaper.rs`/`llm_local_runtime/reaper.rs`.
- **ITEM-10** — verdict: CONCERN — reuses `spawn_run` (clean) and `StreamingService::start_generation` (heavier: needs a conversation+branch+extension registry). Verify the extension registry is constructible off-request in a background task (DEC-5); fall back to the direct `Provider::chat_stream` pattern (summarizer.rs) if registry construction needs request state.
- **ITEM-11** — verdict: PASS — 422-on-cap mirrors `project`; singleton seed mirrors `session_settings::seed_from_config_once`.
- **ITEM-12** — verdict: PASS — REST CRUD mirrors `project/routes.rs`; owner-scope 404 mirrors `mcp/tool_calls`.
- **ITEM-13** — verdict: PASS — emit sites mirror workflow `events.rs`; origin threaded from `SyncOrigin`, `None` from the loop.
- **ITEM-14** — verdict: PASS — structural copy of `mcp/tool_calls/`.
- **ITEM-15** — verdict: PASS — owner-scoped REST; same-perm mutations justified (per-user data, citations precedent).
- **ITEM-16** — verdict: PASS — `create_and_emit` mirrors the detached-emit convention (`mcp/client/session.rs`).
- **ITEM-17** — verdict: PASS — exact `mcp/tool_calls/prune.rs` analog.
- **ITEM-18** — verdict: PASS — additive enum variants + vocab test; Audience tiers match refetch perms.
- **ITEM-19** — verdict: CONCERN — adds a new third-party crate (`croner`); must land in `[workspace.dependencies]` and both member Cargo.tomls, and `cargo check --workspace` must pass. Licence (MIT) is compatible. Resolve before ITEM-8.
- **ITEM-20** — verdict: CONCERN — regen BOTH binaries or the golden parity test + desktop build fail; mechanical but mandatory.
- **ITEM-21** — verdict: PASS — store mirrors `McpToolCalls.store.ts`.
- **ITEM-22** — verdict: PASS — settings-card style ([[feedback_match_settings_card_style]]).
- **ITEM-23** — verdict: CONCERN — the ScheduleBuilder (cron UX + "next 3 runs" preview) is net-new UI with no direct in-repo analog; budget a design pass + gallery states (loaded/empty/error) and an e2e for the create flow.
- **ITEM-24** — verdict: PASS — admin settings page mirrors existing `settingsAdminPages` slot pages.
- **ITEM-25** — verdict: PASS — toast listener mirrors `LlmModelDownloadNotifications.tsx`.
- **ITEM-26** — verdict: PASS — bell mirrors `DownloadIndicatorWidget`; desktop keeps it (not blocklisted, DEC-8).
- **ITEM-27** — verdict: PASS — `scheduled_task_runs` is the `mcp_tool_calls` analog (owner-scoped per-parent history); new `scheduled_tasks` columns are additive; migration 137 > 136, no collision. `bound_conversation_id`/`notification_id` FKs `ON DELETE SET NULL` so a deleted conversation/notification doesn't orphan the audit row (the delete is instead detected → task pause, ITEM-30).
- **ITEM-28** — verdict: CONCERN — the error taxonomy must map ziee's `AppError`/provider errors onto retry-vs-terminal correctly; the auth/perm/validation (non-retry) classes and transient (retry-with-backoff) classes are well-defined by the provider layer, but the mapping needs care. Flap-cap reuse from `auto_start.rs` is clean. Verify the backoff doesn't hold the claim tx (DEC-16: dispatch runs AFTER the claim commits).
- **ITEM-29** — verdict: PASS — `notify_mode` is a small enum honored at the single `create_and_emit` seam; the durable row is always written (auditability), only the toast/badge interrupt is gated. Conforms to the triage best practice.
- **ITEM-30** — verdict: CONCERN — task-bound conversation is a design flip from fresh-per-run (DEC-17, research-driven). Appending to a long-lived conversation grows context unbounded over many runs; mitigation is the existing summarization/`clear_old_tool_results` machinery, but the interaction with a scheduled append needs a phase-5 check. The "pause on conversation delete" needs a detection path (FK SET NULL + a tick-time null-check, since there's no delete trigger) — resolved as a null-check, not a DB trigger.
- **ITEM-31** — verdict: PASS — list + REST + drawer tab mirror `mcp_tool_calls`/`McpServerDrawer` exactly; owner-scoped 404.
- **ITEM-32** — verdict: CONCERN — the continue-in-chat endpoint reuses `create_conversation` + a seeded system/context message; must cap the seeded output size (the run's final output can be large — reuse the chat result caps) and be owner-scoped on the `run_id`. No new execution path.
- **ITEM-33** — verdict: PASS — pure frontend surfacing on existing stores/components; e2e budgeted (TEST-36..38).
- **ITEM-34** — verdict: CONCERN — the `workflow` kind cleanly reuses `run_for_test` (proven dry-run path); the `prompt` kind must guarantee **zero** side effects — the throwaway conversation must be created + torn down (or created ephemeral, mirroring migration 128's `ephemeral` workflow rows) and the notification/append seams explicitly bypassed, not just skipped by omission. Verify no `scheduled_task_runs` row and no `next_run_at` mutation on the test path.
- **ITEM-35** — verdict: PASS — a "Test" action + inline result panel; mirrors the workflow dev dry-run UI; e2e budgeted (TEST-41).
- **ITEM-36** — verdict: CONCERN — change-detection is v1 (user-chosen). Exact set-diff is clean when the result carries identifiable IDs (reuse `lit_search::dedup`); the fallback normalized-text fingerprint must be stable against benign volatility (timestamps/ordering) or `on_change` will over- or under-fire — the normalization rules need care + explicit unit coverage. The "unchanged → suppress notification but still record the run" path must not skip fingerprint persistence.
- **ITEM-37** — verdict: PASS — a `notify_on` toggle + delta line; mirrors the `notify_mode` control; e2e budgeted (TEST-44).

## Lifecycle completeness check (so the plan stops leaking affordances)

The plan now covers the full task lifecycle end-to-end: **author** (create form,
target picker) → **validate** (schedule preview + dry-run "Test" before save,
ITEM-23/34/35) → **save/enable** → **fire** (tick + catch-up) / **test live**
(run-now) → **deliver** (inbox + triage, ITEM-14/29) → **follow up** (bound
conversation / continue-in-chat, ITEM-30/32) → **observe** (run history,
ITEM-31) → **recover** (failure taxonomy + auto-pause + resume, ITEM-28/33) →
**edit/disable/delete** (CRUD, ITEM-12). **Change-detection / only-on-change is now IN v1**
(DEC-20, ITEM-36/37). Deliberately **deferred** (with hooks, not dead-ends):
digest batching (DEC-19), natural-language scheduling (DEC-3), email/push
channels (DEC-23), and task **duplicate** (DEC-25) — none block v1 and each has a
clear later seam.

## Feature-research reconciliation (why the plan grew)

The initial plan implemented the mechanism; the phase-1 feature-level research
(explicitly requested) showed the *experience* was under-specified. The added
items are not gold-plating — each closes a gap that every comparable product
treats as core: **failure surfacing + auto-pause** (Claude/ChatGPT show per-run
success/failure and pause on repeated failure), **run history/activity feed**
(all three expose it), **delivery triage** (interrupt only on actionable), and
**native follow-up on results** (the user's own question; ChatGPT binds a task
to an expandable conversation). The user then **pulled change-detection/"only-notify-on-change" into v1**
(DEC-20, ITEM-36/37 — the top monitoring differentiator for the life-science
use case). Remaining deferred-with-hooks: digest batching windows (DEC-19),
natural-language scheduling (DEC-3), email/push channels (DEC-23), task duplicate
(DEC-25). None paint us into a corner (`notify_mode` + cron-as-source-of-truth +
the `notification` module seam leave the doors open).

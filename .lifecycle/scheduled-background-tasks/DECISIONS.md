# DECISIONS — scheduled-background-tasks

Every product/human input resolved up front so implementation runs nonstop.
Each resolution has a basis; the handful I most want the user to confirm before
`go` are flagged **[CONFIRM]** and repeated in the halt summary.

### DEC-1: Scheduler mechanism — DB table + boot tick loop, or system cron?
**Resolution:** DB-backed `scheduled_tasks` table + a boot-spawned in-process tick loop (module `init` → `tokio::spawn(run_tick_loop)`), reading due tasks each tick. No OS cron, no external scheduler.
**Basis:** codebase — mirrors the existing `memory/reaper.rs`, `mcp/tool_calls/prune.rs`, `llm_local_runtime/reaper.rs` loops; keeps state in Postgres (survives restart), needs no host cron, and works identically inside the desktop embedded server.

### DEC-2: What does a scheduled task reference? [CONFIRM]
**Resolution:** A polymorphic `target_kind` with two kinds at launch: (a) `workflow` — a saved workflow + inputs + model; (b) `prompt` — an assistant (task's or the user's default) + a prompt message + model, run as a fresh conversation. The two map 1:1 to the two proven internal execution seams. The `target_kind` column makes adding kinds later (e.g. a saved-search "recipe") a code-only change.
**Basis:** codebase — `workflow::runner::spawn_run` and `chat::StreamingService::start_generation` are the only two production seams that run an LLM turn without a browser; the user's examples ("re-run a workflow on a cron" → workflow; "check PubMed weekly and summarize" → an agentic assistant turn with lit_search/web_search tools) require both.

### DEC-3: Recurring-schedule format + timezone.
**Resolution:** Standard 5-field cron (`min hour dom mon dow`) parsed by the `croner` crate, plus a per-task IANA `timezone` string; `once` tasks store a single UTC `run_at`. `next_run_at` is always stored in UTC. The create form offers common presets (daily/weekly/monthly) that emit cron under the hood, with a raw-cron escape hatch.
**Basis:** convention — cron is the least-surprising recurring vocabulary (matches ChatGPT/Gemini scheduled-tasks and every comparable agent tool); storing UTC + an explicit tz avoids DST ambiguity.

### DEC-4: Model resolution for a scheduled run (no conversation to inherit from).
**Resolution:** `model_id` is required at task-create time (NOT NULL for `prompt`; required for a `workflow` that has LLM steps) and validated against the creating user's model access. Stored as a snapshot FK (`ON DELETE SET NULL`); a NULLed model disables the task with an error notification.
**Basis:** codebase — `resolve_run_model` returns `WORKFLOW_NO_MODEL_SOURCE` when both `model_id` and `conversation_id` are absent, so a scheduled run MUST carry an explicit model.

### DEC-5: How the `prompt` target executes. [CONFIRM]
**Resolution:** Create a real conversation + branch (`chat::core::create_conversation`/`create_branch`) and run one turn via `StreamingService::start_generation`, so the result is a normal conversation the user can open, continue, and see tool calls in. If constructing the extension registry off-request proves to need request-scoped state, fall back to the `summarizer.rs` pattern (`Provider::new(..).chat_stream(ChatRequest)`) storing the output directly in the notification body — decided during phase-5 implementation, not left open (both paths are proven in-tree).
**Basis:** codebase — `StreamingService::start_generation` is the full agentic pipeline (tools/extensions/memory); the direct-provider path is the documented lighter fallback. The user's "found 12 new papers" example needs tool-calling, favoring the conversation path.

### DEC-6: Catch-up semantics on downtime.
**Resolution:** Coalesced single fire. On boot / first tick, any task whose `next_run_at` is in the past fires **once**, then `next_run_at` advances to the first occurrence after `now` (missed intermediate occurrences are skipped, not backfilled). A `once` task past its time still fires once on the next tick, then disables.
**Basis:** convention — matches ChatGPT Tasks and mirrors `workflow::startup_sweep` (which reconciles in-flight state at boot rather than replaying); backfilling N identical LLM runs would waste tokens and spam the inbox.

### DEC-7: Per-user quota + abuse floor.
**Resolution:** A deployment-wide, admin-configurable `max_active_tasks_per_user` (default 20) enforced with a **422** on create, plus a `min_interval_seconds` floor (default 300) rejecting sub-5-minute crons. Both live in the `scheduler_admin_settings` singleton, read fresh each create/tick.
**Basis:** convention — ChatGPT caps active tasks per plan (3–15); ziee has no plans, so a single admin cap fits; the 422-on-cap + admin-singleton patterns already exist (`project` cap, `memory_admin_settings`).

### DEC-8: Desktop vs server — does a schedule run only when the app is open? [CONFIRM]
**Resolution:** The scheduler module is NOT desktop-blocklisted; it runs inside the embedded server, so on desktop it fires only while the app (or headless mode) is running. Missed firings during app-closed time are handled by DEC-6 catch-up on next launch. A short note in the create UI states this on desktop. No cloud/always-on execution is in scope.
**Basis:** codebase — every existing background loop (`memory/reaper`, etc.) already behaves this way inside the embedded server and is not blocklisted; adding a cloud runner is a separate infrastructure effort ([[project_desktop_embeds_server]]).

### DEC-9: Permissions model.
**Resolution:** New `scheduler::use` (create/manage own tasks + run-now) and `notifications::read` (list/mark/delete own — same perm for reads and the strictly-per-user mutations) granted to the default Users group (migration 135). Admin-only `scheduler::admin::{read,manage}` (quota/retention settings) via the Administrators `*` wildcard. The actual task *execution* re-checks `workflows::execute` / model access downstream in `spawn_run`.
**Basis:** codebase — mirrors the citations `use`-covers-per-user-mutations rationale and the migration-104/107 grant pattern; execution-time re-check is how workflow already gates.

### DEC-10: Multi-instance / HA execution.
**Resolution:** Out of scope. Single-process in-memory scheduling like every existing loop; the `FOR UPDATE SKIP LOCKED` due-claim is included so a *future* multi-instance deployment is race-safe, but exactly-once across replicas (LISTEN/NOTIFY / leader election) is not built now.
**Basis:** codebase — `sync/registry.rs` documents the single-process assumption for the whole realtime layer; HA would be a cross-cutting effort beyond this feature.

### DEC-11: Notification data model + lifecycle.
**Resolution:** Owner-scoped `notifications` rows with `kind`/`title`/`body` + nullable deep-link FKs (`scheduled_task_id`/`workflow_run_id`/`conversation_id`), `read_at`, `created_at`. Live push via `SyncEntity::Notification` (owner audience, origin=None) + an EventBus toast; durable so a missed toast is readable later. Retention: admin `notification_retention_days` (default 30, `0`=forever) pruned by a loop mirroring `mcp/tool_calls/prune.rs`.
**Basis:** codebase — structural copy of `mcp_tool_calls` (owner-scoped history + sync entity + retention prune); no notification/inbox exists today, so this is the greenfield model.

### DEC-12: Notification is a general module, not scheduler-internal.
**Resolution:** `notification` is its own module (own table/REST/sync/UI); the scheduler calls its `create_and_emit` seam. Other future producers (a finished long download, a completed manual workflow run) can write to the same inbox later.
**Basis:** convention — separation of concerns matches how `sync`, `memory`, `mcp/tool_calls` are independent modules; avoids coupling the inbox to one producer.

### DEC-13: `run-now` semantics.
**Resolution:** `POST /{id}/run-now` fires the task's target immediately, off-schedule, WITHOUT mutating `next_run_at` or `last_run_at`'s schedule bookkeeping (it records a run + notification like any firing). Enables "test my task" from the UI.
**Basis:** convention — matches the manual-trigger affordance users expect next to a scheduled item; reuses the same dispatch path as the tick.

### DEC-14: Frontend placement.
**Resolution:** Scheduled Tasks is a top-level authenticated page `/scheduled-tasks` (nav entry) — it's a primary user surface, not buried in settings. The scheduler *admin* quota page lives at `/settings/scheduler` (`settingsAdminPages`). The notification bell is a `sidebarBottom` widget (badge+popover); the full inbox is `/notifications`.
**Basis:** convention — mirrors ChatGPT's dedicated "Scheduled" sidebar page (a primary surface), and reuses ziee's `sidebarBottom` slot (where `DownloadIndicatorWidget` already lives) for the bell.

### DEC-15: New dependency — `croner`.
**Resolution:** Add `croner` (MIT, chrono-compatible, actively maintained) to `[workspace.dependencies]` + the server crate for cron parsing / next-occurrence computation. Not vendored.
**Basis:** convention — cron math is subtle (DST, dow/dom semantics); a maintained crate beats hand-rolling, and `chrono` is already a workspace dep so integration is clean.

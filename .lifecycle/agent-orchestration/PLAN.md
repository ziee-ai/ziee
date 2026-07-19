# PLAN — agent-orchestration (Phase 1 = a DESIGN MENU, scope NOT yet locked)

> **Read this first.** This is a Phase-1 research + planning artifact deliberately
> written as a **menu of candidate capabilities + architecture options**, NOT a
> committed feature. The human will pick scope in a direct chat before we advance
> past Phase 1. Every `ITEM-N` below is a *candidate*; the extensibility question
> (Group D) is presented as **three mutually-exclusive architecture OPTIONS** with
> tradeoffs and a recommendation. Nothing here is implemented; no merge/push/flip.
>
> The problem space (from the human): an **agent-orchestration surface** for
> chat / workflow / future agents, covering (1) on-demand sub-agents, (2)
> background sub-agents, (3) background sandbox execution, (4) a **general,
> extensible** background-job + spawn-sub-agent abstraction (the hard part), and
> (5) `/loop` + `/schedule` in chat (possibly merged into one dialog).
>
> **SOTA research is done** (human asked for it before locking scope) — see
> **[`RESEARCH_SOTA.md`](./RESEARCH_SOTA.md)** (5 primary-source research passes over
> Claude Code / Codex / Cursor / Goose / DBOS / LangGraph / Mastra / …). Two headline
> results folded into this plan: (a) the 5-group spine is **on the right track**,
> with **completeness gaps + a durability/hardening set** added below as **Group F**;
> (b) the backbone recommendation is **REVISED** to a hybrid — *A-substrate + B-facade +
> C-as-a-kind* (see § Architecture options).
>
> **✅ SCOPE NOW LOCKED (human-selected, see § Locked scope):** full **A–E**; backbone =
> **hybrid (reuse `workflow_runs` + `JobKind` registry)**; **all four** completeness
> capabilities in (goal-seeking ITEM-24, steer ITEM-25, unified inbox ITEM-26, event
> triggers ITEM-27); **hardening ITEM-29–32 in as baseline**; build order **A + E first →
> backbone → B + C**. Plus **Group G — agent self-task-management** (`TodoWrite`-style,
> ITEM-34–37), added on the human's request so a long agent run doesn't forget its
> steps. Still **Phase 1** — this refines the menu into the agreed scope; nothing
> implemented, nothing pushed. Ready for Phase 2 (plan-audit) on the human's go.

---

## What already exists (research summary — the plan is mostly *surfacing*, not from-scratch)

Four parallel code-探 sweeps (agent-core, chat agent-host, code_sandbox + background
infra, scheduler) established the ground truth. Key facts every item below builds on:

- **`agent-core` shared loop** (`src-app/agent-core/`) already has the six ports
  (`TranscriptStore/EventSink/ToolProvider/HumanGate/ApprovalPolicy/ModelResolver`)
  + `reviewer` + `AgentExtension` seam. **`AgentCore::fan_out(user_id, Vec<SubagentSpec>, cancel) -> Vec<SubagentSummary>`** (`fanout.rs`) is fully built and tested: bounded by `SubagentLimits{max_threads=6,max_depth=1}`, returns **summaries not transcripts**.
- **BUT fan-out is Rust-only and dormant:** no model-facing `delegate`/`spawn_subagents` tool exists, `ToolScope.allow_delegate` gates nothing (always `false`), and **no host ever calls `fan_out`**. Surfacing it to the model is net-new but the engine is done.
- Chat consumes agent-core behind the opt-in flag **`ZIEE_CHAT_AGENT_CORE=1`** (still hardening); the workflow `kind: agent` step (`workflow/agent_dispatch.rs`) uses it in production.
- **Everything is block-until-done today:** a chat turn (single-flight per conversation via `begin_generation`; the turn itself runs detached via `tokio::spawn` and callers poll `is_generating`), the `workflow_mcp` `wf_<slug>` tool (spawns the runner then **blocks** to terminal), and `code_sandbox` `execute_command` (**synchronous, hard 600 s wall-clock**, 1 MiB/stream output cap, `kill_on_drop(true)` + RAII guards). There is **no** standalone background/detached mode for any of them, and **no generic job-queue table**.
- **`workflow_runs` + runner is the primary durable background executor:** fire-and-forget `spawn_run` → status `pending|running|waiting|resumable|completed|failed|cancelled`, a liveness heartbeat, per-run SSE (`RunHandle` broadcast + **snapshot-on-connect**), a **boot `startup_sweep`** that fails orphans / re-drives `resumable` runs, and cross-session completion notify via the **sync bus** (`SyncEntity::WorkflowRun`, `Audience::owner`) + the durable **`notification` inbox**.
- **The `scheduler` module already implements item 5's substance:** `scheduled_tasks` with **once + recurring cron** (IANA tz), a boot tick loop (`FOR UPDATE SKIP LOCKED`, coalesced catch-up), a **prompt target that drives a real chat turn** (`StreamingService::start_generation` into a bound conversation) or a workflow target, an **unattended-tool allow-list**, results to the `notification` inbox, **continue-in-chat** seeding, admin settings, and a full FE (`ScheduleBuilder.tsx`, `ScheduledTaskFormDrawer.tsx`, standalone "Scheduled Tasks" page). The **only gaps**: no slash-command parser / composer affordance in chat, and no "self-paced (model-decides-interval)" schedule mode.
- **Admin policy home already exists:** `agent_admin_settings` singleton (sandbox mode, unattended approval policy, reviewer, token caps, **`fan_out_max_threads` / `fan_out_max_depth`**), gated `agent::settings::{read,manage}`.
- **Open extension seams to reuse (no central enum to edit):** chat SSE events (`#[macros::compose_chat_stream_events]`) and content-block variants (`#[macros::compose_message_content_variants]`) are proc-macro-composed per module; the chat composer exposes slots (`toolbar_actions`, `toolbar_plus_items`, `input_area_prefix/suffix`, `text_input`); right-panel renderers register via `registerPanelRenderer` + `displayInRightPanel`.
- **Repo caveat:** the sandbox execution engine lives in the **`sdk` submodule** (`sdk/crates/ziee-sandbox`, pinned `9e6d8c74`) — a background-sandbox item touches a submodule, not just the server tree.

---

## Items

> Grouped by capability area (A–E). Items marked **[OPTION]** are alternatives the
> human chooses between, not all-of. Items marked **[stretch]** are opt-in extras.
> This is the candidate set; the selected subset becomes the real build list at the
> scope checkpoint (then re-gated through Phases 2–3).

### Group A — On-demand sub-agents (surface the built `fan_out` to the model)

- **ITEM-1**: Add a core-injected **`delegate` / `spawn_subagents` built-in tool** — `AgentCore` injects it into `ToolProvider::list` when `ToolScope.allow_delegate == true`; the model calls it with N child specs `{system, tool_scope, model_id?, reasoning_effort?}`; the loop routes the call into `AgentCore::fan_out` and returns the **merged child summaries** as one tool result. (Engine exists; this is the missing model-facing seam.)
- **ITEM-2**: Wire `allow_delegate` on the host side — chat agent-host + workflow `kind: agent` set `allow_delegate = true` (gated by an admin/depth guard) so the delegate tool is offered; children keep `allow_delegate = false` (preserves `max_depth = 1`, no grandchildren).
- **ITEM-3**: Enforce guardrails at the delegate call site — child count / `max_threads` / `max_depth` from `agent_admin_settings` (`fan_out_max_*`); a per-call child cap with an honest "capped at N" note (no silent truncation); each child's `tool_scope` narrowed to ≤ the parent's reachable servers (RBAC).
- **ITEM-4**: **Chat surfacing** of a fan-out — a "delegated sub-agents" activity card in the timeline (N children, per-child running/done status, then the merged summary), via a new SSE event + content-block variant on the compose seams. (Presentation over the existing tool-result plumbing.)
- **ITEM-5**: **Workflow surfacing** of a fan-out inside a `kind: agent` step — one `StepProgress` track per child (reuse `WorkflowEventSink`), summaries folded into the step output.
- **ITEM-6** [stretch]: A **friendly domain phrasing** of delegation ("Researching 3 angles in parallel…") consistent with the friendly-agent-surface language, not `spawn_subagents`/`fan_out` jargon.

### Group B — Background sub-agents (run beside a live foreground chat)

- **ITEM-7**: A **background sub-agent run** — the user (or an agent/workflow) launches a bounded sub-agent group that runs **detached** while the foreground chat stays interactive; it does **not** hold the per-conversation generation lock. (Requires the Group-D backbone for its durable run row + lifecycle.)
- **ITEM-8**: A model-facing / user-facing **"check status"** affordance — a `background_status` tool + a chat control that reports each running background task's progress without blocking.
- **ITEM-9**: **Results-land-when-done** — on completion a background sub-agent group posts its merged result into the conversation (a synthesized assistant turn or a system card, reusing the scheduler's `continue_chat` seeding pattern) **and** a `notification` inbox row + `sync` fan-out so an away user is told.
- **ITEM-10**: Concurrency + lifecycle correctness — a background task surviving a page reload / server restart (startup_sweep re-drive), cancellation, and access-loss (conversation deleted → task cancelled + workspace reclaimed).

### Group C — Background sandbox code execution (Claude-Code-style)

- **ITEM-11**: A **background/detached mode for `code_sandbox` execute_command** — start a long command, keep the `tokio::process::Child` **plus all RAII guards** (cgroup/seccomp/progress-FIFO/inflight, which `kill_on_drop(true)` would otherwise reap) alive in a process-global run registry, and return a handle immediately instead of blocking to 600 s.
- **ITEM-12**: A **status/output tool** for a background command — `check_command_status(handle)` / `get_command_output(handle, offset?)` streaming stdout/stderr via the existing FIFO `$ZIEE_PROGRESS → mpsc → SSE` seam (today only wired inside a workflow sandbox step), with the 1 MiB cap + truncation marker preserved.
- **ITEM-13**: **Completion notify + surfacing** — background command completion emits a `notification` row + `sync` fan-out; a chat right-panel shows live output + final status. Reuses the durable per-conversation sandbox workspace (already persists across turns).

### Group D — Extensibility backbone (the architecture question — pick ONE)

> This is the "hard part" the human flagged: a **general, extensible** mechanism
> for background jobs + spawning sub-agents that many cases plug into (sub-agents,
> background sandbox, future long-running tools), NOT one-off hacks. The three
> options below are mutually exclusive; see **§ Architecture options** for full
> tradeoffs + a recommendation. Whichever is chosen becomes the substrate ITEMs
> 7–13 build on.

- **ITEM-14** **[OPTION A]**: **Generalize `workflow_runs` into the background-run backbone.** Add new invocation kinds / sources (`subagent-group`, `sandbox-exec`, `background-turn`) that reuse the runner's spawn/heartbeat, `RunHandle` broadcast SSE, snapshot-on-connect, `startup_sweep` orphan-reclaim, and `SyncEntity::WorkflowRun` notify. Minimal new tables; leans on the most battle-tested durable primitive.
- **ITEM-15** **[OPTION B]**: **New generic `background_job` module.** A first-class job abstraction — a `background_jobs` table + a `JobKind` trait/registry (each producer registers a kind: `subagent-group`, `sandbox-exec`, …) + a tick/executor + per-job SSE (mirroring `workflow/registry.rs`) + `notification`/`sync` on completion + `startup_sweep`. Cleanest open seam; a new long-running kind plugs in without editing a central `match`.
- **ITEM-16** **[OPTION C]**: **An `agent-core` "detached turn" seam.** Extend the loop with a background/suspend-resume turn primitive (the gate already produces `GateOutcome::Suspended`; `resumable` runs already re-drive on boot). Producers are modeled as agent turns/sub-turns reusing the durable transcript + startup_sweep. Narrowest, most agent-native — but the raw sandbox-exec case doesn't map cleanly onto an "agent turn."
- **ITEM-17**: The **shared model-facing surface** on the chosen backbone — a small, uniform tool trio (`spawn_background{kind,spec}` / `check_status{handle}` / `collect_result{handle}`) + a uniform completion-notify + right-panel contract, so sub-agents, background sandbox, and future kinds all speak the same interface (the "not a one-off MCP hack" requirement).

### Group E — `/loop` + `/schedule` in chat (surface the existing scheduler)

- **ITEM-18**: A **chat composer affordance** — a `toolbar_actions`/`toolbar_plus_items` slot button ("Schedule / Loop this…") that opens a merged dialog and creates a `scheduled_task` (prompt target) **bound to the current conversation**. (Backend already exists end-to-end.)
- **ITEM-19** **[OPTION]**: A **slash-command parser** in the composer (`/loop`, `/schedule`) as an *alternative* entry to the same dialog (there is no slash parser today — this is a net-new composer layer). Choose slot-button-only, slash-only, or both.
- **ITEM-20**: A **merged Loop-or-Schedule dialog** mirroring `ScheduleBuilder.tsx` with three modes in one form: **Once** (schedule a one-time message at time T) · **Recurring** (cron/interval loop) · **Self-paced** (a `/loop` with no interval — the model decides when to next check). Text content + target reuse `ScheduledTaskFormDrawer`.
- **ITEM-21**: A **new "self-paced" schedule kind** — the one genuinely-new backend mode (cron covers fixed intervals; self-pacing is model-driven `next_run_at`). Adds a `ScheduleKind::SelfPaced` + a mechanism for the run to propose its own next fire time.
- **ITEM-22**: **Bind-to-current-conversation** — land loop/schedule results in THIS chat (vs the scheduler's per-task bound conversation), reusing `continue_chat` seeding; an in-chat list of a conversation's attached loops/schedules (pause/edit/delete) reusing `ScheduledTaskCard`.
- **ITEM-23** [stretch]: Reconcile the standalone "Scheduled Tasks" page with the new in-chat entry (one source of truth; the chat affordance is a second door onto the same tasks).

### Group F — Research-surfaced additions (from RESEARCH_SOTA.md — completeness + hardening)

> These are NOT in the original 5 groups; the SOTA sweep flagged them. Split into
> completeness gaps (new capability) and correctness/hardening (make B/C/A actually
> production-grade — several are where our current sketch is *behind* SOTA).

**Completeness gaps (new capability, ranked)**
- **ITEM-24** (HIGHEST): **Goal-seeking / verification loop** (`/goal` analog) — a *different axis* from scheduling: keep working across turns until a cheap independent evaluator confirms a natural-language completion condition ("done when the QC figure passes / no missing values"). Answers "green ≠ success"; grounds trust for non-technical users. Fold into the Group-E dialog as an optional "done when…" condition; host on the evaluator-model + workflow + memory ziee already has.
- **ITEM-25** (HIGH): **Steer a running agent** — nudge / redirect / queue a note to a background sub-agent or long sandbox run *without killing it* (Groups B/C). Avoids restart-from-scratch.
- **ITEM-26** (HIGH-MEDIUM): **Unified background-work inbox/dashboard** — one consolidated surface (state + peek + unread + result) across Groups B/C/D/E. The connective tissue that makes the groups feel like one system; every SOTA leader converged on it.
- **ITEM-27** (MEDIUM-HIGH): **Event-driven "monitor & notify" triggers** — "notify me when the sequencing run finishes / this dataset changes / this file appears" (cron can't express this; a top scientist JTBD). Add an event/completion trigger alongside cron; prefer event-push over Group-C polling.
- **ITEM-28**: **Live agent TODO checklist (UI)** — the "plan → steps checking off live" surface. This is only the *render*; the agent-facing mechanism (the tool + context re-injection) is **Group G** — ITEM-28 is absorbed by ITEM-36.

**Correctness / hardening (make B/C/A production-grade — several are corrections)**
- **ITEM-29**: **Persisted task state machine + boot orphan-reclaim** for background work — `queued→running→{completed|failed|cancelled}` **plus a `needs_input` state with a reply affordance**. Replaces the in-memory `tokio::spawn`+`is_generating` flag (which does NOT survive a restart). This is the durability the backbone (§Architecture) provides.
- **ITEM-30**: **Sandbox background output backpressure fix** — continuously drain both pipes into a **ring / head+tail buffer** (or spill to a per-run workspace file with byte-range paging); the current 1 MiB hard drop-after is WRONG for background (loses the recent tail).
- **ITEM-31**: **Sandbox background lifetime policy** — absolute-max + idle/no-new-output reaper + bind to conversation/sandbox teardown; report `timed_out` distinctly; **kill the cgroup** (reaps grandchildren); terminal-state registry reaping + prune-on-every-path (incl. server shutdown); re-apply ALL hardening on the new path.
- **ITEM-32**: **Untrusted-output scanning of child summaries** — scan a sub-agent's merged summary for instruction-shaped injection (`<system-reminder>`/`Human:`/permission strings) before the parent reads it (children run bio/web/lit MCP = untrusted content). Cheap, high-value.
- **ITEM-33** [stretch]: **Named, reusable agent definitions** + a **cumulative per-conversation spawn budget** (concurrency cap ≠ total-spawns cap; Claude uses 200/session) + **streaming child progress** + **per-child sandbox/approval mode**. The Group-A ergonomics/governance polish.

### Group G — Agent self-task-management (the agent's OWN task list, Claude-Code-`Task`-tools-style)

> Added at the human's request: *"agents need to manage their task so they don't
> forget things like how claude code does."* This is the **agent's own working
> checklist** — DISTINCT from Group E (user-scheduled tasks/loops) and from the
> ITEM-26 inbox (a user-facing view of background work). It is a core
> anti-forgetting / self-tracking mechanism for long multi-step agentic runs, and it
> belongs in **`agent-core`** so chat, the workflow agent step, sub-agents, and
> future agents ALL inherit it. (Previously flagged **absent + deferred** in the
> friendly-agent-surface handoff: "live agent-authored task checklist — needs agent
> tool + SSE + renderer → deferred v2." This builds it.)

> **Modeled on Claude Code's CURRENT structured `Task` tools, not legacy `TodoWrite`.**
> Research (RESEARCH_SOTA.md §8) found CC **replaced `TodoWrite` with `TaskCreate`/
> `TaskUpdate`/`TaskGet`/`TaskList`** as of v2.1.142 (per-item create+patch + a
> first-class read-back + deps + `owner` + disk persistence). Group G tracks that
> current design.

- **ITEM-34**: A core agent-facing **task-list tool set** — mirror CC's current `TaskCreate`/`TaskUpdate`/`TaskGet`/`TaskList` (per-item create + patch-by-id + a **first-class read-back**), NOT a single-array `update_task_list` rewrite. Item = `{ subject/content, active_form, status: pending|in_progress|completed, owner?, deps? }` (`active_form` is Anthropic-specific — present-continuous for rendering; Codex's `update_plan` uses a single `step` + status `done`, so don't treat the dual-form as universal). Built into `agent-core` so chat + workflow-agent + **each sub-agent** get their own list. The tool DESCRIPTION must carry the behavioral rules **verbatim** (they are the substance, not just the schema): use it frequently ("you may forget important tasks"); **exactly one `in_progress`** and keep **≥1 `in_progress` until all done**; **mark complete IMMEDIATELY, don't batch**; never complete on failure/partial; use for **3+ steps, skip trivial/conversational**.
- **ITEM-35**: **Keeping the list live in context — TWO distinct mechanisms** (corrected: I had conflated them). (a) *In-session freshness:* when the list CHANGES, re-surface the current list as an **out-of-band `<system-reminder>` block** (with a "don't mention/attribute to the user" note) on the next turn — **change-gated**, NOT re-emitted before every LLM call (lists are tiny so always-inject is tolerable, but change-gated is the token-aware default CC uses). (b) *Surviving compaction:* the `CompactionExtension` **explicitly re-emits** the current list post-summary (a per-turn reminder does NOT by itself survive compaction). Enabler for both: the list's **source of truth is a DURABLE store** (a DB table / an `assistant_core_memory`-style block), re-rendered into the out-of-band block — NOT the raw transcript. This is exactly what CC's disk-backed Task tools do, and it makes "survive compaction" trivially true (also aligns with the workflow-agent's durable-resume).
- **ITEM-36**: **Live checklist render** — surface the list live in chat (+ per-run in the workflow progress view), driven off the `tool_use` stream / a new SSE event on the compose seams (mirror `mcpToolProgress`). Copy CC's render rule: show the `in_progress` item by its **`active_form`** ("Running tests"), all others by **`content`/`subject`** ("Run tests"). Absorbs ITEM-28.
- **ITEM-37** [stretch]: **Sub-agent list semantics (corrected — no bespoke rollup)** — each delegated sub-agent gets its **OWN isolated, run-scoped** list; parent and child **never see each other's list** (CC default: the parent receives only the child's final summary text, NOT its todos — so a fan-out's progress stays legible via summaries without leaking child transcripts). Do **not** ship an automatic "rollup" (neither CC nor Codex does that — that was my invention). IF genuine cross-agent coordination is later needed, adopt CC's proven **shared-list-id + `owner`** opt-in (one shared list many agents write to), never an auto-merge.

---

## Files to touch

> Best-estimate surface per area; the chosen scope narrows this. Migrations are
> **timestamp-named** (`YYYYMMDDHHMM_<name>.sql`) in the **owning module's**
> `migrations/` dir (highest today: `202607170105_mcp_review_classification.sql`).

**Group A — on-demand sub-agents**
- `src-app/agent-core/src/fanout.rs` (route delegate calls; read `reasoning_effort`), `src-app/agent-core/src/core.rs` (inject the delegate tool into the tool list when `allow_delegate`), `src-app/agent-core/src/types.rs` (delegate tool spec / result), `src-app/agent-core/src/ports.rs` (if `ToolProvider::list` needs the delegate hook).
- `src-app/server/src/modules/chat/agent_host/{resolver.rs,dispatcher.rs}` (set `allow_delegate`, surface the fan-out), `src-app/server/src/modules/workflow/agent_dispatch.rs` (same for the workflow host).
- Chat surfacing: a new/extended chat extension emitting the sub-agent SSE event + content-block variant (compose seams), FE renderer under `src-app/ui/src/modules/chat/...` mirroring the friendly timeline card.

**Group B/C/D — background backbone + sub-agents + sandbox** *(shape depends on the chosen Option)*
- Option A: `src-app/server/src/modules/workflow/{runner.rs,registry.rs,repository.rs,models.rs,startup_sweep.rs,progress_sse.rs,events.rs}` + a migration widening run kinds.
- Option B: a NEW `src-app/server/src/modules/background_job/{mod,models,repository,registry,tick,dispatch,events,handlers,routes,permissions}.rs` + migration `background_jobs`.
- Option C: `src-app/agent-core/src/{core.rs,ports.rs,types.rs}` (detached-turn seam) + a thin host registry.
- Sandbox: `src-app/server/src/modules/code_sandbox/{handlers.rs,streaming.rs}` + **submodule** `sdk/crates/ziee-sandbox/src/{sandbox.rs,tools/execute.rs,backend/mod.rs}` (detach the `Child` + guards; a poll/stream API).
- Completion notify: reuse `src-app/server/src/modules/notification/` + `sync` (`SyncEntity` addition), `src-app/server/src/modules/sync/`.
- FE: a background-work **right-panel** renderer (`registerPanelRenderer`) under `src-app/ui/src/modules/chat/...`, plus a `check status` affordance.

**Group E — /loop + /schedule in chat**
- FE (primary): `src-app/ui/src/modules/chat/components/ChatInput.tsx` (+ a new chat extension registering the composer slot), `src-app/ui/src/modules/chat/extensions/text/components/TextInput.tsx` + `Text.store.ts` (only if slash-parsing, ITEM-19), reuse `src-app/ui/src/modules/scheduler/components/{ScheduleBuilder,ScheduledTaskFormDrawer,ScheduledTaskCard}.tsx` + `scheduler` stores.
- BE (thin): `src-app/server/src/modules/scheduler/{schedule.rs,models.rs,dispatch.rs,handlers.rs}` for `SelfPaced` (ITEM-21) + a "bind to existing conversation" input (ITEM-22); migration in `scheduler/migrations/`.

**Group G — agent self-task-management (TodoWrite-style)**
- `src-app/agent-core/src/{extension.rs,types.rs}` + a new `agent-core` extension for the task-list tool + re-injection (the core capability), so all hosts inherit it.
- Storage: per-conversation (chat) / per-run (workflow) — a new table or reuse a jsonb column (mirror `assistant_core_memory` / `conversation_summaries`); a migration in the owning module.
- FE renderer + SSE/content-block on the compose seams (`src-app/ui/src/modules/chat/...` + the proc-macro event/variant seams).

**Cross-cutting**
- `just openapi-regen` (BOTH `ui/` + `desktop/ui/`) for any new REST/type; desktop parity for any new chat/scheduler UI (desktop embeds the server, so the tick loop + backbone already run in-process).
- `src-app/server/src/modules/agent/` (`agent_admin_settings`) for any new orchestration tunable.

---

## Patterns to follow

- **Sub-agent delegate tool** → mirror the existing **built-in MCP tool** contract + `fan_out`'s already-correct "return summaries not transcripts"; the `workflow_mcp` `wf_<slug>` opaque-tool is the model of "one tool call hides an inner run." Guardrails mirror `agent_admin_settings` (`fan_out_max_*`).
- **Background backbone** → mirror **`workflow_runs` + `workflow/runner.rs`** (fire-and-forget `spawn_run`, heartbeat + `AbortOnDrop`, `mark_running` guarded transition) + **`workflow/registry.rs` + `progress_sse.rs`** (`RunHandle` broadcast, **snapshot-on-connect**, subscriber-cap 429) + **`workflow/startup_sweep.rs`** (fail orphans, spare `waiting`/`resumable`, re-drive). The **`scheduler` tick loop** (`FOR UPDATE SKIP LOCKED`, coalesced catch-up) is the model for any polling executor.
- **Completion-notify (away user)** → mirror `workflow/events.rs` (`SyncEntity::WorkflowRun`, `Audience::owner`, notify-only) + the durable **`notification` inbox** (its first producer is the scheduler).
- **Background sandbox** → mirror the workflow sandbox-step progress seam (`workflow/dispatch.rs` FIFO `$ZIEE_PROGRESS → mpsc → SSE` + `sandbox_progress.rs`) and the cancel-only `INFLIGHT` `AbortHandle` registry in `code_sandbox/streaming.rs`; respect `kill_on_drop` + the RAII guard set (own them for the child's full life).
- **/loop + /schedule** → mirror the scheduler's `ScheduleBuilder.tsx` (Once|Recurring, `datetime-local`, cron presets, tz-read-only) + `ScheduledTaskFormDrawer.tsx` (kit `Form` + `zod`, test-fire) + the composer slot precedent (`extensions/voice` `MicButton` at `toolbar_actions`); the prompt-dispatch seam is `scheduler/dispatch.rs::dispatch_prompt → StreamingService::start_generation`.
- **SSE events / content blocks** → register via the proc-macro compose seams (`compose_chat_stream_events` / `compose_message_content_variants`) — no central enum edit; validate from a **clean build** (B4: new compose variants can compile against a stale expansion).
- **Admin settings** → `agent_admin_settings` / `scheduler_admin_settings` singleton pattern (`id=true` PK, DB CHECK + handler `validate()`, REST GET/PUT gated `<x>::admin::{read,manage}`, sync entity, admin card).
- **Chat-vs-agent-core** → any chat-path work lands behind / coordinates with `ZIEE_CHAT_AGENT_CORE`; the workflow `kind: agent` host is the proven consumer to mirror for port impls.
- **Agent self-task-management (Group G)** → the tool set mirrors Claude Code's **current `Task` tools** (`TaskCreate`/`TaskUpdate`/`TaskGet`/`TaskList` — per-item create+patch + read-back; NOT legacy `TodoWrite`'s array-rewrite), backed by a **durable store** (a table / `assistant_core_memory`-style block, not the transcript); the two context mechanisms are (a) a **change-gated out-of-band `<system-reminder>`** re-render and (b) an **explicit `CompactionExtension` re-emit** — mirror both, don't fuse them. Behavioral rules + render rule (`active_form` for the in_progress item) copied verbatim from CC. See RESEARCH_SOTA.md §8.

---

## UI surfaces — checklist + JTBD (answer per surface; a skipped answer ships as a defect)

The feature (at full scope) exposes up to **four** UI surfaces. Each is scoped
against the plan checklist; final answers firm up once the human picks scope.

### Surface 1 — Chat composer Loop/Schedule affordance + merged dialog (Group E)
- **JTBD:** "While chatting, I want to say *do this every morning* / *remind me at 3pm* / *keep checking until X* without leaving the conversation." The user opens a small dialog from the composer, picks Once / Recurring / Self-paced, types the message, and it's bound to this chat.
- **Precedent:** the composer slot = `extensions/voice` `MicButton` (`toolbar_actions`); the dialog = the scheduler's `ScheduleBuilder` + `ScheduledTaskFormDrawer` (reuse, don't rebuild — divergence from the sibling is a bug).
- **Scale/cardinality:** one dialog; the "attached loops/schedules" list per conversation is bounded (admin `max_active_tasks_per_user`, default 20) → numbered `ListPagination` if needed. Reuse `ScheduledTaskCard`.
- **Responsive (~390px/tablet/desktop):** the dialog is a `Drawer` (already responsive in the scheduler module); verify at 390px, mirror the scheduler drawer's breakpoints.
- **Input economy:** timezone auto-detected + shown read-only (already in `ScheduleBuilder`); day-of-week multi-select (already); no raw-JSON — reuse the typed form.
- **Progress:** a live **Test** button (already in `ScheduledTaskFormDrawer`) shows the run's real output before saving.

### Surface 2 — Background-work right panel (Groups B/C)
- **JTBD:** "I kicked off something long (3 sub-agents / a long script); I want to keep chatting and glance at progress, then see the result when it's done." A right-panel tab shows each background task's live status/output and its final result.
- **Precedent:** `registerPanelRenderer` + `displayInRightPanel`; mirror the workflow per-run progress (`RunHandle` broadcast + snapshot-on-connect) so a reopened panel rehydrates. (No `workflow-run` panel exists yet — this is net-new but on the established seam.)
- **Scale:** N background tasks per conversation is small (bounded by concurrency caps); each task's output is capped (1 MiB sandbox / summary-only sub-agents).
- **Responsive:** side panel → mirror the `literature` screening panel's narrow behavior; use quiet `Tabs variant="line"` in the narrow panel (design-system rule J5).
- **Progress:** live status dot + streamed output + a terminal state; **not** a silent boolean spinner — show per-child / per-command status and itemized errors.

### Surface 3 — Delegated sub-agents activity card (timeline, Group A)
- **JTBD:** "When the agent farms work out to 3 helpers, I want to *see* that it did, watch them finish, and read the merged conclusion — not a wall of raw JSON."
- **Precedent:** the friendly-agent-surface "editorial rows" timeline + `literature/LiteratureToolResultCard` (priority-ordered claim-or-delegate over the raw `McpToolUseGroup`). Domain phrasing, not `fan_out`/`spawn_subagents`.
- **Scale:** child count bounded by `fan_out_max_threads`; render a cluster header + per-child status, collapse to the merged summary.
- **Progress:** per-child running/done pills → merged summary; the raw card stays available under progressive disclosure.

### Surface 4 — Admin orchestration settings (reuse)
- **JTBD:** an admin sets fleet-wide guardrails (max background concurrency, sub-agent threads/depth, whether background sandbox is allowed).
- **Precedent:** reuse/extend the existing `agent_admin_settings` card + `scheduler_admin_settings` — mirror `SettingsPageContainer` + `Card`; do not free-style.

> **Multi-instance note:** none of these surfaces is a split-pane/pop-out multi-instance
> view, so the multi-instance interaction-model rules don't bind here (the chat
> right-panel is per-conversation and already handled by the chat store).

---

## Architecture options (the extensibility question — Group D, pick ONE)

The human's requirement: **one general, extensible background-job + spawn-sub-agent
abstraction**, not per-case hacks. Three viable designs, all reusing existing
durable machinery to different degrees.

### Option A — Generalize `workflow_runs` into a "run" backbone
**Sketch:** treat every long-running unit (a sub-agent group, a background sandbox
command, a detached chat turn) as a `workflow_runs` row with a new invocation
kind/source; reuse `runner` spawn+heartbeat, `RunHandle` broadcast SSE +
snapshot-on-connect, `startup_sweep`, and `SyncEntity::WorkflowRun`.
- **Pros:** least new code; inherits the most battle-tested durability (heartbeat, orphan-reclaim, resumable, reconnect-safe SSE) for free; one run table to reason about; the `kind: agent` step already lives here.
- **Cons:** couples non-DAG work to a DAG runner's semantics (steps/outputs/token-caps) that don't all fit a bare sandbox command; overloads `workflow_runs` with rows that aren't workflows (schema + mental-model drift); the "run" concept leaks workflow vocabulary into sub-agents/sandbox.
- **Best when:** we want the fastest path and are willing to bend "workflow" to mean "any durable run."

### Option B — A new generic `background_job` module *(recommended default)*
**Sketch:** a first-class `background_jobs` table + a **`JobKind` trait/registry**
(each producer registers a kind — `subagent-group`, `sandbox-exec`, future kinds)
+ a tick/executor + per-job SSE (mirroring `workflow/registry.rs`) +
`notification`/`sync` completion + `startup_sweep`. Producers plug in **without
editing a central `match`** (the open-seam / extensibility win).
- **Pros:** cleanest boundary — "background job" is its own concept, not a bent workflow; a new long-running kind is an additive registration (satisfies the extensibility/modularity angles the codebase is graded on); doesn't overload `workflow_runs`; the `JobKind` trait is the exact "not a one-off hack" abstraction the human asked for.
- **Cons:** most upfront design + a new module (mirrors `workflow` + `scheduler` mechanics, so it's *replicating* proven patterns rather than reusing the instances); two run-registries to maintain (workflow's + jobs') unless workflow later folds onto it.
- **Best when:** we value a durable, reusable platform seam over minimal diff — the stated goal.

### Option C — An `agent-core` "detached turn" seam
**Sketch:** add a background/suspend-resume turn primitive to the loop itself
(the gate already yields `GateOutcome::Suspended`; `resumable` runs already
re-drive on boot). Model producers as agent turns/sub-turns reusing the durable
transcript.
- **Pros:** most agent-native; sub-agents + background turns fall out naturally; keeps orchestration inside the one shared loop (the core's whole thesis); smallest surface for the *agent* cases.
- **Cons:** the raw **sandbox-exec** case is not an "agent turn," so it needs a separate path anyway → doesn't fully unify; pushes durable-run concerns into `agent-core` (which is deliberately domain-free behind its ports); depends on the still-hardening chat-agent-core cutover.
- **Best when:** the scope is sub-agent-centric and we defer background sandbox.

### Recommendation — REVISED by the SOTA research (see RESEARCH_SOTA.md §5)

**A-substrate + B-facade + C-as-a-kind.** The research changed my initial "Option B
(new table)" answer. The prevailing SOTA pattern (DBOS, Restate, Inngest, and Goose —
Goose is Rust and is *actively collapsing* its split design into one
`execute(RecipeSource, ExecutionMode{Interactive,Background,SubTask})` primitive) is
**ONE durable-run primitive with a `kind` discriminator + decentralized kind
registration, where "spawn sub-agent" is just a run-kind.** ziee is checkpoint-camp on
Postgres — the same camp as **DBOS**, whose `workflow_status` table is architecturally
the same shape as ziee's existing `workflow_runs`. So:

- **Option A is the skeleton** — generalize `workflow_runs` with a `kind` discriminator + a compact typed per-kind jsonb payload (a background job = a 1-step run; a sub-agent = a run whose single step *is* the agent loop). Reuse the runner/heartbeat/`RunHandle` SSE/`startup_sweep`/notification already built.
- **Option B is the skin — KEEP its API + `JobKind` trait registry** (the uniform `spawn_background/check_status/collect_result` + decentralized registration, matching ziee's built-in-MCP registry culture) — **but back it by `workflow_runs`, NOT a new `background_jobs` table.** A second durable substrate = two orphan-sweeps / status models / SSE-sync-notification-retention pipelines — the exact fragmentation Goose is spending a cycle to delete.
- **Option C is one bone** — the agent-core "detached turn" is `JobKind::SubAgent`, not a standalone seam (standalone C doesn't cover non-agent background work and re-forks the split everyone is collapsing).

**Biggest risk:** semantic overload of `workflow_runs`. Mitigate with the `kind`
discriminator + compact typed jsonb (never kind-conditional nullable-column sprawl),
an *optional* step/journal, and **per-`JobKind` policies** for orphan-sweep / flap-cap /
concurrency / retention (a token-heavy LLM sub-agent ≠ a fire-and-forget export — copy
Goose's sub-agent caps: ≤25 turns / 5 min / ~10 concurrent / no recursion).

---

## Locked scope (human-selected)

Resolved via option pickers on 2026-07-19. These are the agreed decisions the Phase-2
plan-audit + Phase-3 tests will be written against.

- **LOCK-1 — Breadth:** full **A–E**, plus **all four** Group-F completeness capabilities: **ITEM-24 goal-seeking**, **ITEM-25 steer-running-agent**, **ITEM-26 unified inbox/dashboard** (ITEM-28 live-TODO rides here as a stretch), **ITEM-27 event-driven triggers**.
- **LOCK-2 — Backbone:** the **hybrid** — *A-substrate + B-facade + C-as-a-kind*: generalize `workflow_runs` with a `kind` discriminator; expose a uniform `spawn_background / check_status / collect_result` + a decentralized **`JobKind` trait registry**; `JobKind::SubAgent` is C. **No separate `background_jobs` table.** (Supersedes the A/B/C options above — those remain as recorded rationale.)
- **LOCK-3 — Hardening baseline:** **ITEM-29–32 are in scope, not optional** — persisted task state machine + boot orphan-reclaim + `needs_input` reply state (29); sandbox output backpressure/ring-buffer (30); sandbox lifetime policy + cgroup-kill + terminal-state reaping (31); untrusted-output scanning of child summaries (32). Fold each in wherever its group (B/C/A) ships.
- **LOCK-4 — Sequencing:** **A + E first** (surface the existing `fan_out` + `scheduler` engines) → **backbone** (D) → **B + C** on it. Group-F items attach to their host group in that order (untrusted-scan 32 with A; goal-seeking 24 with E; state-machine 29 + steer 25 + inbox 26 + triggers 27 with the backbone/B; sandbox hardening 30–31 with C).

### Still-open (smaller — resolvable by convention at Phase 2/4, non-blocking)

- **ITEM-33** (Group-A ergonomics stretch: named agent defs / cumulative spawn budget / streaming child progress / per-child sandbox mode) — kept as a **[stretch]**; not selected. Revisit after A ships.
- **ITEM-18 vs ITEM-19** — `/loop`+`/schedule` entry: composer "+"-menu button vs a slash-command parser vs both. SOTA is NL-first, so the button + NL is the default; a slash parser is optional sugar. Resolve at Phase 4.
- **ITEM-21** self-paced mechanics — include the mirror set (show next-delay + reason, self-stop, ~7-day max-horizon backstop). On-SOTA; default yes.
- **ITEM-22** bind-to-current-conversation — post loop/schedule results into the current chat (vs the scheduler's per-task bound conversation). Default yes for the in-chat entry; confirm at Phase 4.

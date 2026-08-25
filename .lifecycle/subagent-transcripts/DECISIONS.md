# DECISIONS — Surface agent transcripts for background runs and fan-out children

All product choices resolved up front. The four LOCKED owner decisions are
recorded verbatim (DEC-1..4); everything else resolves by codebase convention.
No `AskUserQuestion` is needed — the owner already locked every genuine product
choice.

### DEC-1: Which agent kinds get transcripts?
**Resolution:** BOTH background runs AND fan-out children, shipped in ONE feature. (Owner-LOCKED verbatim: "Ship BOTH background runs + fan-out children in ONE feature.")
**Basis:** user — locked by the owner.

### DEC-2: What do fan-out children show?
**Resolution:** the FULL per-child transcript — thinking + tool calls/results + messages, not just a result summary. (Owner-LOCKED verbatim: "Fan-out children show the FULL per-child transcript (thinking + tool calls/results + messages).")
**Basis:** user — locked by the owner.

### DEC-3: How are child rows retained / cleaned up?
**Resolution:** child run rows CASCADE-delete with their parent and reuse the existing `workflow_runs` retention — NO new admin retention setting. (Owner-LOCKED verbatim: "Child run rows CASCADE-delete with their parent and reuse existing workflow_runs retention — NO new admin retention setting.")
**Basis:** user — locked by the owner.

### DEC-4: Where does the child drill-in live?
**Resolution:** INLINE in the existing `SubAgentActivityCard` (each child row lazy-expands → `AgentActivityTimeline`), NOT a separate drawer. (Owner-LOCKED verbatim: "The child drill-in transcript lives INLINE in the existing SubAgentActivityCard (lazy expand → AgentActivityTimeline), NOT a separate drawer.")
**Basis:** user — locked by the owner.

### DEC-5: What `job_kind` value do child rows carry?
**Resolution:** `'subagent'` — the value ALREADY in the `workflow_runs_job_kind_check` CHECK constraint (migration 202607190700). No CHECK change.
**Basis:** codebase — `202607190700_workflow_job_kind.sql:34` already lists `('workflow','sandbox_exec','subagent')`.

### DEC-6: Exact endpoint paths / names / home module?
**Resolution:** `GET /api/subagent-runs?parent_message_id={uuid}` (list children: id, label, status, created_at) + `GET /api/subagent-runs/{child_id}` (detail incl. `activity[]`). Both live in the **workflow** module (`workflow/routes.rs` + a new `workflow/subagent_runs.rs`) because child rows ARE `workflow_runs`. The DETAIL handler REUSES `get_background_run_detail` (a `subagent` child is background-kind `job_kind<>'workflow'`, so it already owner-scopes + 404s + returns `activity` after ITEM-3); only the LIST-by-`parent_message_id` needs a new repo fn.
**Basis:** convention — mirrors `background_mcp/runs.rs` (owner-scoped typed REST, `get_with`/docs), and reuses the existing detail getter rather than duplicating it.

### DEC-7: chat-vs-run parent linkage mechanics?
**Resolution:** for the ONLY host wired this round (chat fan-out), the child row carries TWO links: `parent_message_id` (the parent assistant `message_id`) — a PLAIN column, the list query key (`WHERE parent_message_id = $1`); and `parent_conversation_id` — FK `conversations(id) ON DELETE CASCADE`, the lifecycle link. `parent_run_id` is NOT added (no workflow/background host fans out through the isolated child path — DEC-11 — so it would be dead). DESIGN explicitly delegated the linkage choice to DECISIONS.
**Basis:** codebase — see the DEC-7 CORRECTION below; the design deferred this.

## DEC-7 CORRECTION — why `parent_conversation_id` (not a `messages` FK) carries the cascade
Verified by RUNNING the schema (DRIFT-1.1): `messages` has NO FK to `conversations`, and `delete_conversation` relies purely on FK cascade — so a `parent_message_id REFERENCES messages ON DELETE CASCADE` (the original DEC-7) would NOT delete a child when its CONVERSATION is deleted (only when the message itself is, which conversation-delete does not do). The dedicated `parent_conversation_id` FK (ON DELETE CASCADE to `conversations`) is therefore the DEC-3 lifecycle guarantee; `parent_message_id` stays a plain query key (owner-scope + `job_kind='subagent'` provide the security). TEST-8 proves the conversation-delete cascade.

### DEC-8: index columns for the children lookup?
**Resolution:** a partial btree index `idx_workflow_runs_parent_message ON workflow_runs (parent_message_id) WHERE parent_message_id IS NOT NULL` — the list query filters on `parent_message_id`. `parent_conversation_id` needs no dedicated index (it is used only by the FK cascade, which uses the PK/FK machinery, and conversation deletes are rare).
**Basis:** convention — mirrors `idx_workflow_runs_conv` (partial index WHERE NOT NULL) in 202607140230.

### DEC-9: no new retention setting — rationale?
**Resolution:** child rows are ordinary `workflow_runs` rows (`job_kind='subagent'`), so the EXISTING workflow-run retention prune already covers them; and `messages`-FK `ON DELETE CASCADE` removes them when the parent message/conversation is deleted. No `subagent_retention_days` settings row is introduced. This is the fixed/reuse choice mandated by the owner (DEC-3).
**Basis:** user (DEC-3) + codebase — reuses the existing retention + FK-cascade machinery.

### DEC-10: `ChildSink` port shape (agent-core, domain-free)?
**Resolution:** `trait ChildSink { async fn for_child(&self, child_run_id: Uuid, label: &str) -> Arc<dyn EventSink>; async fn settle_child(&self, child_run_id: Uuid, ok: bool); }`. NO `ChildParentRef` type — the host bakes the parent identity (conversation/message/user/model) into the CONCRETE factory at construction, so the crate passes back only the child `run_id` + `label` it already owns. This is the maximally domain-free shape (only `Uuid`/`&str`/`Arc<dyn EventSink>` cross the boundary), upholding INV-5.
**Basis:** convention — mirrors the existing optional-injected-port pattern (`SteerNotePort`/`SchedulePort` in `ports.rs`); a `settle_child` hook is added so a child-RUN error/panic (which emits no `Stopped`) still marks the row terminal.

### DEC-11: which hosts inject the factory?
**Resolution:** ONLY hosts that set `isolate_children=true` — the CHAT host (`dispatcher.rs:221`) — because the factory swap lives inside the `isolate_children` block in `fanout.rs`. Workflow/background hosts keep `child_sink_factory: None` (their children are non-isolated and their fan-out already rolls into the parent run's own timeline; unchanged).
**Basis:** codebase — `fanout.rs:273` isolate block is the only sink-swap site; `agent_dispatch.rs:1038` (workflow/background) has `isolate_children:false`.

### DEC-12: `PersistingActivitySink` location + WorkflowEventSink reuse?
**Resolution:** new file `workflow/activity_sink.rs` holding the shared `fn map_agent_event(&AgentEvent) -> Vec<MappedActivity>` (Activity | Log) + `pub(crate) struct PersistingActivitySink { pool, run_id, step_id, seq }`. `WorkflowEventSink` (agent_dispatch.rs) is refactored to consume `map_agent_event` (Activity → SSE+persist, Log → SSE log-line), staying byte-identical; `PersistingActivitySink` persists ONLY the Activity entries.
**Basis:** convention — one mapping, two sinks (DRY per ITEM-1); mirrors `append_agent_activity` reuse.

### DEC-13: `BackgroundRunDetail.activity` field type?
**Resolution:** `pub activity: Vec<ProgressKind>` (the persisted entries ARE serialized `ProgressKind::AgentActivity`; the FE type is `AgentActivityEntry = Extract<ProgressKind,{type:'agent_activity'}>`, so `Vec<ProgressKind>` is the exact matching schema — NO new Rust type). `get_background_run_detail` switches from `query_as!(BackgroundRunDetail,…)` to `sqlx::query!` + manual struct build, projecting `coalesce(step_logs_json -> 'agent::agent_activity','[]'::jsonb)` and `serde_json::from_value` into `Vec<ProgressKind>`.
**Basis:** codebase — `query_as!` requires every field be a column; a manual build is the least-typed-churn way to add a derived jsonb projection while keeping the DTO clean.

### DEC-14: permission gate — new or reused?
**Resolution:** REUSE the existing `WorkflowsRead` permission (`workflows`/`read`), granted to the **Users** system group (202607146095), so every chat user holds it. Owner-scope is by `user_id` on the child row (foreign/missing → 404). NO new permission is defined and none is granted, so A9 (backend deny) and A10 (restricted-user e2e) do NOT apply — CONFIRMED. The security boundary is owner-scope (INV-4), not a bespoke perm.
**Basis:** codebase — `202607146095` grants `workflows::read` to Users; reusing it introduces nothing.

### DEC-15: background persisting sink step id?
**Resolution:** the fixed step id `"agent"` → persists to `step_logs_json['agent::agent_activity']` on the background run's own row.
**Basis:** convention — mirrors the workflow `kind: agent` step's `<stepId>::agent_activity` key; a background run has exactly one logical agent step.

### DEC-16: do resolution-failed (never-spawned) children get a persisted row?
**Resolution:** NO — a child whose model resolution fails (`continue` before the isolate block in `fanout.rs`) never calls `for_child`, so no `workflow_runs` row is created (it has no transcript to view). Only spawned children get a row. The live SSE card still shows it `failed`.
**Basis:** codebase — `fanout.rs:238` `continue` is BEFORE the sink-swap block; a never-ran child has no activity.

### DEC-17: child settle status vocabulary?
**Resolution:** insert with `status='running'`; `settle_child(ok=true)` → `'completed'`, `settle_child(ok=false)` → `'failed'`. These match the existing `workflow_runs` status vocabulary used by background runs.
**Basis:** codebase — the background-run status set (running/completed/failed) already exists.

### DEC-18: migration prefix?
**Resolution:** `202608250100_subagent_child_runs.sql` in `workflow/migrations/` — above the floor `202608210100`, unique, server 2026… sequence (not the desktop 1e13 block).
**Basis:** codebase — BASE.md floor + `find … | sort` uniqueness check.

### DEC-19: does the feature introduce any operational tunable (configurable-settings rule)?
**Resolution:** NO new operational tunable is introduced. Child count is bounded by the EXISTING `SubagentLimits.max_children_per_call` (8); the per-step activity cap is the EXISTING `AGENT_ACTIVITY_MAX_ENTRIES` (500) reused verbatim; retention reuses the existing workflow-run prune (DEC-3/DEC-9). No memory/cpu/timeout/quota/threshold is added, so no settings row / REST / sync / admin card is warranted. The reused constants are existing scale/security bounds, structured as `SubagentLimits`/named consts already.
**Basis:** convention — the configurable-settings DEC is satisfied by "no new tunable"; nothing is shipped as a bare new magic number.

### DEC-20: any genuine product choice needing an AskUserQuestion?
**Resolution:** NO. The four genuine product choices are LOCKED by the owner (DEC-1..4). Every remaining decision (paths, linkage, port shape, types, permission reuse) is resolved by codebase convention above. Implementation runs nonstop.
**Basis:** user — the owner pre-locked the product surface; the rest is convention.

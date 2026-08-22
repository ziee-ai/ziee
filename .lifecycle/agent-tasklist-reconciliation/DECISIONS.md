# DECISIONS — agent task-list terminal-state reconciliation

### DEC-1: What terminal status should a genuinely-unfinished task become at run end?
**Resolution:** a NEW value `abandoned` (not `completed`, not `cancelled`).
`completed` would falsely claim the work was done; `cancelled` collides with the
run-level cancel meaning. `abandoned` reads as "the run ended before this task was
finished," uniformly across completed/failed/cancelled/crashed runs.
**Basis:** convention — the task brief asks for "something honest like
`abandoned`/`cancelled`, NOT silently flipped to `completed`."

### DEC-2: Add `TaskStatus::Abandoned` to agent-core, and expose it to the model?
**Resolution:** Add the variant to `agent-core::TaskStatus` (so a store read
round-trips honestly), map it in the server `status_to_str`/`status_from_str`, and
render it `[-]`. Do NOT add it to `status_schema()` (the model-settable enum stays
`pending|in_progress|completed`).
**Basis:** codebase — `status_from_str` degrades an unknown DB value back to
`Pending` (task_list.rs:62-68), so without the variant the fix is invisible on any
store read (re-injection / resume). Abandonment is a system reconciliation act, not
a model-issued status, so it is read-only in the tool vocabulary.

### DEC-3: Add a foreign key on `run_id`?
**Resolution:** NO — `run_id` is polymorphic and cannot carry a single-target FK.
Instead add a SEPARATE nullable `workflow_run_id uuid REFERENCES workflow_runs(id)`
column, populated (via an existence-guarded subquery) only when the run is a real
`workflow_runs` row; chat/fan-out rows keep it NULL.
**Basis:** codebase — verified `run_id` = `assistant_message_id` for chat
(`chat/agent_host/dispatcher.rs:241`), `workflow_runs.id` for workflow/background
(`workflow/agent_dispatch.rs`), and a fresh non-persisted uuid for fan-out
children. A plain `FK REFERENCES workflow_runs(id)` would fail every chat/fan-out
INSERT. The separate-column pattern mirrors `mcp_tool_calls.workflow_run_id` /
`scheduled_task_runs.workflow_run_id`.

### DEC-4: ON DELETE CASCADE or SET NULL for `workflow_run_id`?
**Resolution:** `ON DELETE CASCADE`.
**Basis:** codebase — the task list is ephemeral run-scoped WORKING state (the
agent's checklist), analogous to `background_run_notes.run_id` and
`file_workflow_runs` which both CASCADE. `SET NULL` is used for
history/audit rows (`mcp_tool_calls`, `scheduled_task_runs`) that must outlive the
run; task-list rows have no such requirement — history need not survive a run
hard-delete.

### DEC-5: Where to reconcile — per runner arm, or at the shared writers?
**Resolution:** at the three shared terminal writers: `mark_status` (all
runner completed/failed/cancelled/timeout arms route through it), `cancel_cas`
(user cancel bypasses `mark_status`), and a bulk sweep in `sweep_at_boot`
(crash-recovery + retroactive). Not per-arm.
**Basis:** codebase — `mark_status` is the single guarded CAS chokepoint; hooking
the shared writers is robust against a missed arm (the exact failure the brief
warns about).

### DEC-6: Reconcile the chat agent-host terminal path?
**Resolution:** DESCOPED this round — the primitive is keyed by `run_id` and
already supports the chat key shape, but no live hook is wired in chat.
**Basis:** codebase — the chat agent-core path is behind the non-default
`ZIEE_CHAT_AGENT_CORE=1` flag (`chat/core/services/streaming.rs:898`,
experimental "for behavioral verification"), unmeasured by the rig, and its
message-id-keyed rows have no crash-recovery model.

### DEC-7: Is any operational tunable introduced (retention / caps / thresholds)?
**Resolution:** No admin-configurable settings row. Reconciliation has no tunable:
the `abandoned` vocabulary is a fixed data-integrity value, cascade cleanup is
immediate on run delete (no retention window), and the sweep runs on the existing
boot schedule. No new `*_settings` table.
**Basis:** convention — this is a correctness/data-integrity fix, not an
operational feature; there is nothing an operator would tune. (Configurable-settings
rule considered and deliberately declined.)

---

- DESCOPED: ITEM-7 — chat agent-host live reconciliation hook: flag-gated (ZIEE_CHAT_AGENT_CORE=1) non-production path, unmeasured by the rig, no crash-recovery model; the run_id-keyed primitive already supports it if the path graduates [approved: orchestrator 2026-08-21 — out of measured/production scope]

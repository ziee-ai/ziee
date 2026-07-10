# BASE — conflict surface vs current origin/main

Branch cut from `origin/main` @ `e253510f9` (fresh worktree, pulled today).

## Highest existing migration
`ls src-app/server/migrations | tail -1` → `00000000000145_create_conversation_deliverables.sql`.
**This branch adds migration `146`** (`00000000000146_scheduled_task_unattended_tools.sql`):
`scheduled_tasks.allowed_unattended_tools JSONB DEFAULT '[]'` +
`scheduled_task_runs.skipped_tools JSONB DEFAULT '[]'` (per DEC-19, for the unattended
tool-policy work ITEM-15/17). The bug-fix items (5-11) + FB items (1-4,12) still add no
schema (new `paused_reason` values are free TEXT; run-prune reuses the existing
`notification_retention_days`). 146 is the next free number vs main's 145 → the merge-gate
C2 re-checks for a collision if main advances before merge.

## Files this branch edits that main may also touch
- `src-app/server/src/modules/scheduler/**` — the scheduler module. It was merged as
  `scheduled-background-tasks`; no other in-flight branch is known to be editing it. Low
  collision risk; re-checked by the merge-gate at merge time.
- `src-app/ui/src/modules/scheduler/**` — same.
- Shared read-only reuse (NOT edited): `assistant/**`, `workflow/**`, `chat/components/ModelSelector`,
  `user-llm-providers/ModelPicker` store, `projects/components/ProjectFormDrawer`. These are
  imported/mirrored, not modified.

## OpenAPI regen implied?
**Yes — for the unattended tool-policy work only.** The FB/picker items + bug fixes (1-12)
need no type change (the three picker fields already exist on `CreateScheduledTask`;
`completed` reuses `paused_reason`). But ITEM-15/17 add `allowed_unattended_tools` to
`Create/UpdateScheduledTask` and `skipped_tools` to `ScheduledTaskRun` → `just openapi-regen`
(BOTH `ui/` + `desktop/ui/`) is required; the merge-gate C3 re-checks regen parity for both
workspaces. The unattended `unattended`/`invocation_source` chat-request flag is threaded
in-process (not necessarily on the public request type) — keep it off the OpenAPI surface if
possible to minimize the regen delta.

## Notes
- Config tunables introduced: run-history retention (reuses existing `notification_retention_days`
  setting — see DEC), transient-retry attempts/backoff (fixed Limits struct — see DEC). No new
  settings row, no new permission (scheduler perms already exist) → no A9/A10 obligation.

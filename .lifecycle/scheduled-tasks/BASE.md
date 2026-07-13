# BASE — conflict surface vs current origin/main

Branch cut from `origin/main` @ `e253510f9` (fresh worktree, pulled today).

## Highest existing migration
`ls src-app/server/migrations | tail -1` → `00000000000145_create_conversation_deliverables.sql`.
**This branch adds migration `153`** (`00000000000153_scheduled_task_unattended_tools.sql`):
`scheduled_tasks.allowed_unattended_tools JSONB DEFAULT '[]'` +
`scheduled_task_runs.skipped_tools JSONB DEFAULT '[]'` (per DEC-19, for the unattended
tool-policy work ITEM-15/17). The bug-fix items (5-11) + FB items (1-4,12) still add no
schema (new `paused_reason` values are free TEXT; run-prune reuses the existing
`notification_retention_days`). 153 is the next free number vs main current max 152 → the merge-gate
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

---

## ROUND 2 — Follow-up & Series (re-based on CURRENT origin/main @ 304f4a011)

Current `ls src-app/server/migrations | tail -1` → `00000000000154_add_voice_streaming_settings.sql`
(main advanced to 154; this branch's own 153 already merged). **Round 2 adds migration `155`**
(`00000000000155_scheduled_task_run_result_preview.sql`): `scheduled_task_runs.result_preview TEXT NULL`
+ `change_summary_json JSONB NULL` (ITEM-40). 155 is the next free number vs current max 154 →
merge-gate C2 re-checks for a collision if main advances before merge.

**Files edited (Round 2):** same `scheduler/**` module (server + ui) as before — still the sole
editor; low collision risk. NEW read-only reuse: `chat/extensions/file/processor.rs`
(`process_file_blocks`, imported for artifact ContentBlocks — not modified) and a paginated-list
handler as the pagination precedent.

**OpenAPI regen implied?** YES — ITEM-40 adds `result_preview`/`change_summary_json` to
`ScheduledTaskRun`, ITEM-41 turns the run-list response into a paged envelope, ITEM-43 adds the
`continue-series` route + response. `just openapi-regen` (BOTH `ui/` + `desktop/ui/`); merge-gate
C3 re-checks parity. No new permission (reuses `scheduler::use`/`scheduler::admin`) → no A9/A10
obligation beyond the `continue-series` owner-scope 404 (the A9 deny path, TEST-46).

# BASE — conflict surface vs current origin/main

Branch cut from `origin/main` @ `e253510f9` (fresh worktree, pulled today).

## Highest existing migration
`ls src-app/server/migrations | tail -1` → `00000000000145_create_conversation_deliverables.sql`.
**This branch adds NO migration** — all fixes reuse existing columns (`paused_reason` is
free TEXT, so new reason values `completed`/`conversation_deleted` need no schema change;
run-history prune reuses the existing `scheduler_admin_settings.notification_retention_days`).
→ No migration-number collision possible.

## Files this branch edits that main may also touch
- `src-app/server/src/modules/scheduler/**` — the scheduler module. It was merged as
  `scheduled-background-tasks`; no other in-flight branch is known to be editing it. Low
  collision risk; re-checked by the merge-gate at merge time.
- `src-app/ui/src/modules/scheduler/**` — same.
- Shared read-only reuse (NOT edited): `assistant/**`, `workflow/**`, `chat/components/ModelSelector`,
  `user-llm-providers/ModelPicker` store, `projects/components/ProjectFormDrawer`. These are
  imported/mirrored, not modified.

## OpenAPI regen implied?
**No.** No request/response type changes: the three picker fields
(`assistant_id`/`workflow_id`/`model_id`) already exist on `CreateScheduledTask`; the
"completed" signal reuses the existing `paused_reason: Option<String>` field; retry/prune/
validation are internal. `openapi.json` + `api-client/types.ts` are untouched → the phase-3/8
frontend gates still fire on the real UI diff, and the merge-gate's C3 regen-parity is a no-op.

## Notes
- Config tunables introduced: run-history retention (reuses existing `notification_retention_days`
  setting — see DEC), transient-retry attempts/backoff (fixed Limits struct — see DEC). No new
  settings row, no new permission (scheduler perms already exist) → no A9/A10 obligation.

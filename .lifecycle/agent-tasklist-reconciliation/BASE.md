# BASE — conflict-surface scoping

- Branch base: `origin/main` = `db2347928`.
- Highest existing server migration prefix: `202607200600`
  (`find src-app/server -path '*/migrations/*.sql' -printf '%f\n' | cut -d_ -f1 | sort -n | tail -1`).
  New migration `202608210100_agent_task_list_reconcile.sql` sorts above it and is
  unique (checked: no duplicate prefixes across `src-app`).
- Desktop migration sequence (`10000000000005` max) is untouched.
- Files this branch edits that main might also touch: `workflow/repository.rs`,
  `agent/task_list.rs`, `agent-core/{types,tasklist}.rs`. No evidence of concurrent
  main churn on these; the merge-gate re-checks against real main at merge time.
- **OpenAPI regen implied? NO** (DRIFT-1.1 briefly said YES, self-corrected). The
  wire DTO `TaskListItemStatus` stays 3-value (its documented purpose is to mirror
  the 3-value FE union; `abandoned` is a post-run DB state that never streams). A
  clean rebuild + regen produced zero diff to `openapi.json`/`types.ts`. No UI
  workspace touched → no UI gates.

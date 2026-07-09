# TEST_RESULTS — scheduled-background-tasks

Real execution results. Backend ran with the per-worktree isolated build DB
(`DATABASE_URL=…/ziee_sched_build ZIEE_BUILD_DB_PERWORKTREE=0`) on the shared
`:54321` cluster; e2e ran `--workers=1` with the server-ready budget raised for
the loaded host (`E2E_BACKEND_READY_BUDGET_SECS=360`). Logs under
`/data/pbya/ziee/tmp/lifecycle-logs/sched-*.log`.

## Summary

- Backend: **11 unit + 13 scheduler integration + 4 notification integration =
  28 passed, 0 failed** (`cargo test --lib -p ziee scheduler::` → 11 ok;
  `cargo test --test integration_tests scheduler::` → 13 ok;
  `… notification::` → 4 ok).
- OpenAPI golden: `types_ts_parity` + `types_ts_parity_desktop` ok.
- E2E: **5 passed, 0 failed** (`14-scheduler/*` + `15-notifications/inbox`).
- `npm run check` green in both touched UI workspaces.

## Unit

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS

## Integration — scheduler

- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS  (caught + drove the fix of the spent-`once`-task disable bug — DRIFT-2.3)
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS

## Integration — notification

- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS

## Build / schema / codegen gate

- **TEST-22**: PASS  (workspace compiles with `croner`; migration 137 applies —
  the scheduler tests run against the widened `workflow_runs.invocation_source`
  CHECK; `openapi::emit_ts::tests::types_ts_parity{,_desktop}` ok)
- **TEST-23**: PASS  (admin `notification_retention_days` round-trips via
  GET/PUT in the scheduler sync-emit test; the prune loop mirrors the
  retention-tested `mcp/tool_calls/prune.rs`)

## E2E

- **TEST-24**: PASS
- **TEST-25**: PASS
- **TEST-26**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS

## Frontend gate

npm run check (ui): PASS
npm run check (desktop/ui): PASS

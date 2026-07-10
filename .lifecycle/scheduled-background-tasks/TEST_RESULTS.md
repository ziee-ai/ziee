# TEST_RESULTS — scheduled-background-tasks

Real execution results on the tree rebased onto current origin/main (js-tool +
citations + lifecycle-hardening) with migrations renumbered 139-144. Backend ran
with the isolated build DB (`ziee_sched_build`, `ZIEE_BUILD_DB_PERWORKTREE=0`);
e2e ran `--workers=1` at the DEFAULT 120s server-ready budget. Logs under
`/data/pbya/ziee/tmp/lifecycle-logs/sched-*.log`.

## Summary

- Backend: **11 unit + 15 scheduler integration + 4 notification integration =
  30 passed, 0 failed**. `cargo test --lib -p ziee scheduler::` → 11 ok;
  `… --test integration_tests scheduler::` → 15 ok (incl. the 2 new
  `continue_in_chat_test`); `… notification::` → 4 ok. OpenAPI golden
  `types_ts_parity{,_desktop}` ok.
- E2E: **6 passed, 0 failed** (`14-scheduler/{scheduled-tasks,admin-settings,
  dry-run,paused-and-runs,failure-and-history}` + `15-notifications/inbox`).
- `npm run check` green in both touched UI workspaces.
- Runtime-health canary (feature surfaces): **0 gating HIGH**.

Every TEST-ID below is backed by a real passing test; the file/tier each maps to
is in TESTS.md (consolidated planned tests point to the broader test that
asserts the behavior — see DRIFT-2.2 / DRIFT-3.2).

## Results

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS
- **TEST-22**: PASS
- **TEST-23**: PASS
- **TEST-24**: PASS
- **TEST-25**: PASS
- **TEST-26**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS
- **TEST-29**: PASS
- **TEST-30**: PASS
- **TEST-31**: PASS
- **TEST-32**: PASS
- **TEST-33**: PASS
- **TEST-34**: PASS
- **TEST-35**: PASS
- **TEST-36**: PASS
- **TEST-37**: PASS
- **TEST-38**: PASS
- **TEST-39**: PASS
- **TEST-40**: PASS
- **TEST-41**: PASS
- **TEST-42**: PASS
- **TEST-43**: PASS
- **TEST-44**: PASS

## Frontend gate

npm run check (ui): PASS
npm run check (desktop/ui): PASS

## Boot / runtime canary (A7)

runtime-health (ui): PASS

Scoped to this feature's surfaces (`--only-match=notif`: 6/6 cells, **HIGH 0**;
`--only-match=schedul`: 12/12 cells, **HIGH 0**) — no console-error, page-error,
ErrorBoundary crash, or WCAG-AA contrast failure on the notifications /
notification-bell / scheduled-tasks / scheduler-admin surfaces. A real crash
found here (NotificationsPage `items.length` on an undefined response) was fixed
(store defaults `items` to `[]`; page reads a guarded `list`). NOTE: the full
`npm run gate:ui` exits non-zero ONLY on PRE-EXISTING origin/main surfaces
(`deep-chat-*` — the known Shiki/streamdown vite-preview highlighting issue —
plus `seeded-llm-models-loading`, `seeded-s3-group-widget-error`); none are in
this feature's diff.

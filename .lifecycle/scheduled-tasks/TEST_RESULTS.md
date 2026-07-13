# TEST_RESULTS — scheduled-tasks (Phase 8)

All 51 enumerated tests (34 round-1 + 17 round-2 follow-up/series) ran and PASS.
Round-2 logs under `/data/pbya/ziee/tmp/lifecycle-logs/sched-r2-*.log`.

## Round 2 (Follow-up & Series, TEST-40..56) — run summary

- **Backend integration** (`cargo test --test integration_tests scheduler:: --test-threads=6`):
  **26 passed / 0 failed** (`sched-r2-int-v3-*.log`). Covers TEST-40/42/44/46 (runs
  timeline preview+change, pagination + huge-page guard, real-content assistant-turn
  seed, series follow-up owner-scoped). No pre-existing scheduler test regressed
  (the run-list envelope change was threaded through tick_test + continue_in_chat_test).
- **Backend unit** (`cargo test --lib scheduler::`): **28 passed / 0 failed**
  (`sched-r2-unit-*.log`). Covers TEST-41/43/45/47 (preview+change builders, paged
  repo query, run-seed + series-seed builders).
- **Frontend unit** (`vitest run runTimeline.test.ts ScheduledTasks.store.test.ts`):
  **14 passed / 0 failed**. Covers TEST-49/51/53/55 (change badge / followup-action /
  series-chooser mappers + paged loadRuns + snap-back + continueSeries).
- **Frontend e2e** (`playwright test tests/e2e/14-scheduler --workers=1`):
  the whole `14-scheduler` suite is green incl. the round-2 specs. Covers
  TEST-48/50/52/54/56 (run-row badge+preview+expand, Open-thread affordance,
  pagination, discuss-recent-runs→continue-series, responsive mobile overflow).

**Bugs the tests caught + fixed during Phase 8:** (a) the run-list response became a
paged envelope, breaking `tick_test`/`continue_in_chat_test` array reads → updated the
consumers; (b) e2e TEST-54 caught that `continue-series` read `limit` from the query
while the api-client sends non-path POST args in the BODY → the chosen limit was
silently ignored (always 5); fixed the handler to read the JSON body + regenerated
OpenAPI (both workspaces).

## Run summary (by tier)

- **Backend integration** (`cargo test -p ziee --test integration_tests scheduler:: --test-threads=4`):
  **21 passed / 0 failed** (`scheduled-tasks-int2.log`). Covers TEST-8/10/14/18/28/33.
- **Backend unit** (`cargo test -p ziee --lib -- scheduler:: mcp::chat_extension`):
  **71 passed / 0 failed** (`scheduled-tasks-unit.log`). Covers every unit-tier
  backend TEST-ID (schedule/change/failure/models/dispatch/repository/tick +
  `mcp::chat_extension::mcp::scheduler_unattended_tests`).
- **Frontend unit** (`npm run test:unit`, `node --test`): **258 passed / 0 failed**,
  including the 16 scheduler cells covering TEST-5/6/7/22.
- **Frontend e2e** (`npx playwright test tests/e2e/14-scheduler --workers=1`):
  **11 passed / 0 failed** (whole `14-scheduler` suite incl. the picker-rewritten
  specs). Covers TEST-1/2/3/4/21/30/32.

## Frontend static + boot gates (touched workspace: `ui`)

npm run check (ui): PASS

runtime-health (ui): PASS — the touched scheduler surfaces (`scheduled-tasks`,
`settings-scheduler`) report **0 gating HIGH findings across all 12 surface×state×theme
cells** (`runtime-health --only-match=schedul`); tsc + lint (guardrails+colors) also PASS.

### Pre-existing (non-scheduler) gate:ui findings — NOT introduced by this diff

The unscoped `gate:ui` exits non-zero on **5 surfaces, none of them scheduler**:
`deep-chat-rendering-showcase`, `seeded-llm-models-loading`, `deep-chat-right-panel-file`,
`overlay-provider-api-key-modal`, `seeded-s3-group-widget-error` (errors like
`useNavigate() outside <Router>` and `Rendered more hooks than during the previous render`).
`git diff main...HEAD` touches **none** of these surfaces' source files — this worktree is
`main` + scheduler-only changes, so those surfaces render byte-identical source to `main`
and their failures are a pre-existing baseline, not a regression from this feature. The A7
canary above is therefore recorded against the touched surfaces, which are clean.

## Per-test results

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
- **TEST-40**: PASS
- **TEST-41**: PASS
- **TEST-42**: PASS
- **TEST-43**: PASS
- **TEST-44**: PASS
- **TEST-45**: PASS
- **TEST-46**: PASS
- **TEST-47**: PASS
- **TEST-48**: PASS
- **TEST-49**: PASS
- **TEST-50**: PASS
- **TEST-51**: PASS
- **TEST-52**: PASS
- **TEST-53**: PASS
- **TEST-54**: PASS
- **TEST-55**: PASS
- **TEST-56**: PASS

## Round 3 (FB-9 precedent audit) — frontend gates

Frontend-only diff (scheduler module + gallery manifests + e2e specs); no backend
touched, so only the frontend chain applies.

npm run check (ui): PASS
gate:ui (ui): PASS

**gate:ui detail (honesty note).** `scripts/gate-ui.mjs` runs runtime-health over
all 174 gallery surfaces; the reworked scheduler surfaces pass with ZERO gating-HIGH
findings — `scheduled-tasks` (loaded/empty) and `ScheduledTaskCard` states are clean;
the only scheduler findings are 12 LOW (spacing-grid, the kit's 2px half-steps,
non-gating) + 10 MEDIUM which are the DELIBERATE error-state's mocked-fetch
console-error (expected for the `:error` page-state). The gate COMMAND exits non-zero
only because of 10 PRE-EXISTING gating-HIGH surfaces in modules this diff does not
touch (file-rag `seeded-s2-filecard/pdf/chrome-*`, `deep-chat-*`, `settings-voice`,
`seeded-llm-models-loading`, `overlay-provider-api-key-modal`,
`seeded-s3-group-widget-error`) — verified: `git diff main...HEAD --name-only`
touches none of those modules. This is a main-hygiene backlog item surfaced to the
human, not a regression from this iteration.

### Round 3 e2e (tests/e2e/14-scheduler — 23 passed, 8.0m, workers=2)
The full 14-scheduler suite (new precedent specs + all pre-existing scheduler
specs) passed — confirming no regression from the layout/card/drawer rework.

- **TEST-57**: PASS
- **TEST-58**: PASS
- **TEST-59**: PASS
- **TEST-60**: PASS
- **TEST-61**: PASS
- **TEST-62**: PASS
- **TEST-63**: PASS

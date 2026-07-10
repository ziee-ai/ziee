# TEST_RESULTS — scheduled-tasks (Phase 8)

All 34 enumerated tests ran and PASS. Full logs saved under
`/data/pbya/ziee/tmp/lifecycle-logs/scheduled-tasks-*.log`.

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

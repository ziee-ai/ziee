# TEST_RESULTS — background-spawn-loop-guard

Backend-only diff (`src-app/server/**`) — no frontend workspace touched, so no
`npm run check` / `gate:ui` / e2e gates apply. Single full run of the enumerated
set (phase 8). Full logs:
`/data/pbya/ziee/tmp/lifecycle-logs/background-spawn-loop-guard-int.log`,
`/data/pbya/ziee/tmp/lifecycle-logs/bsg-unit-resume.log`.

- **TEST-1**: PASS — `background_mcp::spawn_guard::identical_inflight_spec_is_deduped_to_one_run` (acceptance, INV-1)
- **TEST-2**: PASS — `background_mcp::spawn_guard::over_cap_spawn_is_refused_and_creates_no_run` (acceptance, INV-2)
- **TEST-3**: PASS — `background_mcp::spawn_guard::completed_reinjection_does_not_respawn` (acceptance, INV-3; incl. the long-run created-1h-ago/completed-now case)
- **TEST-4**: PASS — `background_mcp::resume::tests::resume_message_carries_no_spawn_directive`
- **TEST-5**: PASS — `background_mcp::spawn_guard::first_spawn_of_a_spec_still_succeeds`
- **TEST-6**: PASS — `workflow::repository::tests::spawn_background_run_drives_to_terminal` (unguarded path unchanged)

Integration run (harness, per-test DBs): `test result: ok. 4 passed; 0 failed`.
Unit runs: resume `6 passed; 0 failed`; workflow repo test `1 passed; 0 failed`.
No `#[ignore]`/`.skip` added; no test soft-skipped.

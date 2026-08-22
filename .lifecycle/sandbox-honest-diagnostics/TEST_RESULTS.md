# TEST_RESULTS — sandbox honest diagnostics

Backend-only diff (`sdk/crates/ziee-sandbox/**` + `src-app/server/src/modules/code_sandbox/**`);
no frontend workspace touched, so no `npm run check` / `gate:ui` / e2e chain applies.

## Compile gates (real cargo exit, not `| tail` — pipefail-safe)

- `cargo check -p ziee --tests` (server crate, the stated gate): PASS — `ZIEE_CARGO_EXIT=0`, 0 errors
  (log: `/data/pbya/ziee/tmp/lifecycle-logs/shd-ziee-check.log`; post-fix re-check `shd-ziee-fix.log` `ZIEE_FIX_EXIT=0`).
- `cargo check -p ziee-sandbox --tests` (SDK crate, from the sdk workspace where dev-deps resolve): PASS — `SDK_CARGO_EXIT=0`, 0 errors
  (log: `/data/pbya/ziee/tmp/lifecycle-logs/shd-sdk-check.log`; post-fix re-check `shd-sdk-fix.log` `SDK_FIX_EXIT=0`).

## Enumerated acceptance tests (single full run — server crate)

`cargo test -p ziee --lib code_sandbox::honest_diagnostics_tests` — `ACC_EXIT=0`,
5 passed / 0 failed (log: `/data/pbya/ziee/tmp/lifecycle-logs/shd-ziee-acctests.log`).
These are the lifecycle-enumerated acceptance tests, filed in the superproject and
reaching the engine via the `code_sandbox::{config, sandbox, tools::execute}`
re-exports.

- **TEST-1**: PASS — `code_sandbox::honest_diagnostics_tests::test1_explain_gives_specific_honest_reason_per_variant`
- **TEST-2**: PASS — `code_sandbox::honest_diagnostics_tests::test2_not_initialized_error_states_the_real_reason`
- **TEST-3**: PASS — `code_sandbox::honest_diagnostics_tests::test3_classify_seccomp_write_splits_epipe_from_genuine_truncation`
- **TEST-4**: PASS — `code_sandbox::honest_diagnostics_tests::test4_redact_host_paths_scrubs_all_bind_sources_and_is_noop_otherwise`
- **TEST-5**: PASS — `code_sandbox::honest_diagnostics_tests::test5_redact_host_paths_scrubs_bwrap_dead_mount_stderr`

## SDK crate-level coverage (additional, not lifecycle-enumerated)

`cargo test -p ziee-sandbox --lib` (from the sdk workspace) — `TEST_EXIT=0`, 5
passed / 0 failed (log: `/data/pbya/ziee/tmp/lifecycle-logs/shd-unit-tests.log`):
the SDK's own in-source unit tests for `explain()`, `classify_seccomp_write`, and
`redact_host_paths`.

## Acceptance-test coverage (design invariants proven)

- INV-1 → TEST-1, TEST-2 (both PASS)
- INV-2 → TEST-3 (PASS)
- INV-3 → TEST-4, TEST-5 (both PASS)

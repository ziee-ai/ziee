# TEST_RESULTS

Backend-only diff (`src-app/server/**`) — only the backend test chain applies; no
frontend workspace touched, so no `npm run check` / `gate:ui` lines are required.

Full logs:
`/data/khoi/home-workspace/ziee/tmp/lifecycle-logs/sandbox-read-file-artifacts-int.log`
(unit run captured inline above it). Run with the server subprocess built from
this branch; integration under `--test-threads=1`.

## Results (every phase-3 TEST-ID)

- **TEST-1**: PASS  (`read_file_reads_model_authored_mcp_artifact`)
- **TEST-2**: PASS  (`read_file_reads_model_authored_llm_artifact`)
- **TEST-3**: PASS  (`edit_file_edits_model_authored_artifact`)
- **TEST-4**: PASS  (`read_file_does_not_read_artifact_from_another_conversation`)
- **TEST-5**: PASS  (`read_file_prefers_workspace_over_model_authored` — now also reads an artifact-only name via the fallback)
- **TEST-6**: PASS  (`read_file_missing_returns_actionable_error_no_host_path`)
- **TEST-7**: PASS  (`list_files_includes_model_authored_artifacts` — incl. mcp+llm and the same-name collapse)
- **TEST-8**: PASS  (`map_tool_error_surfaces_client_errors_and_hides_server_errors`, unit)
- **TEST-9**: PASS  (`read_file_ambiguous_model_authored_names_errors`)

Unit: `test result: ok. 1 passed`. Integration: `test result: ok. 8 passed; 0 failed`.

## Negative control (proves the tests are load-bearing, not paper-green)

Reverted the production resolution logic
(`src-app/server/src/modules/code_sandbox/tools/files.rs` → `origin/khoi`), kept the
tests, rebuilt the server binary, and re-ran the integration suite:
`test result: FAILED. 0 passed; 8 failed`
(log: `/data/khoi/home-workspace/ziee/tmp/sandbox-read-file-artifacts-negctl-*.log`).
Every test fails with the fix removed. Restored `files.rs` from HEAD → back to
`8 passed`. So each test genuinely exercises the fix.

## A-series (deterministic phase-8 checks)

- **A2** clean tree — all load-bearing files committed (scratch symlink git-excluded;
  pgvector submodule at the recorded commit, empty diff).
- **A3** no diff-added `#[ignore]`/`.skip`/`.only`.
- **A4** no cosmetic/always-true assertions.
- **A8/A9/A10** N/A — no new MCP server, no new permission introduced.
- No UI diff → A7/gate:ui / R2-5 route-mock checks do not apply.

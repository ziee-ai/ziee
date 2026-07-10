# TEST_RESULTS — sandbox-tool-approval-loop

Backend-only diff (`src-app/server/**` + `justfile`); no frontend workspace touched, so no
`npm run check` gate applies. All Phase-3 tests run and pass.

## Commands (worktree; `CARGO_TARGET_DIR` set; env sourced)
- Unit: `cargo test --lib mcp::chat_extension::` → **55 passed; 0 failed** (includes the 13
  new `approval_loop_tests` + all existing chat_extension unit tests — no regression).
- Integration: `cargo test --test integration_tests -- --test-threads=1 mcp_approval_loop_`
  → **2 passed; 0 failed**.
- Regression (well-behaved prefixed-name + unique-id path): `cargo test --test
  integration_tests -- --test-threads=1 mcp::elicitation_mcp_test` → **12 passed; 0 failed**.

## Per-TEST results (Phase-3 TESTS.md)
- **TEST-1**: PASS  (recover_server_id_unambiguous_happy_path)
- **TEST-2**: PASS  (recover_server_id_ambiguous_returns_none)
- **TEST-3**: PASS  (recover_server_id_not_found_returns_none)
- **TEST-4**: PASS  (resolve_id_mints_when_empty)
- **TEST-5**: PASS  (resolve_id_mints_on_collision)
- **TEST-6**: PASS  (resolve_id_preserves_good_provider_id)
- **TEST-8**: PASS  (mcp_approval_loop_unresolvable_tool_errors_and_terminates)
- **TEST-10**: PASS (mcp_approval_loop_bare_name_recovers_and_executes)
- **TEST-11**: PASS (`check-mcp-approval` gate — the `just` binary is absent in this env, so
  the recipe body was executed verbatim: `cargo test --lib mcp::chat_extension::` +
  `cargo test --test integration_tests -- --test-threads=1 mcp_approval_loop_`, both green,
  and the `mcp_approval_loop_` filter selects exactly the two new integration tests.)

## Notes
- Also added 7 unit tests for the extracted `resolve_server_and_tool` helper (part of the
  55-unit count) covering well-formed/bare/empty-prefix/`__`-in-name/unknown cases.
- `uvx` is not installed in this environment, so the `uvx mcp-server-fetch`-dependent
  streaming/approval-workflow suites (`mcp_streaming_workflow_test`,
  `mcp_approval_workflow_test`) were not run here; the stub-based `elicitation_mcp_test`
  covers the same finalize/accumulate/tool-call code path without an external server and
  passes, confirming no regression to the well-behaved provider path.
- Live end-to-end gpt-oss repro against the running container is the remaining manual
  acceptance step (see STATUS).

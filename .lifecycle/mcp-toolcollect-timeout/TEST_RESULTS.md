# TEST_RESULTS — mcp-toolcollect-timeout

Backend-only diff (`src-app/server/**`). No `src-app/ui/**` / `src-app/desktop/ui/**`
touched, so the frontend `npm run check` / `gate:ui` / e2e gates do not apply. No new
permission (no A9/A10), no new MCP server (no A8), no migration, no OpenAPI regen.

Enumerated run (phase 8). Logs:
`/data/pbya/ziee/tmp/lifecycle-logs/mcp-toolcollect-timeout-int.log` (first run),
`...-int2.log` (after the phase-7 round-1 test-assertion corrections), and
`...-int3.log` (final, after the round-2 always-mode breaker-consult fix). All three
enumerated results below are from the final `-int3` run: unit `30 passed; 0 failed`,
integration (4 enumerated) `4 passed; 0 failed`.

- **TEST-1**: PASS — `modules::mcp::client::stdio::tests::with_handshake_timeout_elapses_on_a_never_completing_handshake` (unit; a `pending()` handshake returns Err in a bounded window, ready future passes through).
- **TEST-2**: PASS — `mcp::mcp_extension_test::stalling_server_is_skipped_and_llm_still_gets_healthy_tools` (integration; healthy tool reaches LLM, stalling server skipped, bounded).
- **TEST-3**: PASS — `modules::mcp::client::manager::breaker_tests::timeout_origin_failure_opens_the_breaker` (unit; drives real `record_failure_into` → breaker opens on a timeout-origin error, streak deepens).
- **TEST-4**: PASS — `modules::mcp::client::stdio::tests::handshake_budget_floors_to_one_second` (unit; 0/negative timeout floored to 1s).
- **TEST-5**: PASS — `mcp::mcp_extension_test::healthy_auto_server_tools_reach_llm_unchanged` (integration; success-path no-op).
- **TEST-6**: PASS — `mcp::mcp_extension_test::stalling_always_mode_server_does_not_stall_turn` (integration; always-mode stall tolerated, healthy auto tool present).
- **TEST-7**: PASS — `mcp::mcp_extension_test::hanging_stdio_handshake_is_time_bounded` (integration; a real spawns-but-never-handshakes stdio server's create-time probe returns in ~2s, bounded — hangs forever if the inner timeout is reverted).

Result lines: unit `test result: ok. 30 passed; 0 failed`; integration (4 enumerated,
`--test-threads=1`) `test result: ok. 4 passed; 0 failed`.

## Acceptance tests (design-invariant proofs) — all PASS

- INV-1 → TEST-1 (PASS), TEST-7 (PASS)
- INV-2 → TEST-2 (PASS), TEST-6 (PASS)
- INV-3 → TEST-3 (PASS)

## Environmental (Category A) — NOT this branch's regression

The full `mcp::mcp_extension_test` run also includes 8 PRE-EXISTING tests that use a
REAL LLM provider via `get_or_create_test_model` and fail here with
`No AI provider API keys found. Please set at least one in tests/.env.test` — this
worktree's `tests/.env.test` ships placeholder keys (known test-env floor, Category A).
They are NOT in this feature's enumerated set, do not touch the changed code paths, and
fail identically on `main`: `test_mcp_all_tools_with_empty_array`,
`test_mcp_disabled_servers_ignored`, `test_mcp_extension_disabled_by_default`,
`test_mcp_extension_enabled_with_no_servers`, `test_mcp_specific_tool_selection`,
`test_mcp_tools_added_to_llm_request`, `test_mcp_user_can_access_group_servers`,
`test_mcp_user_can_only_access_own_servers`. Every StubChat-based test (incl. this
feature's TEST-2/5/6/7 and the pre-existing `external_tools_labeled_*` /
`labeled_external_tool_still_dispatches` / `test_mcp_access_revocation_*`) passes,
confirming the failures are the missing API key, not a code regression.

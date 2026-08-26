# TEST_RESULTS — code-sandbox-chat-attach

Backend-only diff (`src-app/server/**`). No frontend workspace touched → no
`npm run check` / `gate:ui` / e2e gates apply. No new permission → no A9/A10 deny
tests. No new built-in MCP server (code_sandbox is pre-existing) → A8 N/A.

Logs:
- unit: `/data/pbya/ziee/tmp/lifecycle-logs/code-sandbox-chat-attach-unit.log`
- integration: `/data/pbya/ziee/tmp/lifecycle-logs/code-sandbox-chat-attach-int.log`

Commands (foreground, exit 0):
- `cargo test --lib -p ziee code_sandbox` → `test result: ok. 87 passed; 0 failed`
- `cargo test --lib -p ziee all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not` → `ok. 1 passed`
- `source tests/.env.test && cargo test --test integration_tests code_sandbox_not_advertised_when_disabled -- --test-threads=1` → `ok. 1 passed`

## Results (every Phase-3 TEST-ID)

- **TEST-1**: PASS — `modules::code_sandbox::chat_extension::tests::apply_attach_sets_shared_flag`
- **TEST-2**: PASS — `modules::code_sandbox::chat_extension::tests::should_attach_only_when_tool_capable_and_enabled` [acceptance INV-1]
- **TEST-3**: PASS — `modules::mcp::chat_extension::mcp::builtin_tests::code_sandbox_attaches_on_flag_and_is_not_approval_bypassed` (auto_attach include/exclude leg) [acceptance INV-3]
- **TEST-4**: PASS — `modules::mcp::chat_extension::mcp::builtin_tests::code_sandbox_attaches_on_flag_and_is_not_approval_bypassed` (`!is_builtin_server_id` leg) [acceptance INV-2]
- **TEST-5**: PASS — `mcp::mcp_extension_test::code_sandbox_not_advertised_when_disabled` (integration; execute_command absent when disabled, built-in present as positive control)
- **TEST-6**: PASS — `modules::mcp::chat_extension::mcp::builtin_tests::code_sandbox_execute_command_forces_approval_even_under_autoapprove` [acceptance INV-2]
- **TEST-7**: PASS — `modules::code_sandbox::chat_extension::tests::apply_attach_if_eligible_sets_flag_only_when_eligible` [acceptance INV-1]

## Acceptance-test roll-up (design-invariant proofs)

- **INV-1** — TEST-2: PASS, TEST-7: PASS
- **INV-2** — TEST-4: PASS, TEST-6: PASS
- **INV-3** — TEST-3: PASS

## Regression guard (pre-existing, kept green)

- `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not`: PASS —
  confirms `is_builtin_server_id` was NOT changed (code_sandbox stays approval-gated).

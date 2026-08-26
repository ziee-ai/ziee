# TESTS — code-sandbox-chat-attach

All enumerated tests are reproducible (always-run, rootfs-free). Enabling a real
code_sandbox requires a mounted rootfs (`harness::rootfs_path()` returns None on a
clean box → the harness refuses `sandbox_enabled: true`), so — per the brief — the
acceptance tests assert the ADVERTISE / RESOLVE logic (not a real bwrap exec) and
never self-skip. `[feedback_reproducible_results_only]` / self-skip ≠ PASS.

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/code_sandbox/chat_extension/mod.rs` — asserts: `apply_code_sandbox_attach` inserts the SHARED `ATTACH_FLAG` ("attach_code_sandbox") key with value "true" into the metadata map (the producer/consumer contract point).

- **TEST-2** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-1] file: `src-app/server/src/modules/code_sandbox/chat_extension/mod.rs` — asserts: the pure gating decision `should_attach(tool_capable, enabled)` is TRUE only when BOTH the model is tool-capable AND code_sandbox is enabled, and FALSE for {enabled but not tool-capable}, {tool-capable but disabled}, {neither}. This is INV-1's promise directly: enabled+tool-capable ⇒ the attach flag is set (⇒ execute_command advertised); disabled ⇒ flag not set (⇒ not advertised). `before_llm_call` binds `enabled` to `config::get_state().is_some()` and `tool_capable` to `model_supports_tools`.

- **TEST-3** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-3] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `auto_attach_builtin_ids` INCLUDES `code_sandbox_server_id()` when the `attach_code_sandbox` flag is set, and EXCLUDES it when the flag is absent. The id being in the collector's fetch list is exactly what makes `execute_command` advertised (with the `<server_id>__` prefix) and resolvable via the bare-name recovery map — so a model told about `execute_command` (by the always-on `spawn_background` description) no longer hits "could not resolve an MCP server".

- **TEST-4** (tier: unit) [acceptance] [invariant: INV-2] [covers: ITEM-4] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: with the `attach_code_sandbox` flag set, `auto_attach_builtin_ids` attaches `code_sandbox_server_id()` YET `is_builtin_server_id(code_sandbox_server_id())` is FALSE — code_sandbox is attached but NOT approval-bypassed (execution stays behind manual approval). Mirrors `control_attaches_on_flag_and_is_not_approval_bypassed`. The pre-existing `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not` (asserting the same non-membership) stays green — verified unchanged.

- **TEST-5** (tier: integration) [covers: ITEM-2, ITEM-3] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: on the REAL chat pipeline with a tool-capable stub model and code_sandbox DISABLED (the harness default), the captured LLM request advertises NO tool whose wire name ends with `__execute_command` (the registered extension does not over-attach), while a built-in tool (e.g. `__ask_user`/`__get_tool_result`) IS advertised (positive control that the tool list was built and the model is tool-capable). Rootfs-free; always runs. This is the disabled half of INV-1 exercised end-to-end and proves the new extension is registered + wired without over-attaching.

- **TEST-6** (tier: unit) [acceptance] [invariant: INV-2] [covers: ITEM-5] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `code_sandbox_call_needs_approval` returns TRUE for `execute_command`, `write_file`, `edit_file`, and any unknown tool (fail-safe), and FALSE for `read_file` / `list_files` / `get_resource_link`. This is the `is_code_sandbox` approval-ladder arm's classifier: `execute_command` (arbitrary code) always requires approval, overriding even AutoApprove — so auto-attaching code_sandbox never lets an execution tool auto-run. Reinforces INV-2 beyond the bypass-list non-membership (TEST-4).

- **TEST-7** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-1] file: `src-app/server/src/modules/code_sandbox/chat_extension/mod.rs` — asserts: the producer WIRING `apply_attach_if_eligible` (the pure form of `before_llm_call`'s glue) SETS the `attach_code_sandbox` flag when eligible (tool-capable + enabled) and leaves it unset when disabled or non-tool-capable. Closes the "enabled path never exercised" gap: a producer gated on the wrong condition, or that forgot to call `apply`, fails this (the positive INV-1 half, rootfs-free).

## Coverage map

- ITEM-1 → TEST-1, TEST-2, TEST-7
- ITEM-2 → TEST-5
- ITEM-3 → TEST-3, TEST-5
- ITEM-4 → TEST-4
- ITEM-5 → TEST-6
- INV-1 → TEST-2, TEST-7 (acceptance) [+ TEST-5 disabled-half at integration]
- INV-2 → TEST-4, TEST-6 (acceptance)
- INV-3 → TEST-3 (acceptance)

## Not enumerated (deliberate)

- A rootfs-gated end-to-end POSITIVE (enabled ⇒ `execute_command` advertised on the
  live pipeline) is NOT enumerated: it can only self-skip on a clean box
  (`rootfs_path()` None), and self-skip ≠ PASS. The enabled→advertised promise is
  proven deterministically by TEST-2 (producer gating) + TEST-3 (consumer
  inclusion). A real bwrap exec is Tier-4/6 and out of scope for this fix.

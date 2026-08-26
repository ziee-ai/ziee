# DESIGN_FIDELITY — code-sandbox-chat-attach

- **INV-1** — fidelity: UPHELD — ITEM-1/ITEM-2 set `attach_code_sandbox` only when
  `model_supports_tools` AND `config::get_state().is_some()` (the exact
  enabled+tool-capable predicate the design names); ITEM-3 pushes
  `code_sandbox_server_id()` in `auto_attach_builtin_ids` on that flag, so
  `execute_command` is advertised iff enabled+tool-capable and absent otherwise.
- **INV-2** — fidelity: UPHELD — ITEM-4 deliberately leaves `is_builtin_server_id`
  / `builtin_server_ids()` untouched, so `code_sandbox` stays off the
  approval-bypass list; the existing bypass-membership test and a new negative
  unit test pin it. Reinforced by ITEM-5 (fix round 1): the `is_code_sandbox`
  force-approval arm makes `execute_command` require approval even under
  `ApprovalMode::AutoApprove` (mirroring control/background), so "remains behind
  manual approval" holds in ALL approval modes, not just the ManualApprove default.
- **INV-3** — fidelity: UPHELD — once the flag pushes the id, the MCP collector
  advertises `execute_command` with the `<server_id>__` prefix and builds the
  bare-name recovery map, so the model's `execute_command` call resolves to the
  code_sandbox server instead of failing "could not resolve an MCP server". The
  acceptance test drives the real chat path (stub model emits an
  `execute_command` tool call) and asserts resolution/advertisement.

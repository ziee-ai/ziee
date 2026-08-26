# DRIFT-1 — implementation vs plan/design

Authored live as each item landed (FB-18).

- **DRIFT-1.1** — verdict: none — ITEM-1 (chat extension) landed as planned:
  dedicated `code_sandbox/chat_extension/{mod.rs,extension.rs}` mirroring
  `web_search/chat_extension/`, with shared `ATTACH_FLAG`, a pure `should_attach`
  gating fn, and a pure `apply_code_sandbox_attach`. `before_llm_call` binds
  `enabled = config::get_state().is_some()` and `tool_capable = model_supports_tools`.
  No system nudge (DEC-2). Matches DESIGN_FIDELITY INV-1.

- **DRIFT-1.2** — verdict: none — ITEM-2 (registration) landed at order 21 (DEC-5)
  via `distributed_slice(CHAT_EXTENSIONS)`; `pub mod chat_extension;` added to
  `code_sandbox/mod.rs`. Placed in the existing (non-alphabetical) mod block at its
  original position to avoid the whole-crate/one-file rustfmt reorder churn the repo
  deliberately strips (commits 6b2d523e9 / 55290f6e5); no enforced `cargo fmt
  --check` gate exists on the backend, and the committed block is already unsorted.
  The edit-lint hook flags the block-sort but it is advisory and pre-existing.

- **DRIFT-1.3** — verdict: none — ITEM-3 (consumer branch) landed as one additive
  `if flag(...)` in `auto_attach_builtin_ids`, byte-shaped like the sibling
  `web_search` / `control` branches. Matches INV-3.

- **DRIFT-1.4** — verdict: none — ITEM-4 (the deliberate non-edit) upheld:
  `builtin_server_ids()` / `is_builtin_server_id` untouched. TEST-4 + the
  pre-existing `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not`
  both assert code_sandbox is NOT approval-bypassed. Matches INV-2.

- **DRIFT-1.5** — verdict: none — tests landed per TESTS.md: TEST-1/TEST-2 in
  `code_sandbox/chat_extension/mod.rs`; TEST-3/TEST-4 in mcp.rs; TEST-5 integration
  in `tests/mcp/mcp_extension_test.rs`. No enumerated test dropped.

**Unresolved drifts:** 0

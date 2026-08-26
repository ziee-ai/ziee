# PLAN — code-sandbox-chat-attach

## Design source

Realizes the fix diagnosed against the live rig and the two governing docs:
- `CLAUDE.md` → "Code Sandbox" section (built-in MCP server; `execute_command`;
  enabled via `code_sandbox.enabled: true`) and CLAUDE.md §11 "Built-in MCP
  server checklist" (the two `mcp.rs` edits — `auto_attach_builtin_ids` +
  `is_builtin_server_id` — and "forgetting either = silent tools-never-reach-the-model").
- `agent-kit/docs/CODING_GUIDELINES.md` §11 (same built-in-MCP checklist).

The bug: `code_sandbox` registers and works over direct MCP, but its
`execute_command` tool is NEVER advertised to a chat because `code_sandbox` is
absent from `auto_attach_builtin_ids` and has no attach-flag chat extension.
Meanwhile `background_mcp`'s always-on `spawn_background` description references
the foreground `execute_command` tool, so a tool-capable model tries to call a
tool that was never attached and gets "Could not resolve an MCP server for tool
'execute_command'". This is exactly the §11 silent-failure class (the FIRST of
the two mcp.rs edits was omitted).

The chosen policy (from the diagnosis): AUTO-ATTACH WHEN ENABLED — attach
`code_sandbox` to a chat when code_sandbox is enabled AND the model is
tool-capable, mirroring how `web_search`/`background` attach. It MUST stay behind
manual approval (execution subsystem): the SECOND mcp.rs edit
(`is_builtin_server_id`, the approval-bypass list) is intentionally NOT applied —
`execute_command` runs code and must remain approval-gated.

## Invariants

- **INV-1**: when `code_sandbox` is enabled AND the model is tool-capable,
  `execute_command` is advertised to the chat (present in the turn's tool map);
  when code_sandbox is disabled, it is NOT advertised.
- **INV-2**: `execute_command` remains behind manual approval — `code_sandbox` is
  never in `is_builtin_server_id` (approval-bypass); the existing
  bypass-membership test stays green.
- **INV-3**: a tool-capable chat that is told about `execute_command` (via the
  always-on `spawn_background` description) can actually resolve+call it — the
  model calling `execute_command` no longer yields "could not resolve an MCP
  server."

## Items

- **ITEM-1**: Add a `code_sandbox` chat extension (`chat_extension/` submodule)
  with a shared `ATTACH_FLAG = "attach_code_sandbox"` const and a `ChatExtension`
  impl whose `before_llm_call` sets `metadata["attach_code_sandbox"] = "true"`
  ONLY when the model is tool-capable AND code_sandbox is enabled
  (`config::get_state().is_some()`). Mirrors `web_search/chat_extension/`
  (a pure, unit-testable `apply_code_sandbox_attach` helper; the flag key is the
  shared const so producer/consumer can't desync). No system nudge — the
  `execute_command` tool description + the existing mount-context extension
  already carry usage guidance; attaching the tool is what the fix requires.
- **ITEM-2**: Register the new extension in the `CHAT_EXTENSIONS` distributed
  slice at an order < 30 (the MCP collector's order), so the flag is set before
  `auto_attach_builtin_ids` runs. Expose `pub mod chat_extension;` from
  `code_sandbox/mod.rs`.
- **ITEM-3**: Consume the flag in `auto_attach_builtin_ids`
  (`mcp/chat_extension/mcp.rs`): `if flag(crate::modules::code_sandbox::chat_extension::ATTACH_FLAG)
  { ids.push(crate::modules::code_sandbox::code_sandbox_server_id()); }`.
- **ITEM-4**: Do NOT touch `is_builtin_server_id` / `builtin_server_ids()` —
  `code_sandbox` stays off the approval-bypass list (INV-2). This is a
  deliberate NON-edit, asserted by the existing + a new negative test.
- **ITEM-5**: (added in fix round 1, from the blind security audit) Force
  `execute_command` (and the mutating `write_file` / `edit_file`) through manual
  approval EVEN under `ApprovalMode::AutoApprove` — mirroring `control`
  (`invoke_capability`) and `background` (`spawn_background`). Auto-attaching an
  execution tool otherwise lets arbitrary code auto-run with no prompt on a
  conversation set to AutoApprove, contradicting INV-2's "remains behind manual
  approval". Add a `code_sandbox_call_needs_approval(tool_name)` classifier
  (execution/mutation → approve; read_file/list_files/get_resource_link → auto-run;
  unknown → fail-safe approve) and an `is_code_sandbox` arm in the approval ladder.
  Reinforces INV-2.

## Files to touch

- `src-app/server/src/modules/code_sandbox/chat_extension/mod.rs` (new — `ATTACH_FLAG` const + `apply_code_sandbox_attach` + `before_llm_call` impl + unit tests)
- `src-app/server/src/modules/code_sandbox/chat_extension/extension.rs` (new — `CHAT_EXTENSIONS` registration)
- `src-app/server/src/modules/code_sandbox/mod.rs` (edit — `pub mod chat_extension;`)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (edit — the ONE consumer line in `auto_attach_builtin_ids`, the `is_code_sandbox` approval-ladder arm (ITEM-5), + new unit tests)
- `src-app/server/src/modules/code_sandbox/handlers.rs` (edit — `code_sandbox_call_needs_approval` classifier (ITEM-5))
- `src-app/server/tests/mcp/mcp_extension_test.rs` (edit — integration acceptance test: advertised-when-enabled / absent-when-disabled)
- `src-app/server/tests/mcp/mod.rs` (edit only if a new test file were added — not planned; test lands in existing mcp_extension_test.rs)

## Patterns to follow

- Attach-flag chat extension: **`src-app/server/src/modules/web_search/chat_extension/`**
  (`mod.rs` — `ATTACH_FLAG` const; `web_search.rs` — `before_llm_call` gating +
  pure `apply_*_attach` helper + `#[cfg(test)]`; `extension.rs` — the
  `distributed_slice(CHAT_EXTENSIONS)` registration with an `order` < 30).
- Tool-capable gate: `crate::modules::file::available_files::model_supports_tools(&context.metadata)`
  (used verbatim by `web_search.rs`).
- Enabled predicate: `code_sandbox::config::get_state()` is `Some` only when
  enabled+initialized (used by `mount_context_extension.rs` and `client/stdio.rs`).
- `auto_attach_builtin_ids` consumer branch: the existing `control` / `web_search`
  branches in `mcp/chat_extension/mcp.rs` (~line 244-271).
- Approval posture (attach ≠ bypass): the `control` and `background` precedent —
  auto-attached but deliberately absent from `is_builtin_server_id`
  (mcp.rs tests `control_attaches_on_flag_and_is_not_approval_bypassed`,
  `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not`).
- Integration test shape: `tests/mcp/mcp_extension_test.rs` (tool discovery /
  injection into the LLM request via the stub chat model).

## Non-UI note

Backend-only. No `src-app/ui/**` change, no new permission (code_sandbox perms
already exist), no migration, no OpenAPI/type change. No new top-level module or
seam (the `chat_extension/` submodule mirrors 7 existing siblings). Expected
tier: LIGHT.

## Plan audit (Phase 2)

### Breakage risk

- The only edit to an existing production code path is ONE additive branch in
  `auto_attach_builtin_ids` (a pure fn over `metadata`). It appends an id only
  when the new flag is present; existing callers are unaffected when the flag is
  absent (default false via `flag(...)`). No signature change.
- The new chat extension is additive (a fresh `distributed_slice` entry). It runs
  a cheap `model_supports_tools` + `get_state()` read and returns
  `BeforeLlmAction::Continue`; it cannot break chat (mirrors web_search, which
  also swallows errors). It sets only its own metadata key.
- code_sandbox gains a SECOND chat extension alongside the existing
  `code_sandbox_mounts` (order 12). Both are independent `before_llm_call` hooks;
  no ordering dependency between them (each keys off `get_state()`).

### Pattern conformance

- Mirrors `web_search/chat_extension/` file-for-file (mod.rs const + pure helper
  + test; extension.rs registration). Divergences are deliberate and minimal:
  no provider-chain gate (code_sandbox has no providers) and no system nudge
  (the tool description + mount extension already inform the model).
- The consumer branch in `auto_attach_builtin_ids` is byte-shaped like the
  existing `web_search` / `control` branches.

### Migration collisions

- None. This branch adds no migration (see BASE.md; highest server prefix
  `202608250100` untouched).

### OpenAPI regen

- Not required. No handler/type/schema change; `openapi.json` and
  `api-client/types.ts` are untouched in both `ui/` and `desktop/ui/`.

### Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors `web_search/chat_extension/web_search.rs`;
  `config::get_state()` confirmed as the enabled predicate (mount_context_extension.rs
  line 90, client/stdio.rs); `model_supports_tools` confirmed as the tool-capable gate.
- **ITEM-2** — verdict: PASS — `distributed_slice(CHAT_EXTENSIONS)` auto-registers
  once the submodule is compiled (confirmed by mount_context_extension.rs, which
  registers the same way from a `pub mod` in code_sandbox/mod.rs). Order < 30
  required and available (MCP collector is order 30; web_search uses 26).
- **ITEM-3** — verdict: PASS — one additive branch; `code_sandbox_server_id()`
  exists (mod.rs line 57); `chat_extension::ATTACH_FLAG` path mirrors the 7
  sibling consumer branches already in the fn.
- **ITEM-4** — verdict: PASS — a NON-edit. The existing test
  `all_readonly_builtins_share_approval_bypass_but_execution_ones_do_not`
  (mcp.rs ~5591) already asserts `code_sandbox` is NOT approval-bypassed; leaving
  `builtin_server_ids()` untouched keeps it green (INV-2).
- **ITEM-5** — verdict: PASS — the `is_control` / `is_background` force-approval
  arms (mcp.rs ~3288-3324) are the exact precedent; the new `is_code_sandbox` arm
  is byte-shaped like them and the classifier mirrors
  `background_call_needs_approval`. read_file/list_files/get_resource_link are the
  read-only tools (auto-run); execute_command/write_file/edit_file force approval.

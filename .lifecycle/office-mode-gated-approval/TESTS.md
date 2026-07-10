# TESTS — office-mode-gated-approval

Every ITEM ↔ ≥1 TEST; the security-critical approval classifier is covered
exhaustively (every mode value, spoof server, every decision branch, and the full
approval loop end-to-end). Backend-only (desktop `office_bridge` + server `mcp`); no
frontend workspace touched → no `tier: e2e` required. Tiers mirror the codebase:
in-source `#[cfg(test)]` unit, `tests/…` integration, node `--test` for pane helpers.

## Desktop — collapsed tool surface (ITEM-1, ITEM-2, ITEM-8)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — asserts: `tool_list()` is EXACTLY `{list_open_documents, run_office_js}` (2 tools) and the 5 pruned names (`read_document`, `get_selection`, `add_comment`, `set_track_changes`, `get_tracked_changes`) are ALL absent.
- **TEST-2** (tier: unit) [covers: ITEM-2, ITEM-8] file: `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — asserts: `run_office_js` `inputSchema` requires `doc_full_name` + `script` + `mode`, with `mode` an enum `["read","write"]`; and the `description` string carries the read/write guidance (mentions `mode`, "read", "write", and "approval") so the model is actually told how to set it and that writes prompt.

## Desktop — dispatch (ITEM-3)

- **TEST-3** (tier: unit) [covers: ITEM-3, ITEM-4] file: `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — asserts: `dispatch_tool(mock, <name>, …)` for EACH pruned name (`read_document`, `get_selection`, `add_comment`, `set_track_changes`, `get_tracked_changes`) returns `UNKNOWN_TOOL` — proving the arms + the pane handlers behind them are gone (the crate still compiles without the PPT pre-gate / const).
- **TEST-4** (tier: unit) [covers: ITEM-3] file: `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — asserts: `run_office_js` behaves IDENTICALLY for `mode:"read"` and `mode:"write"` at the daemon — both validate args the same (missing `script`/`doc_full_name` → `INVALID_ARGS`) and both route to the pane (no matching pane → `OFFICE_PANE_NOT_CONNECTED` for either mode), i.e. the daemon does not gate execution on `mode`.
- **TEST-5** (tier: unit) [covers: ITEM-3] file: `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — asserts: `list_open_documents` still dispatches natively and returns the seeded docs (the surviving native tool is unaffected).

## Desktop — pane helpers survive the op removal (ITEM-4)

- **TEST-6** (tier: unit) [covers: ITEM-4] file: `src-app/desktop/tauri/resources/office-bridge/taskpane.test.mjs` — asserts: after removing the 5 pruned `op*` handlers, the node helper suite (`serializeResult`/`describeError`/`capText`/`sameDoc`/…, all still used by `run_office_js`) still loads and passes — a regression guard that the removal didn't break the surviving `run_office_js` path or its exports.

## Desktop — integration (ITEM-7)

- **TEST-7** (tier: integration) [covers: ITEM-1] file: `src-app/desktop/tauri/tests/office_bridge/settings_mcp_test.rs` — asserts: the live `tools/list` JSON-RPC response advertises EXACTLY the 2-tool set (`EXPECTED_TOOLS` updated); none of the 5 pruned tools present.
- **TEST-8** (tier: integration) [covers: ITEM-7] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: against a mock pane, `dispatch_tool("run_office_js", {mode:"read", …})` and `{mode:"write", …}` BOTH round-trip and surface the pane result identically — `mode` is delivered in `params` but does not change daemon/pane execution. (Covers the `test9`/`test12` retargets onto `run_office_js`.)
- **TEST-9** (tier: integration) [covers: ITEM-7] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: a mock pane replying `-32002` to a `run_office_js` request maps to `OFFICE_UNSUPPORTED_ON_HOST` via `dispatch_tool` (the `test16` retarget preserves the exact mapping coverage after the Word-only tools are gone).

## Server — the read-bypass classifier (ITEM-6) — full security matrix

- **TEST-10** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/mcp/chat_extension/office_approval.rs` — asserts: `run_office_js_read_bypass(server_id, tool_name, input)` returns TRUE ONLY for `office_bridge_mcp_server_id()` + `"run_office_js"` + `input.mode == "read"` (exact), and FALSE for every other case — `mode:"write"`, `mode` MISSING, `mode` = `"READ"`/`"Read"`/`"read "`/`"readonly"`/non-string, a DIFFERENT tool (`list_open_documents`) even with `mode:"read"`, and a NON-office server id spoofing a `run_office_js` tool with `mode:"read"`.
- **TEST-11** (tier: unit) [covers: ITEM-6] file: `src-app/desktop/tauri/src/modules/office_bridge/mod.rs` — asserts: the desktop `office_bridge` built-in row id equals `ziee::…::office_bridge_mcp_server_id()` (drift guard so the server's recomputed deterministic v5 can never diverge from the desktop's registered id).

## Server — the extracted approval decision (ITEM-5) — behavior-preservation matrix

- **TEST-12** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/mcp/chat_extension/office_approval.rs` — asserts: `compute_needs_approval(...)` reproduces EVERY pre-existing branch AND the new office branch: (a) built-in server → bypass; (b) control server → delegates to the control classifier (read→bypass, mutating→approve) unchanged; (c) Disabled mode + non-builtin → deny; (d) office_bridge `run_office_js` `read` → bypass; (e) office_bridge `run_office_js` `write` → follows ManualApprove (approve) unless auto-approved; (f) office_bridge `run_office_js` `write` + tool in auto_approved list → bypass (always-allow); (g) normal server ManualApprove not-auto-approved → approve; (h) normal server ManualApprove auto-approved → bypass; (i) normal server AutoApprove → bypass.

## Server — approval loop wiring + the model-behaviour assumption

> Reality (surfaced in implementation, see DRIFT-1): `office_bridge` is a **desktop-only**
> MCP server, so it is NOT registrable in the server (`server/tests/mcp`) harness, and
> the approval-workflow harness is **LLM-driven** — the deterministic office-loop
> integration tests the plan first imagined aren't feasible as specified. The decision
> logic is instead pinned **exhaustively by the unit matrix** (TEST-10 / TEST-12 — every
> branch, every `mode` value, spoof, fail-safe, always-allow), the loop **wiring** is
> proven behaviour-preserving by the EXISTING approval suite, and a real-LLM test
> validates the trust-model's core assumption (the model self-classifies `mode`).

- **TEST-13** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/mcp/mcp_approval_workflow_test.rs` — asserts: the EXISTING approval-workflow suite (auto-approve executes immediately / manual-approve creates a pending approval / approve-and-resume executes) STILL passes end-to-end after the `compute_needs_approval` extraction — the SAME loop that gates office_bridge routes through the extracted fn, so this proves it is behaviour-preserving for control/normal servers live. Runs against the coder.ziee OpenAI-compatible endpoint (`OPENAI_BASE_URL=http://127.0.0.1:4000`, gpt-4o via LiteLLM wildcard — DEC-7).
- **TEST-14** (tier: integration) [covers: ITEM-6] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: REAL-LLM (soft-skips when `ZIEE_OFFICE_REAL_LLM_URL` unset) — given the SHIPPED `run_office_js` schema, a real model (coder.ziee `qwen3.6-35b-a3b`) declares `mode:"read"` for a pure-read task ("read cell A1") and `mode:"write"` for a mutating task ("set A1 to 'hello'"). This validates the trust-based model's load-bearing assumption — that the model reliably self-classifies read vs write — which is exactly what the auto-approve-reads / prompt-writes gating depends on.

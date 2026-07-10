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

## Server — approval loop end-to-end (ITEM-7)

- **TEST-13** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/mcp/office_approval_test.rs` — asserts: with a mock MCP server registered under `office_bridge_mcp_server_id()` exposing `run_office_js`, ManualApprove mode — a `run_office_js` `{mode:"write"}` tool call creates a PENDING approval (`get_pending_approvals` non-empty) and does NOT auto-execute, while a `{mode:"read"}` call creates NO pending approval and auto-executes.
- **TEST-14** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/mcp/office_approval_test.rs` — asserts: "always allow" — with `auto_approved_tools` containing `run_office_js` for that server/conversation, a `{mode:"write"}` call creates NO pending approval and auto-executes (the existing per-conversation memory carries the write grant).
- **TEST-15** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/mcp/office_approval_test.rs` — asserts: DENY — resuming a pending `run_office_js` write approval with a `deny` decision does NOT execute the tool.
- **TEST-16** (tier: integration) [covers: ITEM-7] file: `src-app/server/tests/mcp/office_approval_test.rs` — asserts: FAIL-SAFE — a `run_office_js` call with `mode` OMITTED (or an invalid value) is treated as `write` → creates a pending approval (never silently auto-runs).
- **TEST-17** (tier: integration) [covers: ITEM-6, ITEM-7] file: `src-app/server/tests/mcp/office_approval_test.rs` — asserts: REAL-LLM end-to-end (soft-skips when `ZIEE_OFFICE_REAL_LLM_URL` unset) — a real model (coder.ziee `qwen3.6-35b-a3b`) given the `run_office_js` schema + a READ task emits `mode:"read"` and the call auto-runs (no pending approval); given a WRITE task it emits `mode:"write"` and a pending approval is created. Proves the model actually sets `mode` correctly AND the loop honors it, without needing a live Excel pane (mock office server).

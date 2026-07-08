# TESTS — office-bridge pane RPC

Every ITEM is covered by ≥1 TEST. Tiers mirror the existing office_bridge suite:
unit `#[cfg(test)]` in-source, integration `tests/office_bridge/` on the desktop
`TestServer`/ephemeral-bridge harness. The external boundary mocked is **only** the
Office.js execution — a mock pane WSS client stands in for the real task pane; the
broker, socket loop, and dispatch run for real.

No `tier: e2e` test is enumerated: the diff touches `src-app/desktop/tauri/**` +
`resources/**` + root docs only — no `src-app/ui/**` / `src-app/desktop/ui/**`
workspace — so the frontend e2e gate does not apply. `taskpane.js` runs Office.js
inside a real Office host and cannot be driven headless; its execution is verified
live (TEST-13 Mac, TEST-14 Windows) while its **wire contract** is proven by the
mock-pane integration tests (TEST-6/9).

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: `call_pane` with no registered pane resolves to a typed `OFFICE_PANE_NOT_CONNECTED` `AppError`.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: with a pane registered against a mock mpsc receiver, `call_pane` pushes a well-formed `BridgeRequest` (method + numeric corr `id` + params) down the pane `tx`, and `route_response(corr_id, Ok(result))` unblocks `call_pane` returning that result.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: `call_pane` with a short timeout and no response returns a typed `OFFICE_PANE_TIMEOUT` and removes the pending entry (no leak).
- **TEST-4** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: pane resolution — an exact `doc_key` match is chosen over other panes; with one pane and no key match the sole pane is used; with ≥2 panes and no match → `OFFICE_PANE_NOT_CONNECTED`.
- **TEST-5** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: unregistering a pane (or dropping its `tx`) fails an in-flight `call_pane` with a typed error rather than hanging (the oneshot sender is dropped → recv errors).
- **TEST-6** (tier: integration) [covers: ITEM-2, ITEM-4] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: a mock pane WSS client connects to `server::start(0)`, sends the `register` hello, and a concurrent `broker::call_pane` request reaches the mock pane (correct method/params/corr-id envelope) and the mock's `{result}` reply routes back to the caller — the full socket-loop↔broker round-trip and the exact wire contract `taskpane.js` must implement.
- **TEST-7** (tier: integration) [covers: ITEM-2] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: after the mock pane closes its socket, the pane is unregistered — a subsequent `call_pane` returns `OFFICE_PANE_NOT_CONNECTED`.
- **TEST-8** (tier: integration) [covers: ITEM-2] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: a response frame with an unknown/stale correlation id (and a non-JSON-RPC junk frame) is ignored — the socket loop keeps running and a subsequent valid round-trip still succeeds.
- **TEST-9** (tier: integration) [covers: ITEM-3, ITEM-4] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: with a mock pane registered and answering, `dispatch_tool(platform, "read_document", {doc_full_name})` returns the pane's result mapped into the MCP `tool_result` shape (`content` + `structuredContent`).
- **TEST-10** (tier: unit) [covers: ITEM-3] file: `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — asserts: the 5 pane tools with **no pane connected** now return `OFFICE_PANE_NOT_CONNECTED` (replaces the old `OFFICE_PANE_REQUIRED` assertion; the dead `pane_required_err`/const is removed).
- **TEST-11** (tier: unit) [covers: ITEM-3] file: `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — asserts: `add_comment`/`set_track_changes` targeting a PowerPoint doc still return `OFFICE_UNSUPPORTED_ON_HOST` via the native `doc_host` pre-gate, before any broker call.
- **TEST-12** (tier: integration) [covers: ITEM-3] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: a pane that replies with a JSON-RPC `error` (e.g. host-unsupported op) propagates as a typed `AppError` (message/code preserved) through `dispatch_tool`, not a panic or a success.
- **TEST-13** (tier: integration) [covers: ITEM-4, ITEM-6] file: `MAC_OFFICE_BRIDGE_VERIFICATION.md` — asserts: live on this Mac, with the desktop app running + the add-in sideloaded, each of the 5 pane ops (`get_selection`, `read_document`, `add_comment`, `set_track_changes`, `get_tracked_changes`) round-trips through the real Excel/Word task pane and returns a correct result, recorded with the observed pane log + returned payloads.
- **TEST-15** (tier: integration) [covers: ITEM-2, ITEM-1] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: POSITIVE multi-pane routing — two live panes for two distinct documents each answer, and a call for doc A resolves to A's pane and doc B to B's (exact-match resolution + per-pane response binding, not cross-routed). (Phase-7 fix coverage: resolve_pane exact-first + route_response pane-binding.)
- **TEST-16** (tier: integration) [covers: ITEM-3] file: `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — asserts: a pane reply with code -32002 ("unsupported on this host") maps to OFFICE_UNSUPPORTED_ON_HOST (same as the native PPT pre-gate), not the generic OFFICE_PANE_ERROR.
- **TEST-17** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: route_response from a DIFFERENT pane than a request was routed to does NOT resolve it (cross-pane spoofing guard); the correct pane's reply does. (Phase-7 security fix.)
- **TEST-18** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` — asserts: unregister_pane fast-fails an in-flight call_pane (dropped oneshot → OFFICE_PANE_NOT_CONNECTED) instead of hanging to the timeout. (Phase-7 fix for the close-vs-in-flight hang.)
- **TEST-14** (tier: integration) [covers: ITEM-5] file: `WINDOWS_PANE_VERIFICATION.md` — asserts: the Windows manual live checklist (mirroring TEST-13/Mac) — WebView2 loads `https://localhost:44300/taskpane.html` prompt-free, the WSS connects, and each of the 5 pane ops round-trips in real Word/Excel. A doc-based live verification (DRIFT-1.1): the real WebView2 pane can't be driven from a cargo test, and the cross-platform backend it depends on is proven on Mac by TEST-6/7/8/9/12.

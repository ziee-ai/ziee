# PLAN — office-mode-gated-approval

Collapse the `office_bridge` tool surface to **two** tools — `list_open_documents`
(native discovery) and `run_office_js` (everything else) — and make `run_office_js`
**mode-gated for approval**: the model declares `mode: "read" | "write"`; a `read`
call auto-runs (no prompt), a `write` call goes through the existing per-call
approval (allow-once / always-allow-for-conversation / deny). The pane does NOT
enforce read-only (deliberate, per user decision — see DECISIONS): `mode` is purely
an approval-routing hint the model is trusted to set honestly. This mirrors
`control_mcp`, which is auto-attached-but-not-bypassed and uses a per-call
`control_call_needs_approval` classifier so read ops auto-run and mutating ops
prompt.

Prunes 5 typed tools (`read_document`, `get_selection`, `add_comment`,
`set_track_changes`, `get_tracked_changes`). Cross-crate but backend-only (desktop
`office_bridge` + server `mcp` approval loop; the approval UI already has
approve-once/approve-conversation/deny, so **no frontend change** → no e2e gate).

## Items

- **ITEM-1**: Prune the 5 typed tool descriptors from `tools.rs::tool_list()` → exactly `{list_open_documents, run_office_js}`. Update the `tool_list` unit tests to the 2-tool set (removed names absent).
- **ITEM-2**: Add `mode` (JSON enum `["read","write"]`, **required**) to `run_office_js`'s `inputSchema`, and enrich its `description`: tell the model to set `mode:"read"` ONLY for scripts that purely read (no property sets, no `insert*`/`delete`/`add`/`changeTrackingMode`/`insertComment`) and `mode:"write"` for anything that changes the document — because a `write` will ask the user for approval. Include brief Word examples for the now-inlined flows (adding a comment via `body.search(...).items[0].insertComment(...)`, toggling `document.changeTrackingMode`) so the model still discovers them.
- **ITEM-3**: In `handlers.rs::dispatch_tool` remove the two pruned dispatch arms (`"read_document" | "get_selection"` and `"add_comment" | "set_track_changes" | "get_tracked_changes"`) and the now-orphaned PowerPoint pre-gate they were the sole users of — `doc_host()`, `unsupported_on_ppt_err()`, and the **handlers.rs** `OFFICE_UNSUPPORTED_ON_HOST` const (audit-confirmed sole users; `broker::OFFICE_UNSUPPORTED_ON_HOST` STAYS — run_office_js still maps the pane's `-32002`). KEEP the `OfficeApp` import (used by `seeded_mock`). The `run_office_js` arm is UNCHANGED — the daemon does NOT read `mode` (execution is identical; `mode` is consumed only by the server approval loop). Remove the pruned tools' handler unit tests (`test12_add_comment_on_powerpoint_*`, `test12_set_track_changes_on_powerpoint_*`, `test10_add_comment_on_word_with_no_pane_*`) and **remove `test10_pane_mediated_method_no_pane_is_not_connected` entirely** — its loop is over `[get_selection, read_document, get_tracked_changes]` (all pruned), and its behavior (a pane-mediated tool with no pane → `OFFICE_PANE_NOT_CONNECTED`) is already covered for the sole survivor by `test6_run_office_js_no_pane_is_not_connected`. Keep `test12_list_open_documents`, `test4_edit_document_is_removed`, `test5_run_office_js_invalid_args`, `test6_run_office_js_no_pane`, `test12_unknown_tool`, `test16_run_connect_*`.
- **ITEM-4**: In `taskpane.js` remove `opReadDocument`, `opGetSelection`, `opAddComment`, `opSetTrackChanges`, `opGetTrackedChanges` + their `dispatchOp` `case` labels + the now-unused `ERR_ANCHOR_NOT_FOUND` code. KEEP `opRunOfficeJs` and every shared helper still used by it (`capText`/`MAX_READ_CHARS` via `serializeResult`, `serializeResult`, `describeError`, `safeString`, `ERR_UNSUPPORTED_HOST`, the mis-routing `sameDoc` guard, `openBridge`/`sendRegister`). Update the responsibilities comment to the 2-op surface.
- **ITEM-5**: (server) Extract the per-call approval decision currently inline at `mcp.rs:2120` (`let needs_approval = if is_control … else if is_builtin … else {ManualApprove/auto_approved}`) into a **pure, unit-testable function** `compute_needs_approval(server_id, tool_name, input, approval_mode, is_builtin, is_control, auto_approved_here) -> ApprovalOutcome` (mirrors how `control_mcp` factored `needs_approval_decision`). Behavior-preserving for all existing servers; call it from the loop.
- **ITEM-6**: (server) Add the office read-bypass to that decision. New server helpers in a small module (e.g. `mcp/chat_extension/office_approval.rs`): `office_bridge_mcp_server_id()` = `Uuid::new_v5(&NAMESPACE_URL, b"office_bridge.ziee.internal")` (same derivation the desktop uses), and `run_office_js_read_bypass(server_id, tool_name, input) -> bool` = `server_id == office_bridge_mcp_server_id() && tool_name == "run_office_js" && input.get("mode").and_then(as_str) == Some("read")` (EXACT `"read"` only). In `compute_needs_approval`, an office_bridge `run_office_js` with `mode=="read"` → bypass; **anything else on office_bridge falls through to the normal ManualApprove path** (write → prompt, or auto-run if the user picked always-allow). Fail-safe: `mode` missing / not exactly `"read"` / a non-office server that happens to name a tool `run_office_js` → NOT bypassed.
- **ITEM-7**: Update integration tests that name the pruned tools. `settings_mcp_test.rs::EXPECTED_TOOLS` → the 2-tool set. `pane_rpc_test.rs`: the **`dispatch_tool`-path** tests currently vehicled on pruned tools MUST retarget to `run_office_js` (they'd otherwise hit `UNKNOWN_TOOL`) — `test9_dispatch_tool_read_document_round_trip`, `test12_pane_error_propagates`, `test16_pane_unsupported_maps_to_unsupported_on_host` (the last preserves the `-32002` → `OFFICE_UNSUPPORTED_ON_HOST` coverage). The **`broker::call_pane`-direct** tests (`test6`, `test7`, `test8`, `test15`) forward the method string generically to the mock pane so they still pass, but retarget their method strings to `run_office_js` for cleanliness (no dead tool names). Add a `run_office_js` mock-pane round-trip assertion that `mode` is passed through and does NOT change execution (read vs write both round-trip identically — the daemon ignores `mode`).
- **ITEM-8**: (docs) Update `tools.rs` header, `handlers.rs` capability-model doc block, `taskpane.js` responsibilities comment, `WINDOWS_PANE_VERIFICATION.md` (2-tool surface + the read-auto / write-approval behavior), and `OFFICE_TOOL_SURFACE_DESIGN.md` (record the collapsed surface + the mode-gated permission model + the accepted trade-offs: trust-based, no read-only enforcement; auto-approved reads are a full-content exfiltration channel; "always allow" grants all later writes for the conversation).

## Files to touch

- `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — ITEM-1, ITEM-2, ITEM-8
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — ITEM-3, ITEM-8
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` — ITEM-4, ITEM-8
- `src-app/desktop/tauri/tests/office_bridge/settings_mcp_test.rs` — ITEM-7
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — ITEM-7
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — ITEM-5, ITEM-6 (call the extracted fn)
- `src-app/server/src/modules/mcp/chat_extension/office_approval.rs` (NEW) — ITEM-5, ITEM-6 (`compute_needs_approval` + office classifier + `office_bridge_mcp_server_id` + unit tests)
- (extraction behaviour-preservation is proven deterministically by TEST-12 + the phase-6 blind equivalence audit; the EXISTING `mcp_approval_workflow_test.rs` LLM-driven suite is env-gated — no LLM key here — and office is desktop-only, so it is not run as the proof; see DRIFT-1 + TEST_RESULTS.md)
- `src-app/desktop/tauri/src/modules/office_bridge/mod.rs` — ITEM-6 (a drift test asserting the desktop `office_bridge` row id equals the server's `office_bridge_mcp_server_id()`)
- `WINDOWS_PANE_VERIFICATION.md`, `OFFICE_TOOL_SURFACE_DESIGN.md` — ITEM-8

## Patterns to follow

- **Approval classifier + extraction (ITEM-5/6)** — mirror `control_mcp/handlers.rs`: `control_call_needs_approval` → the pure `needs_approval_decision(tool_name, input, catalog)` with in-source `#[cfg(test)]` (`reads_never_need_approval`, `mutating_invoke_always_needs_approval`). The office classifier keys on server-id + tool-name + `mode` the same way control keys on tool-name + op mutating-ness.
- **Approval-loop integration test (ITEM-7)** — mirror `tests/mcp/mcp_approval_workflow_test.rs`: `create_test_mcp_server` (register a mock under `office_bridge_mcp_server_id()` exposing `run_office_js`), `set_mcp_settings` (ManualApprove + `auto_approved_tools`), `send_message_with_mcp`, `get_pending_approvals` — assert a `write` creates a pending approval and a `read` does not.
- **Tool prune + descriptor/tests (ITEM-1/3/7)** — mirror the `edit_document` removal in `office-run-office-js` (same `tool_list` exact-set + `EXPECTED_TOOLS` + arm/handler-test removal seams).
- **Pane op removal (ITEM-4)** — reverse of how the `op*` handlers were added; delete the functions + `case` lines + sole-use error code, exactly as in the prior `office-run-office-js` pane removal.

# PLAN — office-run-office-js

Replace the `office_bridge` `edit_document.op` enum (the "one tool/op per
Office.js API" anti-pattern) with a single open-ended **`run_office_js`** pane
tool: the model writes an Office.js script body, the connected task pane runs it
inside the host's `{Word,Excel,PowerPoint}.run(context => …)`, and returns the
script's value (or a structured error). This gives "everything Office.js
supports" at ~one tool-schema of context cost, and stops the `op` enum from
growing. `edit_document` (and the native `act_on_document` path it is the sole
consumer of) is removed; the 6 read/gated typed tools stay.

Grounded in the handoff writeup (`OFFICE_TOOL_SURFACE_DESIGN.md` +
`run_office_js` note) and the existing ITEM-9 pane RPC (`bridge/broker.rs` +
`taskpane.js` `dispatchOp`), which `run_office_js` is a natural new pane method
on top of — no broker/protocol change, just a new method string + pane handler.

## Items

- **ITEM-1**: Add the `run_office_js` tool descriptor to `tools.rs` `tool_list()` — `inputSchema` = `{ doc_full_name: string (required, from list_open_documents), script: string (required) }`, `additionalProperties:false`; description tells the model it writes an Office.js *body* that runs inside the host's `{Word,Excel,PowerPoint}.run(context => …)`, may `await context.sync()`, and should `return` a JSON-serializable value; the host app is auto-selected from the target document; requires the document's task pane to be open.
- **ITEM-2**: Remove the `edit_document` tool descriptor (and its `op` enum) from `tool_list()`; update the `tool_list` unit test to assert the new exact 7-tool set (drops `edit_document`, adds `run_office_js`). Also update the integration-tier `EXPECTED_TOOLS` const in `tests/office_bridge/settings_mcp_test.rs` (the `tools/list` HTTP assertion) the same way.
- **ITEM-3**: Add the `run_office_js` dispatch arm to `handlers.rs::dispatch_tool` — require `doc_full_name` (via `require_doc_full_name`) + a non-empty `script` (typed `INVALID_ARGS` otherwise), then route pane-mediated via `broker::call_pane(&doc_full_name, "run_office_js", args.clone())` and wrap with `pane_tool_result`. Host-agnostic (Word/Excel/PowerPoint) → NO PowerPoint pre-gate (unlike the Word-only comment/track tools). Per-call approval is inherited (office_bridge is absent from `is_builtin_server_id`) — no gating code needed (see DEC-2).
- **ITEM-4**: Remove the `edit_document` dispatch arm + the `EditDocumentArgs` struct from `handlers.rs`, and its two `#[cfg(test)]` unit tests (`test12_edit_document_append_returns_ok_and_read_back`, `test12_edit_document_append_empty_text_is_invalid_args`). Drop the now-unused `DocOp` import.
- **ITEM-5**: Remove the now-dead native edit path — `DocOp` (enum) + `ActResult` + the `act_on_document` trait method and all four impls (`MockOfficePlatform` in `platform/mod.rs`, `macos.rs`, `windows.rs`, `unsupported.rs`) plus their supporting helpers (`macos.rs` append-osascript helper, `windows.rs` COM append helper) and the direct `act_on_document` `#[cfg(test)]` tests in `platform/mod.rs` and `platform/unsupported.rs`. (Confirmed sole production consumer was `edit_document` at `handlers.rs:257`.)
- **ITEM-6**: Add the pane-side `run_office_js` handler to `taskpane.js` — a `dispatchOp` `case 'run_office_js'` → `opRunOfficeJs(id, params)` that compiles `params.script` as an async function body via `new Function('context', '"use strict"; return (async function(){' + script + '\n})()')` and runs it inside the host's `Word.run` / `Excel.run` / `PowerPoint.run(function(context){ … })` (chosen by `HOST`), resolves the returned value, and `reply(id, { result, truncated })`. On failure, `replyErr(id, ERR_OP_FAILED, <message>)` with a STRUCTURED message assembled from the Office.js error (`e.name`, `e.message`, `e.code`, `e.debugInfo`) so the daemon surfaces `OFFICE_PANE_ERROR` and the model self-corrects in one retry. Unknown/unsupported host → `ERR_UNSUPPORTED_HOST`.
- **ITEM-7**: Cap the `run_office_js` return value — a pure helper `serializeResult(value)` that JSON-serializes the script's return and applies the existing `capText`/`MAX_READ_CHARS` truncation, returning `{ result, truncated }`, so a huge return can't materialize an unbounded string across WSS into the LLM (mirrors `read_document`'s cap). Non-serializable returns degrade to a readable string, never throw. Add `serializeResult` to the `module.exports` block (alongside `capText`/`normPath`/…) so `taskpane.test.mjs` can unit-test it under node.
- **ITEM-8**: Update the stale doc/header comments that describe the old surface — `tools.rs` module header ("all seven office tools … `edit_document`'s `append_paragraph` op … native"), `handlers.rs` capability-model doc block, `bridge/protocol.rs` example list (`edit_document, …`), the `OpenDoc.full_name` doc comment in `platform/mod.rs` (currently "the stable handle callers pass back to `OfficePlatform::act_on_document`" — reword to reference the pane tools, since `act_on_document` is removed in ITEM-5), and `taskpane.js` responsibilities comment — to describe `run_office_js` + the `edit_document`/native-append removal. **`OpenDoc` is a REST OpenAPI schema (`GET /api/office-bridge/documents`), so rewording its field doc comment requires `just openapi-regen` (desktop only — office_bridge types live in the desktop crate; the ui spec is untouched).**
- **ITEM-9**: Document `run_office_js` for Windows in `WINDOWS_PANE_VERIFICATION.md` — same `taskpane.js` runs in WebView2, so add the live-verify step (open a doc, call `run_office_js`, observe the result) to the cross-platform checklist; note the removal of `edit_document` from the tool surface.

## Files to touch

- `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — ITEM-1, ITEM-2, ITEM-8 (descriptor add/remove + unit test + header)
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — ITEM-3, ITEM-4, ITEM-8 (dispatch arm add/remove + tests + doc block)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/mod.rs` — ITEM-5 (`DocOp`/`ActResult`/trait method/Mock impl + trait test)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/macos.rs` — ITEM-5 (impl + osascript append helper)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/windows.rs` — ITEM-5 (impl + COM append helper)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/unsupported.rs` — ITEM-5 (impl + test)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` — ITEM-6, ITEM-7, ITEM-8 (opRunOfficeJs + dispatchOp case + cap helper + comment)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.test.mjs` — ITEM-7 (node unit test for `serializeResult`)
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — ITEM-3, ITEM-6 (mock-pane `run_office_js` integration + live-mac op)
- `src-app/desktop/tauri/tests/office_bridge/settings_mcp_test.rs` — ITEM-2 (`EXPECTED_TOOLS` const + `tools/list` HTTP assertion)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/protocol.rs` — ITEM-8 (stale example doc comment)
- `WINDOWS_PANE_VERIFICATION.md` — ITEM-9 (cross-platform verify checklist)
- `src-app/desktop/ui/openapi/openapi.json` + `src-app/desktop/ui/src/api-client/types.ts` — ITEM-8 (MECHANICAL regen output from the `OpenDoc` doc-comment reword; produced by `just openapi-regen`, excluded from the phase-6 coverage law and phase-3/8 UI gates)

## Patterns to follow

- **Tool descriptor (ITEM-1/2)** — mirror the existing pane-mediated descriptors in the SAME `tools.rs` (`read_document` / `get_selection`): same `inputSchema` object shape, `required` array, prose style. The `tool_list` unit test mirrors the existing `tool_list_contains_all_seven_tools`.
- **Dispatch arm (ITEM-3/4)** — mirror the existing host-agnostic pane arm in the SAME `handlers.rs` (`"read_document" | "get_selection"` → `require_doc_full_name` → `broker::call_pane` → `pane_tool_result`). Argument validation mirrors the removed `edit_document`'s non-empty-`text` `INVALID_ARGS` check.
- **Pane handler (ITEM-6/7)** — mirror `opReadDocument` in the SAME `taskpane.js` (the `Word.run` / `Excel.run` host branches, `reply`/`replyErr`, the `capText`/`MAX_READ_CHARS` cap, the `.catch` structured-error shape). `PowerPoint.run` mirrors those two.
- **Platform removal (ITEM-5)** — no new pattern; delete along the exact seams `act_on_document`/`DocOp` occupy today across the four `platform/*` impls (reverse of how ITEM-9's `read_document` was added).
- **Integration test (ITEM-3/6)** — mirror the existing mock-pane tests in `tests/office_bridge/pane_rpc_test.rs` (`TEST-6/7/8` request/response over a mock pane) and the `#[cfg(target_os="macos")] #[ignore]` `test13_live_mac_pane_ops` for the live op.
- **Pane-helper unit test (ITEM-7)** — mirror the existing pure-helper tests in `taskpane.test.mjs` (`capText`/`normPath`/`sameDoc`) — node `--test`, `module.exports` under the `#[cfg]`-gated bootstrap.

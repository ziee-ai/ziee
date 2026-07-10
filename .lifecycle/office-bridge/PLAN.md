# PLAN — office-bridge (consolidated)

The office-bridge desktop feature, delivered on `feat/office-bridge` in five staged sub-features and
consolidated here into ONE branch-level lifecycle (base `origin/main`). ITEM/TEST/DEC ids are globally
renumbered (cumulative, collision-free) and grouped by stage; every original `[covers:]` mapping is preserved.

## Items

### Stage: Foundation — module, settings, watcher, bridge listener

- **ITEM-1**: `office_bridge` built-in module skeleton — `modules/office_bridge/{mod,routes,handlers,models,permissions,tools,repository}.rs`; deterministic server id `Uuid::new_v5(NAMESPACE_URL, b"office_bridge.ziee.internal")`; `#[distributed_slice(MODULE_ENTRIES)]` at a free `order` after mcp(65) (use 97); `impl AppModule` with `init()` (probe-gated per ITEM-6 + kill-switch + spawned idempotent loopback `upsert_builtin_server`) and `register_routes` merging the JSON-RPC MCP route + REST settings.
- **ITEM-2**: Migrations `132_create_office_bridge.sql` (singleton `office_bridge_settings` table, `id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id = TRUE)`, enabled + port + last-connect fields) and `133_grant_office_bridge_permissions_to_users.sql` (grant `office_bridge::use` to the Users group, idempotent `DO $$` block).
- **ITEM-3**: Permissions — `OfficeBridgeUse` (`office_bridge::use`), `OfficeBridgeAdminRead` (`office_bridge::admin::read`), `OfficeBridgeManage` (`office_bridge::admin::manage`) as compile-time `PermissionCheck` impls in `permissions.rs`.
- **ITEM-4**: Bridge cert mint — `bridge/cert.rs`: in-process `rcgen` self-signed cert, CN=localhost, `basicConstraints cA:true`, **SAN = localhost + 127.0.0.1 + ::1**; DER+PEM cached under the app data dir so the trusted cert persists across restarts.
- **ITEM-5**: Bridge HTTPS+WSS listener — `bridge/server.rs`: an axum `Router` served over rustls (`axum-server` bind_rustls) on **both `127.0.0.1:44300` and `[::1]:44300`** (dual-stack), serving embedded `/taskpane.html` + `/icon.png` (ITEM-12), a `/bridge` `WebSocketUpgrade` (same-origin JSON-RPC echo/RPC), and token-guarded POST sinks; Origin allowlist (`https://localhost:44300`) + per-session token (`bridge/auth.rs`, copying `llm_local_runtime/proxy.rs`: 32-byte OS-RNG → base64url, SHA-256 hash cache, constant-time compare).
- **ITEM-6**: `OfficePlatform` seam — `platform/mod.rs`: `#[async_trait]` trait (`probe() -> Option<OfficeCaps>`, `list_open_documents`, `watch(tx)`, `act_on_document`, `install_cert_trust`, `register_sideload`, `office_is_elevated`) + `#[cfg]`-selected `static ACTIVE` + `pub fn active()`, copied structurally from `code_sandbox/backend/mod.rs`.
- **ITEM-7**: Windows `OfficePlatform` impl — `platform/windows.rs` (`#[cfg(windows)]`): COM via the `windows` crate — `GetActiveObject` + late-bound `IDispatch` for Word/PowerPoint (Name/FullName/Saved/Path/active), Excel via `AccessibleObjectFromWindow` on `XLMAIN▸XLDESK▸EXCEL7` (OBJID_NATIVEOM, IID_IDispatch), `EnumWindows` presence fallback (`OpusApp/XLMAIN/PPTFrameClass`), `act_on_document` (InsertAfter+Save+read-back), `install_cert_trust` (`certutil -addstore -f Root` via elevated ShellExecute — one UAC), `register_sideload` (HKCU `Software\Microsoft\Office\16.0\WEF\Developer\<path>=<path>`), `office_is_elevated` (TokenElevation on each Office pid).
- **ITEM-8**: macOS `OfficePlatform` scaffold + unsupported fallback — `platform/macos.rs` (`#[cfg(target_os="macos")]`, every path `// UNVERIFIED — Mac spike`, `const MAC_TRANSPORT_VERIFIED: bool = false`): osascript/AppleScript enumerate+act, `security add-trusted-cert` Keychain install, drop manifest into `~/Library/Containers/com.microsoft.{Word,Excel,Powerpoint}/Data/Documents/wef`; `platform/unsupported.rs` (`probe()->None`, methods return NotSupported).
- **ITEM-9**: `office` MCP tool — `tools.rs::tool_list()` descriptors + `handlers::jsonrpc_handler` dispatch for `list_open_documents`, `read_document`, `edit_document`, `add_comment`, `set_track_changes`, `get_tracked_changes`, `get_selection`; native/cross-doc ops route to the daemon (`platform::active()`), in-document ops to that doc's pane over the bridge; every method feature-gated on host capability (PPT comments/track-changes → typed capability error).
- **ITEM-10**: Chat extension + `mcp.rs` edit — `chat_extension/{mod,extension,office_bridge}.rs`: `ATTACH_FLAG="attach_office_bridge_mcp"`, `#[distributed_slice(CHAT_EXTENSIONS)]` at `order` < 30 (use 29), `before_llm_call` sets the flag + a system nudge when open Office docs are present; **one edit** in `mcp/chat_extension/mcp.rs` — add the flag→`office_bridge_server_id()` branch in `auto_attach_builtin_ids`. `is_builtin_server_id` is deliberately NOT edited (mutating tool stays behind approval, like `control_mcp`).
- **ITEM-11**: Live open/close sync — a `SyncEntity::OfficeDocument` variant + the daemon watch loop (poll `list_open_documents`, diff, `sync_publish(OfficeDocument, action, id, Audience::owner(user), None)` on open/close) so the frontend panel updates live; mirrors the `sync` module conventions.
- **ITEM-12**: Add-in assets — `resources/office-bridge/{manifest.xml,taskpane.html,taskpane.js,icon.png}` embedded via `include_dir!` (skill/builtin.rs pattern); multi-host manifest (Document/Workbook/Presentation, one ShowTaskpane ribbon button each, shared `SourceLocation https://localhost:44300/taskpane.html`, `AppDomains`); host-aware taskpane JS (Office.onReady → open same-origin `wss://localhost:44300/bridge` with token → `DocumentSelectionChanged` + capability-gated ops), ported from the proven spike artifacts.
- **ITEM-13**: `[Connect]` installer flow — an admin-gated action (REST `POST /api/office-bridge/connect` + settings UI button) that runs mint→`install_cert_trust`(one UAC)→start bridge→`register_sideload`→detect Office present + detect/warn elevated Office; reports readiness state.
- **ITEM-14**: Frontend "Open Office documents" panel — `src-app/ui/src/modules/office-bridge/{module.tsx,types.ts,chat-extension/extension.tsx,components/OpenDocumentsPanel.tsx,stores/OfficeBridge.store.ts}`: register a chat panel renderer + a `tool_result` card (opens the panel) + a store subscribed to `sync:office_document`; shadcn/Radix components per DESIGN_SYSTEM.md + the `frontend-ui-engineering`/`shadcn-component-*` skills. Mirrors the `literature` panel tri.
- **ITEM-15**: OpenAPI + type regen — run `just openapi-regen` after the DTO/permission/sync-entity additions so `openapi.json` + `api-client/types.ts` (both ui and desktop) regenerate; keep the golden `openapi::emit_ts::tests::types_ts_parity` test green.

### Stage: Pane RPC — daemon↔pane JSON-RPC broker + 5 pane tools

- **ITEM-16**: Bridge broker — `bridge/broker.rs` (new). A process-global registry of
- **ITEM-17**: WSS socket loop — rewrite `bridge/server.rs::handle_socket` from an
- **ITEM-18**: Wire the 5 pane tools — `handlers.rs::dispatch_tool`: replace the
- **ITEM-19**: Task-pane RPC servicing — `resources/office-bridge/taskpane.js`: on WSS
- **ITEM-20**: Windows closeout — a `WINDOWS_PANE_VERIFICATION.md` manual live
- **ITEM-21**: Mac live verification + report — extend `MAC_OFFICE_BRIDGE_VERIFICATION.md`

### Stage: run_office_js — open-ended Office.js pane surface

- **ITEM-22**: Add the `run_office_js` tool descriptor to `tools.rs` `tool_list()` — `inputSchema` = `{ doc_full_name: string (required, from list_open_documents), script: string (required) }`, `additionalProperties:false`; description tells the model it writes an Office.js *body* that runs inside the host's `{Word,Excel,PowerPoint}.run(context => …)`, may `await context.sync()`, and should `return` a JSON-serializable value; the host app is auto-selected from the target document; requires the document's task pane to be open.
- **ITEM-23**: Remove the `edit_document` tool descriptor (and its `op` enum) from `tool_list()`; update the `tool_list` unit test to assert the new exact 7-tool set (drops `edit_document`, adds `run_office_js`). Also update the integration-tier `EXPECTED_TOOLS` const in `tests/office_bridge/settings_mcp_test.rs` (the `tools/list` HTTP assertion) the same way.
- **ITEM-24**: Add the `run_office_js` dispatch arm to `handlers.rs::dispatch_tool` — require `doc_full_name` (via `require_doc_full_name`) + a non-empty `script` (typed `INVALID_ARGS` otherwise), then route pane-mediated via `broker::call_pane(&doc_full_name, "run_office_js", args.clone())` and wrap with `pane_tool_result`. Host-agnostic (Word/Excel/PowerPoint) → NO PowerPoint pre-gate (unlike the Word-only comment/track tools). Per-call approval is inherited (office_bridge is absent from `is_builtin_server_id`) — no gating code needed (see DEC-23).
- **ITEM-25**: Remove the `edit_document` dispatch arm + the `EditDocumentArgs` struct from `handlers.rs`, and its two `#[cfg(test)]` unit tests (`test12_edit_document_append_returns_ok_and_read_back`, `test12_edit_document_append_empty_text_is_invalid_args`). Drop the now-unused `DocOp` import.
- **ITEM-26**: Remove the now-dead native edit path — `DocOp` (enum) + `ActResult` + the `act_on_document` trait method and all four impls (`MockOfficePlatform` in `platform/mod.rs`, `macos.rs`, `windows.rs`, `unsupported.rs`) plus their supporting helpers (`macos.rs` append-osascript helper, `windows.rs` COM append helper) and the direct `act_on_document` `#[cfg(test)]` tests in `platform/mod.rs` and `platform/unsupported.rs`. (Confirmed sole production consumer was `edit_document` at `handlers.rs:257`.)
- **ITEM-27**: Add the pane-side `run_office_js` handler to `taskpane.js` — a `dispatchOp` `case 'run_office_js'` → `opRunOfficeJs(id, params)` that compiles `params.script` as an async function body via `new Function('context', '"use strict"; return (async function(){' + script + '\n})()')` and runs it inside the host's `Word.run` / `Excel.run` / `PowerPoint.run(function(context){ … })` (chosen by `HOST`), resolves the returned value, and `reply(id, { result, truncated, text })` (`text` = the capped string form, so `pane_tool_result` surfaces the value in the readable content channel too). On failure, `replyErr(id, ERR_OP_FAILED, <message>)` with a STRUCTURED message assembled from the Office.js error (`e.name`, `e.message`, `e.code`, `e.debugInfo`) via a `describeError` helper so the daemon surfaces `OFFICE_PANE_ERROR` and the model self-corrects in one retry. Unknown/unsupported host → `ERR_UNSUPPORTED_HOST`.
- **ITEM-28**: Cap the `run_office_js` return value — a pure helper `serializeResult(value)` that JSON-serializes the script's return and applies the existing `capText`/`MAX_READ_CHARS` truncation, returning `{ result, truncated, text }` (native JSON value when it fits + round-trips, else the capped string; `text` = the capped string form), so a huge return can't materialize an unbounded string across WSS into the LLM (mirrors `read_document`'s cap). Non-serializable returns degrade to a readable string, never throw. Add `serializeResult` (and the `describeError` structured-error helper) to the `module.exports` block (alongside `capText`/`normPath`/…) so `taskpane.test.mjs` can unit-test them under node.
- **ITEM-29**: Update the stale doc/header comments that describe the old surface — `tools.rs` module header ("all seven office tools … `edit_document`'s `append_paragraph` op … native"), `handlers.rs` capability-model doc block, `bridge/protocol.rs` example list (`edit_document, …`), the `OpenDoc.full_name` doc comment in `platform/mod.rs` (currently "the stable handle callers pass back to `OfficePlatform::act_on_document`" — reword to reference the pane tools, since `act_on_document` is removed in ITEM-26), and `taskpane.js` responsibilities comment — to describe `run_office_js` + the `edit_document`/native-append removal. **`OpenDoc` is a REST OpenAPI schema (`GET /api/office-bridge/documents`), so rewording its field doc comment requires `just openapi-regen` (desktop only — office_bridge types live in the desktop crate; the ui spec is untouched).**
- **ITEM-30**: Document `run_office_js` for Windows in `WINDOWS_PANE_VERIFICATION.md` — same `taskpane.js` runs in WebView2, so add the live-verify step (open a doc, call `run_office_js`, observe the result) to the cross-platform checklist; note the removal of `edit_document` from the tool surface.

### Stage: Desktop-only relocation — module moved server→desktop

- **ITEM-31**: Relocate the module tree `src-app/server/src/modules/office_bridge/` →
- **ITEM-44**: Add the runtime registration seams in `ziee` (DEC-32), mirroring
- **ITEM-32**: Rewrite the module's cross-crate references: `crate::…` server-framework paths →
- **ITEM-33**: Widen `ziee`'s crate-root public facade with the EXACT set the module needs (all
- **ITEM-42**: Remove office_bridge from the `Repos` factory (`core/repository.rs:229`
- **ITEM-43**: `OfficeBridgeConfig` kill-switch — LEAVE the `Option<OfficeBridgeConfig>` section in
- **ITEM-34**: Invert the hardcoded coupling in `ziee` `src/modules/mcp/chat_extension/mcp.rs`:
- **ITEM-35**: Migrations — move `…133_create_office_bridge.sql` + `…134_grant_office_bridge…` from
- **ITEM-36**: `SyncEntity::OfficeDocument` — LEAVE the variant in `ziee`'s
- **ITEM-37**: Embedded add-in assets — move `src-app/server/resources/office-bridge/` into the
- **ITEM-38**: Frontend — move `src-app/ui/src/modules/office-bridge/` →
- **ITEM-39**: Tests — relocate the module's unit tests (move with the source) and the integration
- **ITEM-40**: Regenerate BOTH OpenAPI specs + `types.ts`: the **web** spec
- **ITEM-41**: Prove the negative: a plain `ziee` server build/spec contains **zero** office_bridge.

### Stage: Mode-gated approval — read auto-runs, write prompts

- **ITEM-45**: Prune the 5 typed tool descriptors from `tools.rs::tool_list()` → exactly `{list_open_documents, run_office_js}`. Update the `tool_list` unit tests to the 2-tool set (removed names absent).
- **ITEM-46**: Add `mode` (JSON enum `["read","write"]`, **required**) to `run_office_js`'s `inputSchema`, and enrich its `description`: tell the model to set `mode:"read"` ONLY for scripts that purely read (no property sets, no `insert*`/`delete`/`add`/`changeTrackingMode`/`insertComment`) and `mode:"write"` for anything that changes the document — because a `write` will ask the user for approval. Include brief Word examples for the now-inlined flows (adding a comment via `body.search(...).items[0].insertComment(...)`, toggling `document.changeTrackingMode`) so the model still discovers them.
- **ITEM-47**: In `handlers.rs::dispatch_tool` remove the two pruned dispatch arms (`"read_document" | "get_selection"` and `"add_comment" | "set_track_changes" | "get_tracked_changes"`) and the now-orphaned PowerPoint pre-gate they were the sole users of — `doc_host()`, `unsupported_on_ppt_err()`, and the **handlers.rs** `OFFICE_UNSUPPORTED_ON_HOST` const (audit-confirmed sole users; `broker::OFFICE_UNSUPPORTED_ON_HOST` STAYS — run_office_js still maps the pane's `-32002`). KEEP the `OfficeApp` import (used by `seeded_mock`). The `run_office_js` arm is UNCHANGED — the daemon does NOT read `mode` (execution is identical; `mode` is consumed only by the server approval loop). Remove the pruned tools' handler unit tests (`test12_add_comment_on_powerpoint_*`, `test12_set_track_changes_on_powerpoint_*`, `test10_add_comment_on_word_with_no_pane_*`) and **remove `test10_pane_mediated_method_no_pane_is_not_connected` entirely** — its loop is over `[get_selection, read_document, get_tracked_changes]` (all pruned), and its behavior (a pane-mediated tool with no pane → `OFFICE_PANE_NOT_CONNECTED`) is already covered for the sole survivor by `test6_run_office_js_no_pane_is_not_connected`. Keep `test12_list_open_documents`, `test4_edit_document_is_removed`, `test5_run_office_js_invalid_args`, `test6_run_office_js_no_pane`, `test12_unknown_tool`, `test16_run_connect_*`.
- **ITEM-48**: In `taskpane.js` remove `opReadDocument`, `opGetSelection`, `opAddComment`, `opSetTrackChanges`, `opGetTrackedChanges` + their `dispatchOp` `case` labels + the now-unused `ERR_ANCHOR_NOT_FOUND` code. KEEP `opRunOfficeJs` and every shared helper still used by it (`capText`/`MAX_READ_CHARS` via `serializeResult`, `serializeResult`, `describeError`, `safeString`, `ERR_UNSUPPORTED_HOST`, the mis-routing `sameDoc` guard, `openBridge`/`sendRegister`). Update the responsibilities comment to the 2-op surface.
- **ITEM-49**: (server) Extract the per-call approval decision currently inline at `mcp.rs:2120` (`let needs_approval = if is_control … else if is_builtin … else {ManualApprove/auto_approved}`) into a **pure, unit-testable function** `compute_needs_approval(server_id, tool_name, input, approval_mode, is_builtin, is_control, auto_approved_here) -> ApprovalOutcome` (mirrors how `control_mcp` factored `needs_approval_decision`). Behavior-preserving for all existing servers; call it from the loop.
- **ITEM-50**: (server) Add the office read-bypass to that decision. New server helpers in a small module (e.g. `mcp/chat_extension/office_approval.rs`): `office_bridge_mcp_server_id()` = `Uuid::new_v5(&NAMESPACE_URL, b"office_bridge.ziee.internal")` (same derivation the desktop uses), and `run_office_js_read_bypass(server_id, tool_name, input) -> bool` = `server_id == office_bridge_mcp_server_id() && tool_name == "run_office_js" && input.get("mode").and_then(as_str) == Some("read")` (EXACT `"read"` only). In `compute_needs_approval`, an office_bridge `run_office_js` with `mode=="read"` → bypass; **anything else on office_bridge falls through to the normal ManualApprove path** (write → prompt, or auto-run if the user picked always-allow). Fail-safe: `mode` missing / not exactly `"read"` / a non-office server that happens to name a tool `run_office_js` → NOT bypassed.
- **ITEM-51**: Update integration tests that name the pruned tools. `settings_mcp_test.rs::EXPECTED_TOOLS` → the 2-tool set. `pane_rpc_test.rs`: the **`dispatch_tool`-path** tests currently vehicled on pruned tools MUST retarget to `run_office_js` (they'd otherwise hit `UNKNOWN_TOOL`) — `test9_dispatch_tool_read_document_round_trip`, `test12_pane_error_propagates`, `test16_pane_unsupported_maps_to_unsupported_on_host` (the last preserves the `-32002` → `OFFICE_UNSUPPORTED_ON_HOST` coverage). The **`broker::call_pane`-direct** tests (`test6`, `test7`, `test8`, `test15`) forward the method string generically to the mock pane so they still pass, but retarget their method strings to `run_office_js` for cleanliness (no dead tool names). Add a `run_office_js` mock-pane round-trip assertion that `mode` is passed through and does NOT change execution (read vs write both round-trip identically — the daemon ignores `mode`).
- **ITEM-52**: (docs) Update `tools.rs` header, `handlers.rs` capability-model doc block, `taskpane.js` responsibilities comment, `WINDOWS_PANE_VERIFICATION.md` (2-tool surface + the read-auto / write-approval behavior), and `OFFICE_TOOL_SURFACE_DESIGN.md` (record the collapsed surface + the mode-gated permission model + the accepted trade-offs: trust-based, no read-only enforcement; auto-approved reads are a full-content exfiltration channel; "always allow" grants all later writes for the conversation).

## Files to touch

### office-bridge

- `src-app/server/src/modules/office_bridge/mod.rs`, `routes.rs`, `handlers.rs`, `models.rs`, `permissions.rs`, `tools.rs`, `repository.rs`
- `src-app/server/src/modules/office_bridge/bridge/{cert.rs,server.rs,auth.rs,protocol.rs}`
- `src-app/server/src/modules/office_bridge/platform/{mod.rs,windows.rs,macos.rs,unsupported.rs}`
- `src-app/server/src/modules/office_bridge/chat_extension/{mod.rs,extension.rs,office_bridge.rs}`
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (one edit: `auto_attach_builtin_ids`)
- `src-app/server/src/modules/sync/` (add `SyncEntity::OfficeDocument` variant)
- `src-app/server/src/modules/mod.rs` (declare `pub mod office_bridge;`)
- `src-app/server/migrations/00000000000132_create_office_bridge.sql`, `00000000000133_grant_office_bridge_permissions_to_users.sql`
- `src-app/server/resources/office-bridge/{manifest.xml,taskpane.html,taskpane.js,icon.png}`
- `src-app/server/Cargo.toml` (add `rcgen`, `axum` `ws` feature, `axum-server`+`rustls`, `windows` crate features — `#[cfg(windows)]`)
- `src-app/server/tests/office_bridge/{mod.rs,mcp_test.rs,settings_test.rs,bridge_test.rs}`, `src-app/server/tests/integration_tests.rs` (add `mod office_bridge;`)
- `src-app/ui/src/modules/office-bridge/{module.tsx,types.ts,chat-extension/extension.tsx,components/OpenDocumentsPanel.tsx,stores/OfficeBridge.store.ts}`
- `src-app/ui/tests/e2e/<n>-office-bridge/office-bridge.spec.ts`
- `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`, `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts` (mechanically regenerated)

### office-bridge-pane-rpc

- `src-app/desktop/tauri/src/modules/office_bridge/bridge/broker.rs` (**new**)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/mod.rs` (declare `broker`)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/server.rs` (`handle_socket` rewrite)
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` (`dispatch_tool` pane arms + new typed errors)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` (RPC servicing)
- `src-app/desktop/tauri/tests/office_bridge/mod.rs` (register the new test module)
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` (**new** — cross-platform mock-pane integration)
- `src-app/desktop/tauri/tests/office_bridge/bridge_test.rs` (update: the removed echo)
- `WINDOWS_PANE_VERIFICATION.md` (**new**, repo root — Windows manual live checklist, DRIFT-1.1)
- `MAC_OFFICE_BRIDGE_VERIFICATION.md` (extend)

### office-run-office-js

- `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — ITEM-22, ITEM-23, ITEM-29 (descriptor add/remove + unit test + header)
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — ITEM-24, ITEM-25, ITEM-29 (dispatch arm add/remove + tests + doc block)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/mod.rs` — ITEM-26 (`DocOp`/`ActResult`/trait method/Mock impl + trait test)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/macos.rs` — ITEM-26 (impl + osascript append helper)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/windows.rs` — ITEM-26 (impl + COM append helper)
- `src-app/desktop/tauri/src/modules/office_bridge/platform/unsupported.rs` — ITEM-26 (impl + test)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` — ITEM-27, ITEM-28, ITEM-29 (opRunOfficeJs + dispatchOp case + cap helper + comment)
- `src-app/desktop/tauri/resources/office-bridge/taskpane.test.mjs` — ITEM-28 (node unit test for `serializeResult`)
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — ITEM-24, ITEM-27 (mock-pane `run_office_js` integration + live-mac op)
- `src-app/desktop/tauri/tests/office_bridge/settings_mcp_test.rs` — ITEM-23 (`EXPECTED_TOOLS` const + `tools/list` HTTP assertion)
- `src-app/desktop/tauri/src/modules/office_bridge/bridge/protocol.rs` — ITEM-29 (stale example doc comment)
- `WINDOWS_PANE_VERIFICATION.md` — ITEM-30 (cross-platform verify checklist)
- `src-app/desktop/ui/openapi/openapi.json` + `src-app/desktop/ui/src/api-client/types.ts` — ITEM-29 (MECHANICAL regen output from the `OpenDoc` doc-comment reword; produced by `just openapi-regen`, excluded from the phase-6 coverage law and phase-3/8 UI gates)

### office-bridge-desktop-only

- **Delete from `ziee` server:** `src-app/server/src/modules/office_bridge/**` (whole tree),
  `src-app/server/migrations/00000000000133_create_office_bridge.sql`,
  `src-app/server/migrations/00000000000134_grant_office_bridge_permissions_to_users.sql`,
  `src-app/server/resources/office-bridge/**`, `src-app/server/tests/office_bridge/**`,
  and the office_bridge branch in `src-app/server/src/modules/mcp/chat_extension/mcp.rs`.
- **Edit in `ziee` server:** `src/modules/mcp/chat_extension/mcp.rs` (new AUTO_ATTACH_BUILTINS
  slice + iterate it), `src/lib.rs` / `src/module_api/**` (widen the public surface ITEM-33 needs),
  `src/modules/server/mod.rs` or wherever `create_modules` self-test counts modules.
- **Add to desktop crate:** `src-app/desktop/tauri/src/modules/office_bridge/**` (relocated tree),
  `src-app/desktop/tauri/migrations/10000000000006_create_office_bridge.sql` +
  `…0007_grant_office_bridge_permissions_to_users.sql`,
  `src-app/desktop/tauri/resources/office-bridge/**`, `src-app/desktop/tauri/tests/office_bridge/**`,
  register the module in `src-app/desktop/tauri/src/modules/mod.rs`.
- **Frontend:** move `src-app/ui/src/modules/office-bridge/**` →
  `src-app/desktop/ui/src/modules/office-bridge/**`; update both UIs' module registries;
  move the e2e spec `ui/tests/e2e/20-office-bridge/**` → `desktop/ui/tests/e2e/…`.
- **Regenerated (committed):** `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`,
  `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts`, and the
  gallery/kit generated files in whichever workspace changed.

### office-mode-gated-approval

- `src-app/desktop/tauri/src/modules/office_bridge/tools.rs` — ITEM-45, ITEM-46, ITEM-52
- `src-app/desktop/tauri/src/modules/office_bridge/handlers.rs` — ITEM-47, ITEM-52
- `src-app/desktop/tauri/resources/office-bridge/taskpane.js` — ITEM-48, ITEM-52
- `src-app/desktop/tauri/tests/office_bridge/settings_mcp_test.rs` — ITEM-51
- `src-app/desktop/tauri/tests/office_bridge/pane_rpc_test.rs` — ITEM-51
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — ITEM-49, ITEM-50 (call the extracted fn)
- `src-app/server/src/modules/mcp/chat_extension/office_approval.rs` (NEW) — ITEM-49, ITEM-50 (`compute_needs_approval` + office classifier + `office_bridge_mcp_server_id` + unit tests)
- (extraction behaviour-preservation is proven deterministically by TEST-78 + the phase-6 blind equivalence audit; the EXISTING `mcp_approval_workflow_test.rs` LLM-driven suite is env-gated — no LLM key here — and office is desktop-only, so it is not run as the proof; see DRIFT-1 + TEST_RESULTS.md)
- `src-app/desktop/tauri/src/modules/office_bridge/mod.rs` — ITEM-50 (a drift test asserting the desktop `office_bridge` row id equals the server's `office_bridge_mcp_server_id()`)
- `WINDOWS_PANE_VERIFICATION.md`, `OFFICE_TOOL_SURFACE_DESIGN.md` — ITEM-52

## Patterns to follow

### office-bridge

- **Built-in MCP module skeleton + registration + loopback upsert + JSON-RPC handler + the mcp.rs auto-attach edit + permissions + grant migration**: mirror `src-app/server/src/modules/web_search/` (mod/routes/handlers/models/permissions/tools/repository + `chat_extension/`) and `migrations/…097/098`.
- **Cross-platform OS seam (`OfficePlatform` trait + `#[cfg]` `ACTIVE` + `active()` + probe-gated registration + a mock impl for tests)**: mirror `src-app/server/src/modules/code_sandbox/backend/mod.rs` and the trait+mock shape of `desktop/tauri/src/modules/remote_access/tunnel.rs`.
- **Per-session token auth (mint/hash-cache/constant-time verify)**: mirror `src-app/server/src/modules/llm_local_runtime/proxy.rs`.
- **Windows native FFI**: mirror the `windows-sys` usage + TCP-listener enumeration in `src-app/server/src/modules/llm_local_runtime/deployment/local.rs::verify_loopback_bind` for style; COM/IDispatch via the `windows` crate.
- **Embedded resource dir**: mirror `src-app/server/src/skill/builtin.rs` (`include_dir!("$CARGO_MANIFEST_DIR/resources/…")`).
- **Realtime open/close → panel**: mirror `src-app/server/src/modules/sync/` (`sync_publish` + `SyncEntity`) and the SSE handler shape.
- **Frontend chat panel + tool_result card + store-subscribes-to-sync**: mirror `src-app/ui/src/modules/literature/{module.tsx,chat-extension/extension.tsx,components/LiteratureScreeningPanel.tsx,types.ts}`, using shadcn/Radix per `src-app/ui/src/components/ui/KIT_MANIFEST.md` + `DESIGN_SYSTEM.md` and the `frontend-ui-engineering` / `shadcn-component-discovery` / `shadcn-component-review` skills.
- **OpenAPI regen + golden parity**: `just openapi-regen`; keep `src-app/server/src/openapi/emit_ts.rs` `types_ts_parity` green.
- **Integration-test layout**: mirror `src-app/server/tests/web_search/` (fixtures in `mod.rs` + `*_test.rs`, `mod office_bridge;` in `tests/integration_tests.rs`).

### office-bridge-pane-rpc

- **Broker registry + process-global state** → mirror `bridge/auth.rs`
  (`LazyLock<RwLock<…>>`, poison-recovering `unwrap_or_else(|p| p.into_inner())`,
  bounded store) for the pane/pending maps.
- **Correlation-over-a-channel + oneshot reply** → mirror
  `mcp/elicitation/registry.rs` (a `Lazy<Mutex<HashMap<id, {tx: oneshot::Sender}>>>`
  with `register` / `respond` / `take`, poison-recovering) — the repo's exact
  "register a pending request keyed by id, resolve it from another task via a
  oneshot" idiom (the ask_user/elicitation correlation). The `call_pane` timeout
  wraps the oneshot recv in `tokio::time::timeout`.
- **JSON-RPC envelopes** → reuse the existing `bridge/protocol.rs`
  (`BridgeRequest`/`BridgeResponse`/`BridgeError`) verbatim; no new wire types.
- **Typed tool errors** → mirror the existing `handlers.rs` `pane_required_err` /
  `unsupported_on_ppt_err` constructors (`AppError::new(StatusCode, CODE, msg)`).
- **Mock-pane integration test** → mirror `tests/office_bridge/bridge_test.rs`
  (ephemeral `server::start(0, tempdir)`, a `tokio-tungstenite` client acting as the
  pane) — extend it with a client that answers a daemon→pane request.
- **`#[cfg(windows)] #[ignore]` live test** → mirror `tests/office_bridge/windows_com_test.rs`.
- **Task-pane JS** → mirror the existing `resources/office-bridge/taskpane.js`
  structure (`Office.onReady`, `send`, `log`); keep it dependency-free ES5.

### office-run-office-js

- **Tool descriptor (ITEM-22/2)** — mirror the existing pane-mediated descriptors in the SAME `tools.rs` (`read_document` / `get_selection`): same `inputSchema` object shape, `required` array, prose style. The `tool_list` unit test mirrors the existing `tool_list_contains_all_seven_tools`.
- **Dispatch arm (ITEM-24/4)** — mirror the existing host-agnostic pane arm in the SAME `handlers.rs` (`"read_document" | "get_selection"` → `require_doc_full_name` → `broker::call_pane` → `pane_tool_result`). Argument validation mirrors the removed `edit_document`'s non-empty-`text` `INVALID_ARGS` check.
- **Pane handler (ITEM-27/7)** — mirror `opReadDocument` in the SAME `taskpane.js` (the `Word.run` / `Excel.run` host branches, `reply`/`replyErr`, the `capText`/`MAX_READ_CHARS` cap, the `.catch` structured-error shape). `PowerPoint.run` mirrors those two.
- **Platform removal (ITEM-26)** — no new pattern; delete along the exact seams `act_on_document`/`DocOp` occupy today across the four `platform/*` impls (reverse of how ITEM-30's `read_document` was added).
- **Integration test (ITEM-24/6)** — mirror the existing mock-pane tests in `tests/office_bridge/pane_rpc_test.rs` (`TEST-43/7/8` request/response over a mock pane) and the `#[cfg(target_os="macos")] #[ignore]` `test13_live_mac_pane_ops` for the live op.
- **Pane-helper unit test (ITEM-28)** — mirror the existing pure-helper tests in `taskpane.test.mjs` (`capText`/`normPath`/`sameDoc`) — node `--test`, `module.exports` under the `#[cfg]`-gated bootstrap.

### office-bridge-desktop-only

- **Desktop server-side module registration:** there is no existing desktop module that registers
  a full `ziee` `AppModule` via `MODULE_ENTRIES` (existing desktop modules use the Tauri-side
  `DesktopModule` trait). Mirror the **server** built-in-MCP modules for the module SHAPE
  (`src-app/server/src/modules/web_search/` — the closest sibling: built-in MCP server + chat
  extension + settings + permissions), but place the files in the desktop crate and reference the
  framework via `ziee::` (the way `desktop/tauri/src/modules/remote_access/` already uses
  `sqlx::query_as!` against its own desktop-migration tables).
- **Desktop migrations:** mirror `src-app/desktop/tauri/migrations/10000000000003_create_remote_access_settings.sql`
  (the `1000…` numbering + `set_ignore_missing` coexistence) and its application via
  `run_desktop_migrations` (`desktop/tauri/src/modules/backend/mod.rs`).
- **AUTO_ATTACH_BUILTINS distributed slice:** mirror the existing `MODULE_ENTRIES` /
  `CHAT_EXTENSIONS` slice definitions (`module_api/types.rs`, `chat/core/extension/registry.rs`)
  for the `#[distributed_slice]` pattern.
- **Desktop OpenAPI merge:** already implemented in `src-app/desktop/tauri/src/openapi.rs`
  (`create_modules()` + `create_desktop_modules()` routes merged) — no change, just regenerate.
- **Frontend module in desktop UI:** mirror how `src-app/desktop/ui/` registers its modules
  (whatever the desktop-ui module registry is) and the existing shadcn office-bridge components
  (unchanged, just relocated).

### office-mode-gated-approval

- **Approval classifier + extraction (ITEM-49/6)** — mirror `control_mcp/handlers.rs`: `control_call_needs_approval` → the pure `needs_approval_decision(tool_name, input, catalog)` with in-source `#[cfg(test)]` (`reads_never_need_approval`, `mutating_invoke_always_needs_approval`). The office classifier keys on server-id + tool-name + `mode` the same way control keys on tool-name + op mutating-ness.
- **Approval-loop integration test (ITEM-51)** — mirror `tests/mcp/mcp_approval_workflow_test.rs`: `create_test_mcp_server` (register a mock under `office_bridge_mcp_server_id()` exposing `run_office_js`), `set_mcp_settings` (ManualApprove + `auto_approved_tools`), `send_message_with_mcp`, `get_pending_approvals` — assert a `write` creates a pending approval and a `read` does not.
- **Tool prune + descriptor/tests (ITEM-45/3/7)** — mirror the `edit_document` removal in `office-run-office-js` (same `tool_list` exact-set + `EXPECTED_TOOLS` + arm/handler-test removal seams).
- **Pane op removal (ITEM-48)** — reverse of how the `op*` handlers were added; delete the functions + `case` lines + sole-use error code, exactly as in the prior `office-run-office-js` pane removal.

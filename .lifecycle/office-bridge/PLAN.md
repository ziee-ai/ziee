# Office Bridge — PLAN

Feature: an Office.js task-pane add-in that talks to ziee over its own locally-trusted
`https://localhost` + same-origin **WSS** bridge, a non-elevated native (COM) daemon that
enumerates/acts on open Word/Excel/PowerPoint documents, an "Open Office documents" chat panel, and
an `office` built-in MCP tool. **Windows fully implemented + tested; macOS seam + scaffold gated on
one unproven cert-trust unknown.** All office-side facts are empirically proven (see the session
spike reports); this plan integrates them into ziee following the existing built-in-MCP-module +
cross-platform-seam patterns.

## Items

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

## Files to touch

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

## Patterns to follow

- **Built-in MCP module skeleton + registration + loopback upsert + JSON-RPC handler + the mcp.rs auto-attach edit + permissions + grant migration**: mirror `src-app/server/src/modules/web_search/` (mod/routes/handlers/models/permissions/tools/repository + `chat_extension/`) and `migrations/…097/098`.
- **Cross-platform OS seam (`OfficePlatform` trait + `#[cfg]` `ACTIVE` + `active()` + probe-gated registration + a mock impl for tests)**: mirror `src-app/server/src/modules/code_sandbox/backend/mod.rs` and the trait+mock shape of `desktop/tauri/src/modules/remote_access/tunnel.rs`.
- **Per-session token auth (mint/hash-cache/constant-time verify)**: mirror `src-app/server/src/modules/llm_local_runtime/proxy.rs`.
- **Windows native FFI**: mirror the `windows-sys` usage + TCP-listener enumeration in `src-app/server/src/modules/llm_local_runtime/deployment/local.rs::verify_loopback_bind` for style; COM/IDispatch via the `windows` crate.
- **Embedded resource dir**: mirror `src-app/server/src/skill/builtin.rs` (`include_dir!("$CARGO_MANIFEST_DIR/resources/…")`).
- **Realtime open/close → panel**: mirror `src-app/server/src/modules/sync/` (`sync_publish` + `SyncEntity`) and the SSE handler shape.
- **Frontend chat panel + tool_result card + store-subscribes-to-sync**: mirror `src-app/ui/src/modules/literature/{module.tsx,chat-extension/extension.tsx,components/LiteratureScreeningPanel.tsx,types.ts}`, using shadcn/Radix per `src-app/ui/src/components/ui/KIT_MANIFEST.md` + `DESIGN_SYSTEM.md` and the `frontend-ui-engineering` / `shadcn-component-discovery` / `shadcn-component-review` skills.
- **OpenAPI regen + golden parity**: `just openapi-regen`; keep `src-app/server/src/openapi/emit_ts.rs` `types_ts_parity` green.
- **Integration-test layout**: mirror `src-app/server/tests/web_search/` (fixtures in `mod.rs` + `*_test.rs`, `mod office_bridge;` in `tests/integration_tests.rs`).

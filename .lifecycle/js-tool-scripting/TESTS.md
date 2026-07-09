# TESTS — js-tool-scripting

Tiers mirror the codebase: unit = in-source `#[cfg(test)]` (Rust) / `*.test.ts`
(frontend `npm run test:unit`); integration = `src-app/server/tests/js_tool/`;
e2e = `src-app/ui/tests/e2e/`. Every ITEM is covered ≥1×. No cosmetic tests: the
integration + e2e tests exercise the REAL dispatcher / real rquickjs runtime /
real elicitation resolve path — only the LLM/HTTP upstreams are mocked.

## Runtime & caps (ITEM-2) — the security core
- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: an `AsyncRuntime`/`AsyncContext` evaluates `1+1` → `2`, proving the `rquickjs` dep + `macro`+`futures` features + default allocator are wired.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: the script wrapper `(async()=>{return 21*2})()` awaits its Promise and returns final value `42`.
- **TEST-3** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: `console.log`/`warn`/`error` are captured into `console_lines` in order and truncated at the console cap.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: a throwing script returns `error{message, line}` with a nonzero line number (self-correction signal).
- **TEST-5** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: `while(true){}` is killed by the interrupt handler within the CPU/gas bound and returns a "CPU limit exceeded" error rather than hanging.
- **TEST-6** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: allocating past `set_memory_limit` raises a caught `OutOfMemory` and returns a memory-limit error (verifies the cap is live under the default allocator).
- **TEST-7** (tier: unit) [covers: ITEM-2, ITEM-11] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: an oversized final value is truncated to the output cap with a truncation marker.
- **TEST-37** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: a script sees NO ambient capability — `typeof require`, `typeof fetch`, `typeof process`, `typeof globalThis.Deno`, and any fs/net access are all `undefined`/throw; only injected `ziee.*` exists.

## Host bridge (ITEM-3)
- **TEST-8** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/js_tool/host_bridge.rs` — asserts: unique tool names bind to `ziee.tools.<name>`; a name collision across two servers binds to `<server>_<tool>`; `ziee.toolList()` returns each tool's name + JSON schema.
- **TEST-9** (tier: integration) [covers: ITEM-3, ITEM-9] file: `src-app/server/tests/js_tool/mod.rs` — asserts: a script calling `ziee.tools.<tool>()` against a `MockMcpServer` dispatches through the REAL MCP dispatcher, the result is returned into the script, and a `mcp_tool_calls` row is recorded with `source='script'` (proves migration 133 admits `'script'`).
- **TEST-10** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/js_tool/host_bridge.rs` — asserts: after `max_tool_calls` host-fn invocations the next call throws a JS error (over-cap, no silent truncation).

## Approval suspend/resume (ITEM-4)
- **TEST-11** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/run-js-inner-approval.spec.ts` — asserts: a script's GATED sub-tool surfaces an approve/deny prompt; clicking APPROVE resolves via the side-channel `/respond` and the run_js card reaches its final result (the full suspend→approve→resume round-trip is a UI/SSE flow, best exercised end-to-end).
- **TEST-12** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/run-js-inner-approval.spec.ts` — asserts: clicking DENY resolves via `/respond`; the script catches `ToolApprovalDenied` and the run_js card shows its fallback value.
- **TEST-13** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/js_tool/approval.rs` — asserts: `request_approval` that is never answered resolves via the timeout arm as `Denied` (the suspend/resume primitive; the stream-closed arm is `test_stream_closed_denies`).
- **TEST-14** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/js_tool/approval.rs` — asserts: the needs-approval decision matches the normal loop — `is_builtin` server → bypass (no prompt); control-mutating `invoke_capability` → prompt; `ManualApprove` + not per-tool-allowlisted → prompt.

## Executor + context economics (ITEM-5)
- **TEST-15** (tier: integration) [covers: ITEM-5, ITEM-8] file: `src-app/server/tests/js_tool/mod.rs` — asserts: an end-to-end `run_js` tool_use (via the chat path with a `MockMcpServer`) whose script loops a read-only tool over N items and returns a summary produces a SINGLE `run_js` `tool_result` (the summary); the N intermediate results are NOT appended as separate `tool_result` blocks in the message history.
- **TEST-16** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/js_tool/runtime.rs` — asserts: a pure-CPU spin is killed by the gas interrupt (`test_cpu_interrupt_kills_infinite_loop`) and a post-await runaway by the shared cancel flag (`test_cancel_flag_terminates`); the watchdog's pending-counter (approval-wait excluded from the active budget) is verified by the integration approval path.

## Built-in registration (ITEM-6)
- **TEST-17** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/js_tool/mod.rs` — asserts: `run_js_mcp_server_id()` is the stable `Uuid::new_v5(NAMESPACE_URL, b"run_js.ziee.internal")` (deterministic, unchanging).
- **TEST-18** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/js_tool/mod.rs` — asserts: after boot the `mcp_servers` row exists with `is_built_in=true`, `is_system=true`, `transport_type='http'`, and a loopback `/api/run-js/mcp` url (mirrors bio's editable-builtin registration test).
- **TEST-19** (tier: integration) [covers: ITEM-6, ITEM-10] file: `src-app/server/tests/js_tool/mod.rs` — asserts: the loopback handler answers `initialize` + `tools/list` (returns the `run_js` descriptor), returns a "must be invoked in chat context" error for `tools/call`, and gates on `js_tool::use` (401 unauth / 403 without perm).

## Chat-extension attach flag (ITEM-7)
- **TEST-20** (tier: unit) [covers: ITEM-7] file: `src-app/server/src/modules/js_tool/chat_extension/js_tool.rs` — asserts: `before_llm_call` sets `attach_run_js_mcp="true"` + prepends the generic system nudge when tool-capable AND enabled; sets nothing when disabled or not tool-capable.

## The two mcp.rs edits + intercept (ITEM-8)
- **TEST-21** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `auto_attach_builtin_ids` includes the run_js id when `attach_run_js_mcp` is set; `is_builtin_server_id(run_js_mcp_server_id())` is `true` (extends the existing shared approval-bypass-seam test).

## source='script' + migration (ITEM-9)
- **TEST-23** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/modules/mcp/tool_calls/models.rs` — asserts: `McpToolCallSource::Script.as_str() == "script"` and it round-trips through serde snake_case.

## Permission + config (ITEM-10)
- **TEST-25** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/js_tool/mod.rs` — asserts: a default Users-group member is granted `js_tool::use` (migration 134) and can reach the handler; a user without it gets 403.
- **TEST-26** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/core/config.rs` — asserts: `JsToolConfig` defaults `enabled=true`; a `js_tool.enabled=false` deploy config is parsed and (verified in TEST-20's disabled branch) suppresses the attach flag.

## Result shape + self-correction (ITEM-11)
- **TEST-27** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/js_tool/mod.rs` — asserts: the `run_js` result's `structured_content` carries `{ result, console[], tool_calls[] }` with the per-sub-tool trace, and `ziee://` host paths are scrubbed from it.
- **TEST-28** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/js_tool/mod.rs` — asserts: a script that throws returns a result whose `error` carries message + line (the one-retry self-correction contract), not a transport failure.

## Module registration + desktop parity (ITEM-12)
- **TEST-29** (tier: unit) [covers: ITEM-12] file: `src-app/server/src/modules/js_tool/mod.rs` — asserts: the `js_tool` `AppModule` is registered in the module set and its module order sits in the built-in band without colliding (compile + registry presence check).

## Frontend inner-approval (ITEM-13)
- **TEST-30** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/chat/run-js-inner-approval.spec.ts` — asserts: resolving a run_js approval issues `POST /api/mcp/elicitation/{id}/respond` (observed via network mock) with the approve/deny payload (the resolve wiring, exercised through the real component).
- **TEST-31** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/chat/run-js-inner-approval.spec.ts` — asserts: model emits a `run_js` call → the script's gated sub-tool surfaces an approve/deny prompt (`tool-approval-*` testids) → clicking approve resolves via the side-channel and the run_js card reaches its final result; a deny variant shows the script's fallback.

## Source tone + gallery coverage (ITEM-14)
- **TEST-32** (tier: e2e) [covers: ITEM-14] file: `src-app/ui/tests/e2e/chat/run-js-tool-scripting.spec.ts` — asserts: the McpToolCallsTab history lists the run_js sub-tool calls tagged with the `script` source (its tone/label rendered, no fallback-default).
- **TEST-33** (tier: e2e) [covers: ITEM-14] file: `src-app/ui/tests/e2e/visual/gallery-runtime.spec.ts` — asserts: the new `run_js` call-card + inner-approval gallery deep-states render across light/dark themes with zero runtime HIGH findings (console-error/page-error/request-failed/AA-contrast), satisfying `check:state-matrix`/`check:gallery-coverage`.

## OpenAPI regen parity (ITEM-15)
- **TEST-34** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden test is green after regen (the committed `types.ts` matches regeneration from `openapi.json`, incl. the new `McpToolCallSource` `Script` value).

## Primary flow + provider-agnostic e2e (ITEM-5, ITEM-11, cross-cutting)
- **TEST-35** (tier: e2e) [covers: ITEM-5, ITEM-11] file: `src-app/ui/tests/e2e/chat/run-js-tool-scripting.spec.ts` — asserts: with a mocked SSE stream, the model emits a `run_js` call whose script loops a read-only tool; the chat renders ONE run_js tool card with the final summary (not N intermediate cards), and the `McpToolCallsTab` history lists the sub-tool calls tagged source `script`.

- **TEST-36** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/run-js-real-llm.spec.ts` — asserts: a real tool-capable model (provider-agnostic; wired to the local Qwen bridge via `OPENAI_BASE_URL`/`ZIEE_TEST_LLM_MODEL`, self-skips otherwise per the repo's real-LLM convention) CHOOSES to call `run_js`, the embedded QuickJS runtime executes the script in-process (a run_js `tool_use`→`tool_result`), the card reaches `completed` (no error), and the model's answer reflects the computed value (6*7=42) — proving the capability is provider-independent end-to-end. Ran GREEN against qwen3.6-35b-a3b (see DRIFT-2, resolved).

## Admin-configurable limits increment (ITEM-16..27)

### Model + validate + mapping + cache (unit)
- **TEST-38** (tier: unit) [covers: ITEM-17] file: `src-app/server/src/modules/js_tool/settings.rs` — asserts: `UpdateJsToolSettings::validate()` accepts an empty patch + each field's in-range boundary, and rejects each field's out-of-range value (memory<16MiB, stack<64KiB, wall 0 or >3600, approval<5, runs 0 or >256, dispatch 0 or >64, trace 0 or >10000) with a 422 `JS_TOOL_LIMIT_OUT_OF_RANGE`.
- **TEST-39** (tier: unit) [covers: ITEM-20] file: `src-app/server/src/modules/js_tool/limits.rs` — asserts: `JsCaps::from_settings(&s)` maps memory_bytes/max_stack_bytes into `runtime` (`JsLimits`), wall_secs/approval_timeout_secs into the `Duration`s, and max_concurrent_dispatch/max_trace_entries into the new `JsCaps` fields; `gas`/`output_bytes`/`console_bytes`/`max_tool_calls`/`max_approvals` retain their default values (not settings-driven).
- **TEST-40** (tier: unit) [covers: ITEM-19] file: `src-app/server/src/modules/js_tool/settings_cache.rs` — asserts: `defaults()` returns exactly the migration-135 DEFAULTs (128 MiB / 512 KiB / 300 / 300 / 8 / 6 / 256), so a DB-unavailable fallback matches a fresh install.
- **TEST-41** (tier: unit) [covers: ITEM-21] file: `src-app/server/src/modules/js_tool/executor.rs` — asserts: the global-run-sem resize helper grows available permits via `add_permits` and shrinks via `forget_permits` for a delta (no underflow at the configured floor), and `JsCaps` carries `max_concurrent_dispatch`/`max_trace_entries` that the dispatcher/trace paths read (compile + a focused assertion on the permit math).

### REST GET/PUT + gate + validation + sync (integration)
- **TEST-42** (tier: integration) [covers: ITEM-16, ITEM-24] file: `src-app/server/tests/js_tool/settings.rs` — asserts: `GET /api/js-tool/settings` as an admin returns 200 with the seeded default row (proves migration 135 created + seeded the singleton).
- **TEST-43** (tier: integration) [covers: ITEM-18, ITEM-24] file: `src-app/server/tests/js_tool/settings.rs` — asserts: `PUT /api/js-tool/settings` with a partial patch (e.g. `wall_secs`) returns 200 with the updated row, and a subsequent `GET` reflects the change (COALESCE PATCH persists; other fields untouched).
- **TEST-44** (tier: integration) [covers: ITEM-17, ITEM-24] file: `src-app/server/tests/js_tool/settings.rs` — asserts: `PUT` with an absurd value (e.g. `memory_bytes: 1` and separately `max_concurrent_runs: 100000`) returns **422** and does NOT mutate the row (validation-rejects-absurd; the admin can't footgun the server).
- **TEST-45** (tier: integration) [covers: ITEM-23] file: `src-app/server/tests/js_tool/settings.rs` — asserts: a plain Users-group member (has `js_tool::use` but NOT `js_tool::settings::*`) gets **403** on both GET and PUT; an admin gets 200 (the read/manage gate).
- **TEST-46** (tier: integration) [covers: ITEM-24] file: `src-app/server/tests/js_tool/settings.rs` — asserts: an unauthenticated GET/PUT returns **401**.
- **TEST-47** (tier: integration) [covers: ITEM-19, ITEM-21, ITEM-22] file: `src-app/server/tests/js_tool/settings.rs` — asserts: **db-value-honored-at-execution** — after `PUT`ing a tiny `memory_bytes` (16 MiB), a `run_js` script (stub-model chat path) that allocates past it returns the memory-limit error, proving the DB value flows cache→`JsCaps::from_settings`→the live evaluation (and the PUT cache-invalidate took effect without a restart).
- **TEST-48** (tier: integration) [covers: ITEM-25] file: `src-app/server/tests/js_tool/settings.rs` — asserts: a successful `PUT` emits a `SyncEntity::JsToolSettings` `Update` event (observed via `SyncProbe`) to the `JsToolSettingsRead` audience.
- **TEST-49** (tier: unit) [covers: ITEM-27] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` + `types_ts_parity_desktop` golden tests stay green after regen with the new `JsToolSettings`/`UpdateJsToolSettings` schemas, `JsTool.get/updateSettings` operations, `SyncEntity::JsToolSettings`, and `Permissions.JsToolSettings{Read,Manage}`.

### Admin settings page (e2e)
- **TEST-50** (tier: e2e) [covers: ITEM-26] file: `src-app/ui/tests/e2e/settings/js-tool-settings.spec.ts` — asserts: an admin opens `/settings/js-tool`, the current limits render in the form, editing a numeric field (e.g. wall seconds) + Save shows success and the value persists after a reload (GET/PUT round-trip through the real page).
- **TEST-51** (tier: e2e) [covers: ITEM-26] file: `src-app/ui/tests/e2e/settings/js-tool-settings.spec.ts` — asserts: entering an out-of-range value surfaces a validation error (client zod and/or the 422) and the save is rejected — validation-rejects-absurd, visible in the UI.
- **TEST-52** (tier: e2e) [covers: ITEM-26, ITEM-25] file: `src-app/ui/tests/e2e/settings/js-tool-settings.spec.ts` — asserts: a settings change made in one session is reflected in a second admin session without a manual reload (the `sync:js_tool_settings` refetch) — the sync-emit user-visible path.
- **TEST-53** (tier: e2e) [covers: ITEM-23, ITEM-26] file: `src-app/ui/tests/e2e/settings/js-tool-settings.spec.ts` — asserts: a non-admin user (lacking `js_tool::settings::read`) does not see the Programmatic Tools admin entry / the section renders its no-permission guard — the UI permission gate.

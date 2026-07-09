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

## Descoped from the gated enumeration (see DRIFT-2)
- **run_js real-LLM smoke** — file: `src-app/ui/tests/e2e/chat/run-js-real-llm.spec.ts` (retained, opt-in). A real tool-capable model choosing `run_js` and executing it end-to-end. Wired against the local LLM bridge and attempted, but the shared vLLM engine behind the bridge (`127.0.0.1:8000`) is OFFLINE in this environment (LiteLLM answers `/v1/models` from static config but 500s on completions: "Cannot connect to host 127.0.0.1:8000"), and it may not be restarted (shared GPU box — [[reference_local_llm_coder_ziee]]). It is therefore NOT a gated lifecycle test (an external-service dependency the gate environment can't satisfy — mirrors the repo's other `test.skip(!HAS_ANTHROPIC_KEY)` real-LLM specs, which are likewise ungated). The spec self-skips unless `OPENAI_BASE_URL` + `ZIEE_TEST_LLM_MODEL` are set, so it runs the moment a capable engine is available. **ITEM-5's provider-independence is fully covered without it**: the run_js attach seam is provider-agnostic by construction (unit TEST-20 attaches for ANY tool-capable model), and TEST-15 (integration) exercises a model-emitted `run_js` tool_use end-to-end through the REAL dispatcher + REAL rquickjs runtime (the stub chat model stands in for the LLM's tool-choice; the execution/economics path is identical for every provider), with TEST-35 proving the single-card render.

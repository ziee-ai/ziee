# PLAN — js-tool-scripting

Provider-agnostic **programmatic tool calling**: a new built-in tool
`run_js(script)` where ANY model writes JavaScript that runs in an **embedded
QuickJS interpreter inside the ziee server process**, with the conversation's
MCP tools injected as async host functions (`ziee.tools.web_search({query})`).
Intermediate sub-tool results stay inside the running script; only the script's
**final value** returns to the model's context — giving PTC token economics
(loop over 200 items, only the summary hits context) for every provider.

Architecture is pre-decided (see DECISIONS.md): NOT the OS code_sandbox (its
mac/windows backends cross a VM boundary, so a live host-function that re-enters
the in-process MCP dispatcher is impossible by construction); NOT a bearer token
in a sandbox (egress is open, tokens exfiltratable); an **embedded interpreter**
is cross-platform in-process everywhere, needs no credential (the injected host
function IS the capability), has no ambient fs/net/env, and its host-fn calls
land in the existing dispatcher chokepoint so per-call APPROVAL + `mcp_tool_calls`
RECORDING work — including suspending the script in-process while awaiting a user
approval (an async host fn that resolves on approve/deny).

## Items

### Backend — embedded runtime + host bridge
- **ITEM-1**: Add `rquickjs` `0.12.1` to `[workspace.dependencies]` + the server crate, features `["macro","futures"]`, keep the **default libc allocator** (do NOT enable `rust-alloc`/`allocator`, or `set_memory_limit` is a silent no-op) and do NOT enable the experimental `parallel` feature.
- **ITEM-2**: `js_tool::runtime` — an embedded-interpreter wrapper over `AsyncRuntime`/`AsyncContext` that evaluates the model script wrapped as `(async () => { <script> })()`, awaits the resulting Promise, and returns a structured outcome `{ value, console_lines, error{message,line} }`. Applies the caps: `set_memory_limit` (memory), `set_max_stack_size` (stack), an interrupt handler over a shared cancel-flag + gas counter (CPU/loop kill for `while(true){}`), and bounded `console.*` capture. Pure and unit-testable in isolation (no chat context, no host functions).
- **ITEM-3**: `js_tool::host_bridge` — injects `ziee.tools.<name>()`, `ziee.call(name,args)`, and `ziee.toolList()` into the JS global, one async host function per tool in the conversation's accessible set (unique friendly names; on collision → `<server>_<tool>`). Each host-fn call re-enters `McpSessionManager::get_or_create_with_context(server_id, …, source=Script)` → `execute_tool` with `sse_tx=None` so intermediate calls emit no tool cards; results are returned into the script and never leave it. Enforces the `max_tool_calls` cap (over-cap → the host fn throws).
- **ITEM-4**: `js_tool::approval` — per-call approval suspend/resume for a sub-tool that needs approval (SAME rules as the normal loop: `is_builtin_server_id` → bypass; control-mutating `invoke_capability` → approve; `ManualApprove` + not per-tool-allowlisted → approve). Registers a `tokio::sync::oneshot` in the reused `mcp::elicitation::registry` (vetted owner-binding, fail-closed), emits an approval-flavored SSE event on the live `sse_tx`, and `tokio::select!`s over {oneshot, `sse_tx.closed()`, 300 s timeout}. Approve → dispatch the sub-tool; deny/cancel/timeout → throw a catchable JS `ToolApprovalDenied` into the script.
- **ITEM-5**: `js_tool::executor` — the entry `mcp.rs` calls. Builds the tool set (reusing `validate_and_build_config` + `auto_attach_builtin_ids`), wires runtime + bridge + approval, runs the script under an **active-execution** wall-clock deadline (extended by approval-wait durations so a legitimate minutes-long approval never trips the CPU/wall guard), and assembles the final `McpContentData::ToolResult`.

### Backend — built-in registration + chat wiring
- **ITEM-6**: `run_js_mcp` built-in registration (`mod.rs`/`repository.rs`/`routes.rs`/`handlers.rs`/`tools.rs`) mirroring `memory_mcp/`: deterministic id `run_js_mcp_server_id() = Uuid::new_v5(NAMESPACE_URL, b"run_js.ziee.internal")`; idempotent boot upsert of a `mcp_servers` row (`is_built_in=true`, `is_system=true`, `transport_type='http'`, loopback `url=/api/run-js/mcp`); `tools/list` returns the `run_js(script)` descriptor; loopback `tools/call` returns a "must be invoked in chat context" error (execution is intercepted upstream, exactly like `ask_user`). Reuses `code_sandbox::types::{JsonRpcRequest,JsonRpcResponse,JsonRpcError,ConversationIdHeader}`.
- **ITEM-7**: `js_tool::chat_extension` (order 29, before the mcp collector at 30) — a `before_llm_call` that, when the model is tool-capable and `js_tool.enabled`, sets `metadata["attach_run_js_mcp"]="true"` and prepends a system nudge enumerating the available `ziee.tools.*` bindings + usage + limits. Exposes `pub const ATTACH_FLAG = "attach_run_js_mcp"`.
- **ITEM-8**: The two `mcp/chat_extension/mcp.rs` edits + the execution intercept: add the id to `auto_attach_builtin_ids` (so the model SEES `run_js`) and to `is_builtin_server_id` (so the model's `run_js` call auto-approves — the script START is not gated). In the tool-execution loop (mcp.rs ~2311), when `server_id == run_js_mcp_server_id()`, route to the `js_tool::executor` (passing `self.session_manager`, the already-assembled accessible servers, the conversation/user/branch/message ids, `sse_tx`, `elicit_notify_tx`) instead of the generic `execute_tool`.
- **ITEM-9**: `McpToolCallSource::Script` — add the variant to the enum in `mcp/tool_calls/models.rs` (+ `as_str => "script"`) so script-driven sub-tool calls record with `source=script`; add migration `00000000000133_mcp_tool_calls_source_script.sql` that `ALTER`s the `mcp_tool_calls_source_check` constraint to admit `'script'` (mirroring migration 108's widen for `'workflow'`).
- **ITEM-10**: Permission + deploy config: a `js_tool::use` permission (`permissions.rs`) granted to the default Users group via migration `00000000000134_grant_js_tool_permissions_to_users.sql` (mirror 098/104); a deploy-level `js_tool: Option<JsToolConfig>` in `core/config.rs` with `enabled` (default `true`, mirroring `web_search`/`lit_search`) as the kill switch; runtime caps as config-defaulted constants in `js_tool::limits`.
- **ITEM-11**: Result shape + persistence: the run_js result is `McpContentData::ToolResult { text: readable digest, structured_content: { result, console[], tool_calls[] trace, error? } }`, output-size-capped and `ziee://`-scrubbed. Only the final `result` reaches the model's context; the per-sub-tool trace is inspectable via `structuredContent`/`get_tool_result` + the `mcp_tool_calls` history. On script error the `error` carries message + line so the model can self-correct in one retry.
- **ITEM-12**: Register the `js_tool` module in `modules/mod.rs`; confirm desktop parity — js_tool is in-process + cross-platform by construction, so it is ENABLED on desktop (no `CORE_MODULE_BLOCKLIST` entry, no desktop-side disable), unlike server-only network features.

### Frontend (both `src-app/ui` and `src-app/desktop/ui`)
- **ITEM-13**: Inner-tool approval surface — handle the approval-flavored elicitation SSE (render-hint `tool_approval`) in `mcp/chat-extension/extension.tsx`; render an approve/deny prompt reusing `ToolCallPendingApprovalContent`'s visual via a thin `JsToolApprovalContent`; resolve through the EXISTING `POST /api/mcp/elicitation/{id}/respond` (`resolveElicitation`) — approve/deny/timeout, no new REST endpoint.
- **ITEM-14**: `script` source tone in `McpToolCallsTab` (both workspaces); gallery deep-states for the run_js call card + the inner-tool approval prompt (`dev/gallery/deepStates.tsx` + `dev/gallery/fixtures/chat-deep.ts`) so `check:state-matrix` / `check:gallery-coverage` pass.
- **ITEM-15**: OpenAPI regen for BOTH binaries (`just openapi-regen`) → `openapi.json` + `api-client/types.ts` in `ui` and `desktop/ui`, picking up the new `McpToolCallSource` `Script` value + any new elicitation render-hint field.

## Files to touch

### Backend — new
- `src-app/Cargo.toml` (workspace dep) + `src-app/server/Cargo.toml` (crate dep)
- `src-app/server/src/modules/js_tool/mod.rs`
- `src-app/server/src/modules/js_tool/runtime.rs`
- `src-app/server/src/modules/js_tool/host_bridge.rs`
- `src-app/server/src/modules/js_tool/approval.rs`
- `src-app/server/src/modules/js_tool/executor.rs`
- `src-app/server/src/modules/js_tool/limits.rs`
- `src-app/server/src/modules/js_tool/handlers.rs`
- `src-app/server/src/modules/js_tool/routes.rs`
- `src-app/server/src/modules/js_tool/tools.rs`
- `src-app/server/src/modules/js_tool/repository.rs`
- `src-app/server/src/modules/js_tool/permissions.rs`
- `src-app/server/src/modules/js_tool/chat_extension/mod.rs`
- `src-app/server/src/modules/js_tool/chat_extension/extension.rs`
- `src-app/server/src/modules/js_tool/chat_extension/js_tool.rs`
- `src-app/server/migrations/00000000000133_mcp_tool_calls_source_script.sql`
- `src-app/server/migrations/00000000000134_grant_js_tool_permissions_to_users.sql`
- `src-app/server/tests/js_tool/mod.rs`

### Backend — edited
- `src-app/server/src/modules/mod.rs` (register `js_tool`)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (2 edits + execution intercept + `#[cfg(test)]`)
- `src-app/server/src/modules/mcp/tool_calls/models.rs` (`McpToolCallSource::Script`)
- `src-app/server/src/core/config.rs` (`JsToolConfig`)
- `src-app/server/tests/integration_tests.rs` (mod include for `js_tool` tests)

### Frontend — new/edited (mirror in BOTH `src-app/ui` and `src-app/desktop/ui`)
- `src-app/ui/src/modules/mcp/chat-extension/extension.tsx` (approval SSE handler)
- `src-app/ui/src/modules/mcp/chat-extension/components/JsToolApprovalContent.tsx`
- `src-app/ui/src/modules/mcp/stores/McpComposer.store.ts` (resolve wiring, if not fully reused)
- `src-app/ui/src/modules/mcp/components/common/McpToolCallsTab.tsx` (`script` source tone)
- `src-app/ui/src/dev/gallery/deepStates.tsx` + `src-app/ui/src/dev/gallery/fixtures/chat-deep.ts`
- `src-app/ui/src/api-client/types.ts` + `src-app/ui/openapi/openapi.json` (regenerated)
- `src-app/desktop/ui/src/...` mirrors of every file above
- `src-app/ui/tests/e2e/chat/run-js-tool-scripting.spec.ts`
- `src-app/ui/tests/e2e/chat/run-js-inner-approval.spec.ts`

## Patterns to follow
- **Built-in MCP server (thin) + boot upsert + deterministic id** → closest: `src-app/server/src/modules/memory_mcp/` (`mod.rs`/`handlers.rs`/`routes.rs`/`tools.rs`/`repository.rs`). Reuse the JSON-RPC envelope + `ConversationIdHeader` from `code_sandbox::types` (do not re-declare).
- **Attach-flag chat extension + deploy kill switch** → `src-app/server/src/modules/web_search/chat_extension/` + `web_search`'s `core/config.rs` `WebSearchConfig`/`default_web_search_enabled` block.
- **In-process suspend/resume with a oneshot + side-channel resolve** (the load-bearing analog) → `mcp/chat_extension/helpers.rs::run_ask_user_elicitation` + `mcp/elicitation/registry.rs` + `mcp/elicitation/handlers.rs::respond_to_elicitation`. Do NOT use the `tool_use_approvals` turn-boundary flow — a live QuickJS call stack cannot survive ending the HTTP request.
- **Tool dispatch re-enter + recording** → `mcp/chat_extension/mcp.rs` execute path (`get_or_create_with_context` → `execute_tool`) and the chokepoint `mcp/client/session.rs::call_tool` (recording is automatic on a stamped session).
- **`source` enum + CHECK-constraint migration** → `mcp/tool_calls/models.rs` (`McpToolCallSource`) + migration `00000000000108_mcp_tool_calls_workflow_run.sql` (the exact ALTER shape to mirror).
- **Permission grant migration** → `00000000000098` / `00000000000104` (`DO $$` idempotent grant to the `Users` group).
- **Frontend approval reuse** → `modules/mcp/chat-extension/components/ToolCallPendingApprovalContent.tsx` (visual) + `ElicitationFormContent` + `McpComposer.store.ts::resolveElicitation` (side-channel resolve). Tool-call card is reused verbatim (`McpToolUseGroup`/`McpToolCallUI`/`McpToolUseRenderer`).
- **Gallery deep-state** → the existing `deep-chat-tool-approval` cell in `dev/gallery/deepStates.tsx` + fixtures in `dev/gallery/fixtures/chat-deep.ts`.
- **E2E** → `src-app/ui/tests/e2e/chat/mcp-tool-approval-optimistic.spec.ts` + `tests/e2e/helpers/sse-mock-helpers.ts` (mocked SSE two-call sequence); `mcp-tool-approval-real-llm.spec.ts` for the real-LLM variant.

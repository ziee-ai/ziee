# DECISIONS — js-tool-scripting

Every human/product input resolved up front so implementation runs nonstop. The
two the user is asked to explicitly acknowledge before implementation are **DEC-1
(runtime)** and **DEC-2/DEC-3 (approval-suspend semantics)**; both are resolved
below with a recommendation.

### DEC-1: Which embedded JS runtime?
**Resolution:** `rquickjs` `0.12.1` (QuickJS-NG), features `["macro","futures","parallel"]`, **default libc allocator** (do NOT enable `rust-alloc`/`allocator`, or `set_memory_limit` becomes a silent no-op). Single-threaded `AsyncRuntime`/`AsyncContext` driven on the request's Tokio task. Drop the `loader` feature (v1 evaluates one flat script; no `import`).

**Amendment (impl-wins, DRIFT-1.1):** `parallel` IS enabled — but ONLY for the `Send`/`Sync` marker impls it gates on `AsyncRuntime`/`AsyncContext` (rquickjs `context/async.rs`: `unsafe impl Send for AsyncContext` is `#[cfg(feature = "parallel")]`). Without it the runtime is `!Send`, and awaiting the evaluation inside the `tokio::spawn`ed, `#[async_trait]`-boxed-`Send` chat-turn task would not compile. We still drive ONE runtime from a SINGLE task and never touch it concurrently from multiple threads, so the experimental concurrent-access caveat does not apply — we only need the `Send` marker so the future may migrate between worker threads between awaits. This also required making the runtime's capture sinks `Send` (`Rc`→`Arc<Mutex>`/atomics).
**Basis:** convention — it is the only candidate meeting every hard requirement in-process: async host functions that await a Rust future and resume JS (`futures` + `Async(...)` → real Promises), an interrupt handler for CPU/loop kill, `set_memory_limit`/`set_max_stack_size`, zero ambient capabilities, and a bundled ~1 MB QuickJS-NG with pre-generated Linux/macOS/Windows bindings (no bindgen/clang; a C compiler is already a documented host dep for pgvector). deno_core/V8 drags in ~14 MB + a build-time V8 download + heavy version churn; boa lacks a memory cap and cannot kill a synchronous runaway loop; a bun/node subprocess has full ambient OS access (the repo's native spawn path never `env_clear()`s) — the exact thing the embedded design exists to avoid.

### DEC-2: How does the script suspend for a per-call approval, and how is it resolved?
**Resolution:** In-process, via the **reused `mcp::elicitation::registry` oneshot mechanism** — NOT the `tool_use_approvals` turn-boundary flow. The host fn registers a `tokio::sync::oneshot` in the existing registry with `content_id = None` (owner-bound, fail-closed), emits a new `runJsApprovalRequired{elicitation_id, server_name, tool_name, input}` SSE event on the live `sse_tx`, and `tokio::select!`s over {oneshot, `sse_tx.closed()`, 300 s timeout}. The user's approve/deny resolves through the EXISTING `POST /api/mcp/elicitation/{id}/respond` endpoint (it already tolerates `content_id = None`, so no server-side change to the resolver). The frontend renders an approve/deny card (`JsToolApprovalContent`, reusing `ToolCallPendingApprovalContent`'s visual) whose buttons call `resolveElicitation(id, 'accept'|'decline')`.
**Basis:** codebase — a live QuickJS call stack cannot be serialized/resumed across an HTTP-request boundary, so the `tool_use_approvals` mechanism (which ends the request and resumes by re-running generation on the next send) is structurally impossible here. `run_ask_user_elicitation` is the vetted in-process suspend/resume analog (global registry + oneshot + side-channel resolve + owner-binding), and its registry + respond endpoint are reusable as-is with `content_id = None`.

### DEC-3: What happens on deny / timeout / stop / multiple pending approvals?
**Resolution:** Approve (`accept`) → the host fn dispatches the sub-tool and returns its result into the script. Deny (`decline`) → the host fn throws a catchable JS `ToolApprovalDenied` error, so a script that `try/catch`es it can continue with a fallback; an uncaught throw ends the script with that error. Timeout (300 s, no response) and `sse_tx.closed()` (user hit Stop) → resolve as cancel → the host fn throws (same catchable error, distinct message). Multiple concurrent approvals are supported: each gated call registers its own random `elicitation_id`, so `Promise.all([gatedA(), gatedB()])` surfaces two independent prompts that resolve independently.
**Basis:** codebase — mirrors the three `tokio::select!` arms of `run_ask_user_elicitation` (oneshot / stream-closed / timeout), each mapping to a non-panicking outcome; the registry already keys by unique id so concurrency is free.

### DEC-4: Is `run_js` enabled by default, and for which conversations?
**Resolution:** A deploy-level `js_tool: { enabled: bool }` kill switch in `core/config.rs`, **default `true`**, mirroring `web_search`/`lit_search`/`bio_mcp`. When enabled, `run_js` auto-attaches to every **tool-capable** conversation (the chat extension sets `attach_run_js_mcp` only when the model is tool-capable). No per-user setting in v1.
**Basis:** convention + user intent — the feature's purpose is provider-agnostic PTC economics for every deployment; the interpreter has zero ambient capability and exposes only tools the model already has, and mutating sub-tools still require per-call approval, so default-on carries the same user-facing risk surface as the model's existing tool access. (Flagged to the user as a confirmable default; flip the `default_js_tool_enabled()` return to make it opt-in.)

### DEC-5: Is the model's `run_js` call itself gated, or auto-approved?
**Resolution:** The script START is **auto-approved** — add `run_js_mcp_server_id()` to `is_builtin_server_id` so the model's `run_js` tool_use executes without a manual-approval prompt. Per-call approval applies only to **gated sub-tools invoked inside** the script (DEC-2/DEC-3).
**Basis:** user — the brief explicitly directs "auto-approve the script START, per-call approval for gated tools INSIDE it."

### DEC-6: Built-in MCP server row, or inline intercept like `ask_user`?
**Resolution:** **Both (hybrid).** Register a built-in `mcp_servers` row (`run_js.ziee.internal`, loopback `/api/run-js/mcp`) so `run_js` appears in `tools/list`, auto-attaches, and the model sees it — but **intercept execution inline** in the mcp.rs tool-execution loop (route `server_id == run_js_mcp_server_id()` to the `js_tool` executor). The loopback `tools/call` handler returns an explicit "must be invoked in chat context" error and is never reached in normal flow.
**Basis:** codebase — `ask_user` uses exactly this shape (registered under `elicitation_mcp`, executed inline in `execute_tool`) because only the live chat-stream context holds the `sse_tx`/`elicit_notify_tx` needed to suspend. `run_js` needs even more live context (the `session_manager` + the assembled accessible-server list for host-fn dispatch), which lives one scope up in mcp.rs — so the intercept sits there, not in `execute_tool`.

### DEC-7: Which tools are injected as host functions?
**Resolution:** Exactly the conversation's accessible set — reuse `validate_and_build_config(pool, user_id, mcp_servers)` + `auto_attach_builtin_ids(metadata)`, the same list assembled into `request.tools` that the model sees. Auto-approved built-ins dispatch immediately; gated tools go through DEC-2. Excluded: `run_js` itself (no self-recursion) and `Always`-mode tools (already pre-run before the turn).
**Basis:** user + codebase — the brief specifies "exactly the conversation's attached/auto-attached tools (same set the model sees)"; that set is produced by the two named functions.

### DEC-8: How are tools named inside the script?
**Resolution:** `ziee.tools.<name>()` where `<name>` is the tool's own name when unique across attached servers; on collision `<server>_<tool>`. Also expose `ziee.call(name, args)` (dynamic) and `ziee.toolList()` (returns each binding's name + JSON input schema). The system nudge (DEC below) is GENERIC and directs the model to call `ziee.toolList()` for exact bindings at runtime.
**Basis:** convention — the wire tool name is the unfriendly `<server_id>__<tool>`; a friendly, collision-safe binding + runtime introspection keeps scripts readable without the nudge needing compile-time knowledge of the per-conversation tool set.

### DEC-9: What is the script's contract and final output?
**Resolution:** The script body is wrapped as `(async () => { <script> })()` and its resolved Promise value IS the final output (the model writes `return <summary>`). `console.log/warn/error` are captured separately (not part of the return value). A thrown error becomes the result's `error{message,line}`.
**Basis:** convention — an async IIFE gives the model a natural `await`/`return` model and a single unambiguous output value.

### DEC-10: Concrete limits.
**Resolution:** memory `128 MiB`; max stack `512 KiB`; CPU/loop guard = interrupt handler over a shared cancel-flag + gas counter that kills a pure-CPU spin within ~1–2 s of solid JS execution; `max_tool_calls = 100` per script (over-cap → the host fn throws); overall **active-execution** wall-clock backstop `300 s` (extended by approval-wait duration so a legitimate long approval never trips it); per-approval timeout `300 s`; final-output cap `128 KiB` (truncated with a marker); console capture cap `64 KiB`. All configurable via `JsToolConfig` with these defaults.
**Basis:** convention — mirrors `code_sandbox_settings` resource-limit shape, `ask_user`'s 300 s elicitation timeout, and the MCP result caps (1 MiB/16 KiB); the interrupt-vs-tokio-timeout split is the verified rquickjs pattern (interrupt bounds CPU only; wall-clock bounds active time; the awaited tool owns its own timeout).

### DEC-11: Result shape returned to the model.
**Resolution:** `McpContentData::ToolResult { text: &lt;human digest: the final value preview + a one-line run summary&gt;, structured_content: { result, console: [...], tool_calls: [{tool, status, duration_ms, ...}], error?: {message,line} } }`, output-capped (DEC-10) and `ziee://`-scrubbed. The `text` channel is what the model reads by default; `structured_content` is inspectable via `get_tool_result`. Only the final `result` reaches context; intermediate sub-tool results never become `tool_result` blocks.
**Basis:** codebase + user — matches the MCP tool-result + `structured_content` persistence contract and the brief's "final value + captured console + structured per-tool-call trace + errors with line numbers."

### DEC-12: Recording of the script's sub-tool calls.
**Resolution:** Add `McpToolCallSource::Script` (`as_str => "script"`); sub-tool calls route through `get_or_create_with_context(..., source=Script)` → `session.call_tool`, so they record automatically. Migration `00000000000133` ALTERs the `mcp_tool_calls_source_check` constraint to admit `'script'`.
**Basis:** codebase — migration `108` set the exact precedent (widening the same CHECK to add `'workflow'`); recording is the chokepoint's free behavior on a stamped session.

### DEC-13: Desktop enablement.
**Resolution:** `js_tool` is ENABLED on desktop — no `CORE_MODULE_BLOCKLIST` entry, no desktop-side disable. Desktop embeds the server, and the interpreter is in-process + cross-platform with no external dependency.
**Basis:** codebase — the desktop blocklist is for server-only features (network sidecars, host-only surfaces); an embedded interpreter is the opposite, and cross-platform-in-process is the feature's defining property.

### DEC-14: Migration numbers.
**Resolution:** `00000000000133_mcp_tool_calls_source_script.sql` and `00000000000134_grant_js_tool_permissions_to_users.sql`.
**Basis:** codebase — highest existing migration is `00000000000132`; no collision.

### DEC-15: Chat-extension order + nudge content.
**Resolution:** `js_tool` chat extension at `order = 29` (before the mcp collector at 30). The system nudge is GENERIC — it explains `run_js`, tells the model intermediate results stay in the script, and directs it to `ziee.toolList()` for exact bindings — so it needs no compile-time knowledge of the assembled tool set and the tie with `citations` (also 29) is benign.
**Basis:** codebase — the attach-flag extensions live at orders 26–29; ties among pre-mcp flag-setters are order-independent.

### DEC-16: Does `run_js` emit `ziee://` file artifacts (trusted-resource-emitter)?
**Resolution:** No for v1. `run_js` returns a value, not files, so `run_js_mcp_server_id()` is NOT added to `is_trusted_resource_emitter`. A sub-tool that itself returns a `resource_link` is handled by that sub-tool's own recording/persistence, unchanged.
**Basis:** scope — v1's output is a JSON value; file-producing scripts are a future extension, deliberately out of scope to keep the trusted-emitter surface minimal.

### DEC-17: Test-harness enablement.
**Resolution:** `js_tool` stays enabled (default-on) in the integration harness, matching production; the `js_tool` integration tests require it on. If an existing chat/integration test hard-asserts an exact tool list/count, update that assertion (a real signal, not a reason to gate the default).
**Basis:** codebase + memory — no cosmetic gating; the default should match production so tests exercise the real path.

### DEC-18: Provider-agnostic proof.
**Resolution:** Provider-independence is structural — `run_js` is injected into `request.tools` identically for every provider, so the backend + unit/integration tests (which are provider-agnostic by construction) prove the mechanism. The real-LLM e2e (TEST-36) drives whatever provider key/bridge is available and soft-skips otherwise.
**Basis:** codebase + memory — the tool-injection path is provider-independent; on this box `.env.test` provider keys are placeholders, and the LiteLLM bridge (master key `sk-local-audit`) is the available real-model path, so the real-LLM spec soft-skips cleanly.

### DEC-19: JS ↔ Rust value marshalling.
**Resolution:** Marshal across the boundary via QuickJS's built-in JSON: host-fn args → `ctx.json_stringify(value)` → `serde_json::from_str` → the dispatcher's `serde_json::Value`; the `ToolResult` text/`structured_content` → serialized JSON → `ctx.json_parse(...)` back into a JS value the script consumes. No extra rquickjs `serde` feature required.
**Basis:** convention — QuickJS ships JSON; a stringify/parse bridge is dependency-free and keeps the marshalling explicit and auditable.

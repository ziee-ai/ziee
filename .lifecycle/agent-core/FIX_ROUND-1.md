# FIX_ROUND-1 — blind-audit findings triage + fixes

Three blind sub-agents reviewed `git diff main...HEAD` (diff-only, no author
reasoning) across ≥12 angles (correctness, concurrency, error-handling,
resource-lifecycle, security, perms, secrets, modularity, maintainability, api,
simplification, dead-code). Findings recorded in `LEDGER.jsonl`.

## Fixed this round (all CONFIRMED high/medium security + correctness)

- **SEC-HIGH authz bypass** (`dispatch.rs` call_mcp_tool uuid-branch) — re-validate
  the model-supplied server_id against the user's accessible set (built-in enabled
  OR group-assigned), restoring the legacy accessible-set gate. Closes cross-group
  execution using another server's admin secret headers.
- **CORRECT-HIGH tool-error aborts turn** (`core.rs`) — a tool failure now yields
  an `is_error` tool_result and the loop continues (no orphan tool_use).
- **CORRECT-HIGH resume double-execute** (`core.rs`) — `resume_executes_pending`
  gates the loop's native resume; chat sets it `false` (its RegistryBridge's MCP
  `before_llm_call` already executes approved tools on resume), so the tool runs
  once, not twice.
- **CORRECT-HIGH partial-suspend orphan** (`core.rs`) — the loop processes the whole
  round before finalizing on suspend, so every approval-needing tool gets a pending
  row and none is orphaned.
- **SEC-MED denial fail-open** (`gate.rs`) — the denial read now fails CLOSED
  (Prompt on DB error, never a silent Auto of a denied tool).
- **SEC-MED approval-claim race** (`gate.rs`) — Auto only when the single-use
  `delete_tool_approval` claim returns `Ok(true)`.
- **SEC-MED is_trusted over-trust** (`resolver.rs`) — the name fallback no longer
  trusts `code_sandbox`/`control_mcp`.
- Reverted the `ZIEE_CHAT_AGENT_CORE` default flip → **HELD at opt-in** pending the
  above (per the security agent: finding #1 "should block the cutover"). Plus dead
  `is_resume` removed.

**Verification:** `agent-core` 36/36; `cargo check -p ziee` green; and the
deterministic happy-path suites re-run GREEN on the opt-in path after the fixes —
`chat::chat_stream_test` 6/6, `chat::streaming_test` 10/10, the real-bridge
`agent_core_tool_bridge_test` 1/1 — so the fixes introduced no regression.

## Deferred (CONFIRMED, lower-severity design/quality — do NOT gate correctness/security)

Tracked as follow-ups (they are maintainability/api/dead-code, not behavior bugs),
each with a concrete fix in the LEDGER:
- MOD-HIGH: move `call_mcp_tool` + `McpCallScope`/`CancelSignal` out of
  `workflow::dispatch` into a shared `mcp/tool_call.rs`; both hosts import it there.
- MAINT-HIGH/MED: de-duplicate `split_tool_name`/`mcp_to_agent_result` + the
  4-site `__`-split into one `parse_namespaced_tool` in the shared MCP module.
- API-MED: a `ChatAgentTurn::new(provider_bundle, …)` + a `McpCallOptions` struct
  (retire the 9-positional-param / boolean-gate soup).
- CORRECT-MED: add `StopReason::Suspended` so a gated turn isn't reported as
  `"cancelled"`.
- LIFECYCLE-MED: a RAII terminal/slot guard on the spawned generation task (mirror
  the legacy `TerminalGuard`) so a panic can't latch `begin_generation`.
- DEAD-CODE: wire or delete `on_delta`, the per-step token cap, `ToolNotification`,
  and the `inputs`/`TurnContext.inputs` seam (unused by both hosts today).

**New confirmed findings:** 0

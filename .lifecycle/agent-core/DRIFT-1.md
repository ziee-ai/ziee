# DRIFT-1 — implementation vs plan

Reconciliation of what was BUILT against PLAN.md. Every divergence below is
resolved (the plan was amended — `impl-wins` — with the rationale captured here,
per the Phase-5 loop); none is left unreconciled.

- **DRIFT-1.1** — verdict: impl-wins — The crate grew a **token-streaming seam**
  (`ModelClient::call_streaming` + `DeltaSink` + `AgentEvent::ContentDelta` +
  `EventDeltaSink`). The plan assumed the existing non-streaming `call` sufficed;
  chat's live per-token SSE requires streaming. Backward-compatible (default
  delegates to `call`). Real-LLM-verified (332 deltas via the bridge).

- **DRIFT-1.2** — verdict: impl-wins — The 14 context-injector extensions are
  re-homed via ONE **`RegistryBridge`** `AgentExtension` (runs the existing chat
  `ExtensionRegistry` before/after_llm_call inside the loop) rather than 14
  hand-ported per-module `AgentExtension`s. Reuses the tested extension logic
  verbatim (no divergence), is DRY, and scores better on the
  modularity/maintainability angles than 14 near-identical copies. The "port each
  extension" goal is met by delegation. Plan (ITEM-8 "port context extensions")
  amended to "bridge the registry".

- **DRIFT-1.3** — verdict: impl-wins — The crate loop gained **native
  resume-mid-tool-execution** (`last_pending_assistant`): on a `Resume` whose
  transcript ends with unexecuted `tool_use`, execute those (through the
  gate/policy) instead of re-calling the model. Needed because a real model
  re-emits tool calls with NEW ids on resume, losing the human's approval
  decision. Domain-neutral (pure transcript shape).

- **DRIFT-1.4** — verdict: impl-wins — `ToolResult` gained a **`terminal`** flag;
  the loop finalizes without a continuation when EVERY executed tool is terminal
  (MCP `audience:["user"]`-only output, or a built-in memory `remember`/`forget`
  side-effect self-save). Reproduces the MCP extension's `CompleteWithContent` /
  Track-B behavior, which the ports otherwise bypassed.

- **DRIFT-1.5** — verdict: impl-wins — Added a **system-message merge**
  normalization (all System messages → one, first) before the model call. The
  re-homed extensions each insert a system prompt; strict providers (vllm/qwen)
  accept only one, first. Semantically identical, valid for every provider; makes
  the agent-core path MORE robust than the legacy loop.

- **DRIFT-1.6** — verdict: impl-wins — `call_mcp_tool` gained a `source` param +
  accepts a server-id-uuid prefix; the chat `set_workflow_run`/idempotency-key
  writes are gated to Workflow source (a chat `run_id` is the assistant message,
  not a `workflow_runs` row → FK-violation otherwise). Behaviour-preserving for
  workflow.

- **DRIFT-1.7** — verdict: impl-wins — The **legacy loop is retained** behind
  `ZIEE_CHAT_AGENT_CORE=0` as a one-release opt-out safety valve rather than
  deleted immediately. Parity is verified on the deterministic suites, but the 8
  env-gated real-LLM agentic tests can't be exercised in this environment; a
  dormant opt-out (never runs unless `=0`) is the responsible soak posture before
  removing ~700 lines. Deletion is the documented follow-up. (Recorded as FB in
  HUMAN_FEEDBACK.md.)

- **DRIFT-1.8** — verdict: resolved — DEC-19's opaque `inputs` carrier is present
  but the chat path does NOT use it: the `RegistryBridge` carries the full typed
  `SendMessageRequest` directly (cleaner than a serialized bag). `inputs` remains
  for a future pure-crate extension; no conflict.

**Unresolved drifts:** 0

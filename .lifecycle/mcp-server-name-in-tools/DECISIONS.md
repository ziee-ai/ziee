# DECISIONS

### DEC-1: Which server identifier tags tools and heads roster lines?
**Resolution:** `server.name` (e.g. `biognosia`) in BOTH the per-tool `[…]` prefix and the roster line.
**Basis:** user — khoi's plan refinement (users refer to servers as biognosia/rcpa/dscc; `name` is shorter/token-cheaper than `display_name`; using the same identifier in both places lets the model link a tagged tool to its roster entry).

### DEC-2: Source of the per-server blurb?
**Resolution:** the existing `mcp_servers.description` column. Do NOT capture MCP `initialize.instructions`.
**Basis:** user — khoi's refinement (keeps the client path untouched and moots prompt-injection: no untrusted server-provided text ever enters the system prompt).

### DEC-3: Which servers are included — filter field?
**Resolution:** `is_built_in = false` (external servers only). Explicitly NOT `is_system`.
**Basis:** user — khoi's refinement (biognosia is `is_system = true` but `is_built_in = false`; an `is_system` filter would wrongly drop it). Applies to BOTH the per-tool label and the roster section.

### DEC-4: Per-tool label format?
**Resolution:** `"[<name>] <original tool description>"` (single space after `]`).
**Basis:** convention — matches the plan's approved token-cheap format; label lives only in the description, never the wire name.

### DEC-5: Are always-mode external servers in the roster?
**Resolution:** No — the roster lists only auto-mode external servers (those whose tools are advertised with `[name]` prefixes). Always-mode servers `continue` before adding tools; their pre-fetched context is already injected into the user turn.
**Basis:** codebase — keeps the `[name]` prefixes and the roster in exact correspondence (`mcp.rs:1853-1957` always-mode branch does not advertise tools). Noted in STATUS as a scoping choice.

### DEC-6: What does "(N tools)" count?
**Resolution:** the number of that server's tools actually ADVERTISED to the model (pushed into `all_tools` after `convert_mcp_tool_to_ai_tool` drops any name-guard failures) — the honest count the model sees.
**Basis:** convention — matches what the model is shown; avoids over-counting tools that were dropped by the charset/length guard.

### DEC-7: How is an external server with an empty description rendered?
**Resolution:** `- <name> (N tools)` (name + count only; no `— ` blurb).
**Basis:** user — khoi's refinement.

### DEC-8: Configurable-settings rule — is anything an admin tunable, or fixed behavior?
**Resolution:** FIXED, always-on behavior. No settings row, no admin toggle, no new permission.
**Basis:** convention — the feature introduces NO operational tunable (no resource limit, retention, rate/quota, threshold, or model/provider selection). It is deterministic system-prompt assembly with negligible token cost, mirroring the always-on `tool_system_guidance` (`mcp.rs:83-108`), which likewise has no toggle. Promoting a future "disable server roster" toggle would be a separate, additive change; nothing here is footgun-shaped.

### DEC-9: Where is the roster section placed in the prompt?
**Resolution:** appended to the SAME iteration-1 System message that receives `tool_system_guidance` (`mcp.rs:2149-2162`); a new System message at index 0 only if none exists — mirroring the existing block. One append per turn.
**Basis:** convention — reuses the established system-note idiom; keeps a single cacheable system prefix.

### DEC-10: How does the integration test observe the per-tool label on the wire?
**Resolution:** (revised — see DRIFT-1) use the existing `common::oai_capture_stub::StubChat`, which already captures the **verbatim** `/v1/chat/completions` body (so `tools[].function.description` and the system `messages` are directly inspectable) AND scripts tool calls (`StubPlan`/`StubToolCall`). **No harness change needed** — the originally-planned `RecordedRequest` extension to `common::stub_chat.rs` is NOT made (strictly smaller diff, and `common::stub_chat` is untouched).
**Basis:** codebase — `mcp_approval_loop_test.rs` already drives MockMcpServer + oai_capture_stub this way; capturing the raw body is exactly what proves the label reached the provider by RUNNING the real advertise path (B7).

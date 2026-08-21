# INFRA_INTEGRATION — mandatory phase-5 walks

## 1. User-experience walk

**How a real user meets this defect.** The user asks the model something that
needs a tool whose result happens to contain `data: ` — on the live rig,
"list my citations", where one of 17 entries is titled *"Mobilizing the base of
neuroscience data: the case of neuronal morphologies"*. The model issues
`list_citations`; the server answers correctly (HTTP 200, `Content-Type:
application/json`, a valid 193,956-byte JSON-RPC result); the client misroutes
it and produces `AppError::internal_error("No data found in SSE response")`.

The failure is **silent from the user's side**: the activity rail shows the tool
step, the step completes, and no content comes back. The model then answers from
nothing — reporting an empty library, or filling the gap. Nothing in the UI says
"the client failed to parse a valid response", so the user has no way to
attribute it, and re-running is deterministic (the same citation is still
there), so it reads as "the feature is broken" rather than "a transient error".

**Blast radius is content-shaped, not feature-shaped.** It is not a citations
bug. Any tool result containing the literal `data: ` fails identically — a log
excerpt, a code snippet, a YAML fragment, a chat message quoted back, a paper
title. The live-UI audit measured **48 occurrences across a 372-result
calibration corpus (~13%)**. That is the real severity: a common substring in
ordinary English and in machine output silently voids a tool call.

**After the fix** the content returns verbatim and the model answers from it.

## 2. Infrastructure-integration walk

Everything downstream consumes the `ToolResult` this function produces, so the
defect truncates all of it and the fix restores all of it. Each was checked for
behaviour that must be handled rather than assumed:

| subsystem | interaction | finding |
|---|---|---|
| **MCP tool-call history** (`mcp/tool_calls/record.rs`) | Recording happens in `McpSession::call_tool`, which wraps this client call. A parse failure is recorded with terminal status `failed`. | No change needed. The fix converts a spurious `failed` row into a `completed` one; the recorder is agnostic. Confirms the defect was *observable* in history all along — the rows exist, attributed to the wrong cause. |
| **Chat tool_result blocks** (`mcp/chat_extension/mcp.rs`) | The `ToolResult` becomes a persisted `tool_result` content block, with `structured_content` capped at 1 MB. | No change needed. The fix restores content that was being dropped before it reached the save site; caps are applied downstream and are unaffected. |
| **`structured_content` + `tool_result_mcp`** | `get_tool_result` pages a prior result, including `structuredContent`. | No change needed, but relevant: a misrouted call persisted NO result, so `get_tool_result` had nothing to recall — the defect propagated into the model's later recall path too. |
| **`resource_link` persistence** (`mcp/resource_link.rs`) | Operates on `ToolResult` content blocks. | No change needed. Links inside a misrouted result were never seen; the fix restores them to the existing, unchanged persistence path. |
| **Approval / elicitation flow** | Elicitation-capable servers answer `tools/call` with `text/event-stream` and route to Branch 2, untouched by ITEM-1. ITEM-3 does touch Branch 2's payload extraction. | Verified by TEST-7/8/9 driving the real elicitation path. ITEM-3 only *widens* what that extractor accepts (no-space `data:`, multi-line blocks); no previously-parsing stream stops parsing. |
| **Sampling path** (`call_tool_with_sampling`) | Shares the same hand-rolled extraction (N1). | ITEM-3 changes it identically to N2. Not covered by a direct integration test (it needs a sampling handler); it is the *same two-line delegation* to the same helper as N2, which TEST-8/9 do cover. Recorded as a known coverage boundary rather than claimed as tested. |
| **Built-in MCP servers** (citations, memory, files, web_search, lit_search, knowledge_base, bio, workflow, background, control) | All are loopback JSON-RPC and answer `tools/call` with `application/json`. | **All of them route through the defective Branch 3.** This is why the impact is broad: every built-in tool shared the defect, and any of their results containing `data: ` failed. |
| **Permissions / sync / migrations** | None touched. | No permission introduced (A9/A10 N/A), no migration (no collision), no sync entity, no OpenAPI surface. |

## 3. Entity-lifecycle walk

This change introduces no entity and no surface that holds one. The only object
whose lifecycle it affects is the transient `ToolResult` produced per tool call:

- **created** — by the parse this change fixes; previously, for `data: `-bearing
  content, it was never created at all (an `Err` was produced instead).
- **consumed** — synchronously by the chat MCP extension, which persists a
  `tool_result` block and records an `mcp_tool_calls` row. Both paths already
  handle the `Err` case (that is what shipped), so restoring the `Ok` case adds
  no new state transition.
- **removed / access-loss** — governed by existing retention
  (`mcp_user_policy.tool_call_retention_days`) and conversation deletion cascade.
  Unchanged.

There is no local-vs-sync duality to check here: the result is not a
cross-device entity and emits no `sync:` event of its own.

# TESTS — enumerated up front

Backend-only diff (no `src-app/ui/**`), no new permission → no `tier: e2e` and no
`[negative-perm]` spec required by the gate.

The `StubChat` stub LLM (`tests/common/stub_chat.rs`) records the outgoing
`ChatRequest`. It captures `tool_names` + `all_text` (all message text incl. system)
today; TEST-4 additionally needs per-tool descriptions, so `RecordedRequest` gains an
**additive** `tool_descriptions` field (+ a `description_for(bare)` helper) — a
general recording capability, not a feature-specific harness workaround.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — asserts: `convert_mcp_tool_to_ai_tool(id, tool, Some("biognosia"))` yields description `"[biognosia] <orig>"`; with `None` the description is byte-identical to the original; the composed wire NAME is exactly `<uuid>__<tool>` in BOTH cases (label never touches the name).
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — asserts: a tool with empty/None description + `Some("rcpa")` → description `"[rcpa] "` (label present, no orig text); an oversize composed name and a bad-charset tool name still return `None` regardless of the label (the guards check the name, not the description).
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `connected_servers_section` renders `- <name> — <desc> (N tools)` for a described server; `- <name> (N tools)` when description is empty/None; multiple servers appear as separate lines under one `## Connected MCP servers` heading; an empty input returns `None` (no section).
- **TEST-4** (tier: integration) [covers: ITEM-2, ITEM-4] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: with a tool-capable model + an external MockMcpServer (description set, one tool) attached, the captured request shows the external tool's description prefixed `[<server.name>] …`, the always-on built-in tool (`ask_user`/`get_tool_result`) description is UNprefixed, and the system text contains `## Connected MCP servers` listing the external server name but NOT any built-in server.
- **TEST-5** (tier: integration) [covers: ITEM-1, ITEM-4] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: the external mock tool still round-trips dispatch — the stub emits the `<uuid>__<tool>` call it saw, the server routes it to the mock, and the tool result flows back — proving the description label did not disturb the wire-name dispatch path.

## Negative-control plan (per ziee-negative-control-your-tests)

For each load-bearing test, before declaring done: revert the specific change and
confirm the test fails with the intended diagnostic, then restore.
- TEST-1/2 ← remove the `[label] ` prepend → prefix assertion goes red.
- TEST-3 ← force `connected_servers_section` to `None` → section assertions go red.
- TEST-4 ← remove the built-in gate (label built-ins too, or include built-ins in
  the roster) → the "built-in UNprefixed / absent from roster" legs go red.
- TEST-5 ← (control) with the label applied, dispatch must still succeed; a broken
  name path would fail this regardless.

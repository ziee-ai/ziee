# INFRA_INTEGRATION — walks per item

## User-experience walk

A user connects an external MCP server (e.g. biognosia) with a description, opens a
tool-capable chat, and asks "what tools do I have / what is biognosia?". Today the
model sees `<uuid>__search_bio` with a bare tool description and cannot name the
server. After this change: each biognosia tool's description reads `[biognosia] …`,
and the system prompt carries `## Connected MCP servers\n- biognosia — <desc> (7 tools)`,
so the model answers "biognosia is …" and correctly attributes each tool. Built-in
tools (read_file, ask_user, web_search) are unchanged — no `[…]` noise, not listed —
so the user's own servers stand out. No new UI, no new setting, no new permission.

## Infrastructure-integration walk

- **Chat pipeline / system-prompt assembly** — the roster is appended to the SAME
  iteration-1 System message as `tool_system_guidance` (DEC-9), so it rides the
  existing cacheable system prefix; not re-emitted on tool-loop continuation turns
  (iteration > 1). No ordering change vs assistant/project/file extensions.
- **MCP tool-call + dispatch** — the wire tool NAME is unchanged, so
  `resolve_server_and_tool`, the bare-name recovery map, approval routing, and
  `send_tool_start_event` are all unaffected. Only `description` text changes.
- **Anthropic name guard** — the 128-char/charset checks operate on the composed
  NAME; the label is added to the description only, so no tool that passes today
  starts failing (and no dropped tool starts passing).
- **Always-mode servers** — advertise no tools (pre-run + inject context), so they
  are excluded from the roster (DEC-5) to keep prefixes ↔ roster in correspondence.
- **Built-in servers** — `is_built_in = true` → no label, not in the roster (DEC-3).
  The always-on `ask_user`/`get_tool_result` built-ins are the in-request control the
  integration test uses to prove the gate.
- **Prompt cost** — one roster line per external server + a short `[name] ` per
  external tool; server descriptions emitted once (roster), not per tool. Negligible.
- **Sync / notifications / streaming / workflow runner** — untouched (no entity,
  no event, no persisted state). No settings row (DEC-8).

## Entity-lifecycle walk

No new entity, no cache, no persisted state, no sync surface — the change is pure
per-request prompt assembly derived from the already-fetched `McpServer` rows and the
live `list_tools()` result. A server disabled/removed mid-session simply drops out of
`server_configs` on the next request (existing behavior), so its `[name]` prefixes and
roster line vanish together on the next turn. Nothing to add/remove/mutate beyond what
the existing accessibility scoping already handles.

# PLAN — Surface the (external) MCP server name + description to the LLM

## Problem

The chat model can't tell which named MCP server a tool belongs to, and can't
answer "what is `rcpa`/`dscc`/`biognosia`". `convert_mcp_tool_to_ai_tool`
(`mcp/chat_extension/helpers.rs:109`) advertises each tool as name
`{server_uuid}__{tool}` + description = the tool's own text only; the server's
human `name`/`description` is never threaded in, and there is no server-level
roster in the system prompt.

## Approach

Two-part, backend-only, scoped to **external** servers (`is_built_in = false`):
1. Prefix each external tool's *description* with `[<server.name>] ` (wire NAME
   unchanged — dispatch + Anthropic charset/128-char guards untouched).
2. Append a once-per-turn "## Connected MCP servers" section to the iteration-1
   system message, listing each external server as `- <name> — <description> (N tools)`.
Blurb source = the existing `mcp_servers.description` column (no
`initialize.instructions` capture → no client-path changes, no untrusted text in
the prompt). Same `server.name` identifier in both places so the model links a
tagged tool to its roster entry.

## Items

- **ITEM-1**: Add a `server_label: Option<&str>` parameter to
  `convert_mcp_tool_to_ai_tool` (`helpers.rs`). When `Some(label)`, prepend
  `"[<label>] "` to the tool description (label applied to the description only,
  never the wire name). When `None`, description is unchanged. Charset/128-char
  guards continue to check the composed NAME (unaffected by the label).
- **ITEM-2**: At both call sites in `mcp.rs` (auto-mode `~:2041`, static `ask_user`
  descriptor `~:1974`) pass `(!server.is_built_in).then(|| server.name.as_str())`,
  so built-in tools stay unlabeled and only external tools get the `[name]` prefix.
  Update the 3 existing `helpers.rs` unit-test call sites to pass `None`.
- **ITEM-3**: Add a pure helper `connected_servers_section(servers: &[(&str, Option<&str>, usize)]) -> Option<String>`
  in `mcp.rs` that renders the "## Connected MCP servers" markdown from
  `(name, description, advertised_tool_count)` tuples; `None`/empty description →
  `- <name> (N tools)`; returns `None` when the slice is empty.
- **ITEM-4**: In the advertising loop, accumulate the external-server roster
  (auto-mode servers only — the ones whose tools are advertised; each server's
  entry counts the tools actually pushed to `all_tools`), then on
  `context.iteration == 1` append `connected_servers_section(...)` to the same
  system message that receives `tool_system_guidance`. No external servers → no
  section added.

## Files to touch

- `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — ITEM-1 (+ its unit tests)
- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — ITEM-2, ITEM-3, ITEM-4 (+ unit test for the section builder)
- `src-app/server/tests/mcp/mcp_extension_test.rs` — integration coverage (external+built-in mock)

## Patterns to follow

- **System-note append idiom** — the existing iteration-1 block at `mcp.rs:2149-2162`
  (`tool_system_guidance`): keep the roster section in that same block, appended to
  the first System message (or a new one at index 0). Closest sibling.
- **Pure, unit-testable string helper** — `connected_servers_section()` mirrors
  `tool_system_guidance()` (`mcp.rs:83-108`): takes data, returns a `String`, so it
  is directly `#[cfg(test)]`-testable with no I/O.
- **Optional-param convert** — mirror the existing `convert_mcp_tool_to_ai_tool`
  shape; only add a param, keep the `Option<ai_providers::Tool>` return + guard flow.
- **Mock-driven integration** — `tests/mcp/mcp_extension_test.rs` + the existing
  `tests/mcp/fixtures/mock_mcp_server.rs` `MockMcpServer`.

## Notes (not scope — STATUS only)

- Cross-server tool-name collisions and very-long server names are pre-existing and
  out of scope; note in STATUS.
- Always-mode external servers (tools pre-run, not advertised) are excluded from the
  roster so the `[name]` prefixes and the roster correspond exactly — see DEC.

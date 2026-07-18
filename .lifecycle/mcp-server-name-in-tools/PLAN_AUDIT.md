# PLAN_AUDIT — audited against the codebase

## Breakage risk

- `convert_mcp_tool_to_ai_tool` has exactly **3 real callers**: `mcp.rs:1974`,
  `mcp.rs:2041`, and its own `#[cfg(test)]` block (3 calls at helpers.rs:1101/1113/1124).
  The `tests/common/stub_chat.rs` hit is a **doc comment only** (line 46), not a call.
  Adding an `Option<&str>` param is a compile-forced update at all 3 real callers —
  no silent-miss risk. No external crate imports this fn (it is `pub` within the
  server crate; grep shows no cross-crate use).
- Wire tool **name is unchanged** → the return-path dispatch (`resolve_server_and_tool`,
  `mcp.rs:295-314`), the bare-name recovery map (`mcp.rs:2078-2122`), the
  `send_tool_start_event` server label, and the charset/128-char guards are all
  unaffected. The label is added to `description` only.
- The roster section is appended to the SAME iteration-1 System message that already
  receives `tool_system_guidance`; it does not create a competing System block or
  alter message ordering. Prompt-cache impact is a small, stable, cacheable prefix
  addition (same slot as the existing nudge), not per-turn volatile text.
- `server.description` is `Option<String>` already on the in-scope `McpServer` row —
  no new fetch, no nullability surprise. `is_built_in` is a plain bool on the row.

## Pattern conformance

- ITEM-1 keeps `convert_mcp_tool_to_ai_tool`'s existing `Option<Tool>` return + guard
  flow; only prepends to the description string. Conforms to the fn's own shape.
- ITEM-3's `connected_servers_section()` mirrors `tool_system_guidance()` (pure
  `&[data] -> String/Option<String>`), the established idiom for prompt fragments.
- ITEM-4 reuses the existing iteration-1 append block (mcp.rs:2149-2162) rather than
  introducing a second System-message insertion path.
- Integration test mirrors `tests/mcp/mcp_extension_test.rs` + `MockMcpServer`.

## Migration collisions

None. This feature adds no migration (BASE.md: highest is 158; we add 0).

## OpenAPI regen

Not required. No serialized-type change (`McpServer.description` is already in the
schema; the label + roster are runtime prompt assembly). No `openapi.json` /
`api-client/types.ts` delta → backend-only diff; frontend gates N/A.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — additive `Option<&str>` param; 3 callers compile-forced; guards check the NAME not the description, so prefix is safe.
- **ITEM-2** — verdict: PASS — `server.is_built_in` + `server.name` both in scope at both call sites (mcp.rs:1848-1851 resolves `server`); `(!is_built_in).then(...)` is the gating idiom.
- **ITEM-3** — verdict: PASS — pure string helper, mirrors `tool_system_guidance`; no I/O, directly unit-testable.
- **ITEM-4** — verdict: PASS — appends to the existing iteration-1 System message; roster scoped to advertised (auto-mode) external servers so prefixes ↔ roster correspond. Always-mode exclusion recorded as DEC.
- **ITEM-5** — verdict: PASS — pure `sanitize_prompt_field(&str, cap) -> String` (collapse control/whitespace + cap); no I/O, applied once per server for the label and at roster build; defends the system-prompt section structure. No behavior change for well-formed names/descriptions.

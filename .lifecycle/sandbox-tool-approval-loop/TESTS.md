# TESTS — sandbox-tool-approval-loop

Backend-only diff (MCP chat extension + justfile). No frontend path touched → no `tier: e2e`
required. All new integration tests share the `mcp_approval_loop_` name prefix so ITEM-4's
`check-mcp-approval` gate selects them with one substring filter.

## Unit (in `mcp.rs` `#[cfg(test)] mod tests`, run via `cargo test --lib mcp::chat_extension::`)

- **TEST-1** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `recover_server_id_for_bare_name("execute_command", &map)` returns `Some(server_id)` when the map has exactly one server for that bare name.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: an ambiguous bare name (mapped to the `None` sentinel because two servers advertise it) returns `None` — the recovery refuses to guess.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: an unknown bare name (absent from the map) returns `None`.
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `resolve_unique_tool_use_id("", &used)` mints a fresh non-empty `call_<uuid>` id.
- **TEST-5** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `resolve_unique_tool_use_id("tool_use", &used)` mints a fresh id when `used` already contains `"tool_use"` (covers within-batch AND cross-iteration collision, which share the `used`-membership mechanism), and the minted id is not in `used`.
- **TEST-6** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: a unique provider id (e.g. `toolu_abc`) not in `used` is returned unchanged (good ids preserved).

## Integration (`tests/mcp/mcp_approval_loop_test.rs`, run via `cargo test --test integration_tests -- --test-threads=1 mcp_approval_loop_`)

Driven end-to-end through the real chat path: a scriptable OpenAI stub
(`common::oai_capture_stub`) emitting a BARE tool name + an in-process HTTP MCP mock
(`fixtures::mock_mcp_server`) under manual-approve. The decisive anti-loop assertion is
`StubChat::request_count()` (buggy path ≈ 10 LLM calls; fixed ≈ 2).

- **TEST-10** (tier: integration) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/server/tests/mcp/mcp_approval_loop_test.rs` — asserts: (`mcp_approval_loop_bare_name_recovers_and_executes`) a tool call emitted with a **bare** name (no `<uuid>__` prefix) is recovered to the advertising server (the approval row's `server_id == mock id`, not NULL), gets a non-empty unique id, and after approval the tool **actually executes** (the mock receives a `tools/call`); the turn does not hit `max_iteration` and the LLM is not re-called in a loop.
- **TEST-8** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/mcp/mcp_approval_loop_test.rs` — asserts: (`mcp_approval_loop_unresolvable_tool_errors_and_terminates`) an unresolvable bare name (advertised by no server) yields a NULL-server_id approval; after approval it surfaces a clear error AND the approval row is **deleted** (not re-looped), and the turn does not hit `max_iteration`.
- **TEST-11** (tier: integration) [covers: ITEM-4] file: `justfile` — asserts: `just check-mcp-approval` exists, its `mcp_approval_loop_` filter selects the new integration tests + the `mcp::chat_extension::` unit tests, and the recipe exits 0 (green).

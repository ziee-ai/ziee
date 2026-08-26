# PLAN — mcp-toolcollect-timeout

Backend hardening fix: a single misconfigured/hanging MCP server must not stall
a chat send. The MCP chat-extension `before_llm_call` collects tools from each
attached server before the LLM call; there is NO timeout, and the stdio
`initialize` handshake `().serve(transport).await` blocks forever when a server
never speaks MCP (e.g. `command=npx, args=[]`). One hanging server therefore
stalls the whole turn — the LLM is never called and the user gets an EMPTY
assistant message with no error.

## Design source

Realizes `agent-kit/docs/CODING_GUIDELINES.md` §2 (outbound HTTP & SSRF — "Always
set `.connect_timeout()` + `.timeout()` on every external client"), §5 (resource
lifecycle & cleanup — a repeatedly-hanging child must trip the breaker, not be
re-dialed every turn), and §6 (error handling — no silent swallow; distinguish
failure paths; a timeout is a surfaced-and-skipped failure, never a hang). There
is no separate design doc; these guideline sections ARE the non-negotiables, plus
the diagnosis recorded in this file's header.

## Invariants

- **INV-1**: every outbound MCP connect/handshake await is time-bounded — no
  unbounded `serve()`/connect await (CODING_GUIDELINES.md §2: "Always set
  `.connect_timeout()` + `.timeout()` on every external client").
- **INV-2**: a failed OR timed-out MCP server during tool-collection is tolerated
  (warn + skip) and NEVER aborts the send — the LLM is still called with the
  reachable servers' tools (CODING_GUIDELINES.md §6: no silent swallow, distinguish
  failure paths; and the bug itself).
- **INV-3**: a connect timeout is recorded as a connection failure so the
  circuit-breaker opens on a repeatedly-hanging server (CODING_GUIDELINES.md §5:
  resource lifecycle; reuse `manager.rs` breaker).

## Items

- **ITEM-1**: Outer/auto-mode — in `mcp.rs::before_llm_call`, wrap the auto-mode
  `session_manager.get_or_create_with_context(...).await` AND `session.list_tools().await`
  in `tokio::time::timeout`. On elapsed, route into the EXISTING `warn!(… "— skipping"); continue;`
  path (indistinguishable from the current connect/list `Err` arms) so the loop
  keeps collecting the other servers and still builds the LLM request.
- **ITEM-2**: Outer/always-mode — in `mcp.rs::before_llm_call`, wrap the always-mode
  session build (`resolve_server_for_session` + `McpSession::new*`), `list_tools`,
  and each per-tool `call_tool` await in `tokio::time::timeout`, mirroring the
  existing `warn!` arms (warn + skip). An always-mode server must never stall the turn.
- **ITEM-3**: Inner/native — in `stdio.rs::connect_native`, bound the handshake
  `().serve(transport).await` with `tokio::time::timeout`. On elapse, return the
  same `errors::upstream_error(name, UpstreamFailure::Unreachable, msg)` shape used
  on `serve()` failure (message noting the timeout), preserving stderr-capture on
  the error path where practical.
- **ITEM-4**: Inner/sandboxed — in `stdio.rs::connect_sandboxed`, bound both
  sandboxed `().serve(...).await` handshakes (LinuxBwrap child + VM AsyncRwTransport)
  with `tokio::time::timeout`, returning the same `Unreachable` error shape on elapse.
- **ITEM-5**: Timeout budget helper — a single small helper in `stdio.rs` that wraps
  a serve future in `tokio::time::timeout(Duration::from_secs(timeout_seconds.max(1)))`
  and maps `Elapsed` → `Unreachable` upstream_error, used by ITEM-3 + ITEM-4 (no
  magic numbers; one definition). The budget source is `server.timeout_seconds`
  (DEC-1). ITEM-1/ITEM-2 use `server.timeout_seconds.max(1)` inline for the outer
  `tokio::time::timeout` (no serve future to wrap there).
- **ITEM-6**: Breaker-recording at the OUTER connect-timeout arm (auto path) —
  because the outer `tokio::time::timeout` cancels `get_or_create_with_context`
  before `create_session_tracked → record_connection_failure` can run, the outer
  connect-timeout arm must itself call `session_manager.record_connection_failure`
  so INV-3 holds on the reachable tool-collection path (audit F1). Make
  `record_connection_failure` `pub(crate)` and split its body into a testable free
  fn `record_failure_into`. (DEC-5.)
- **ITEM-7**: Breaker-recording for ALWAYS-mode — always-mode builds sessions via
  `McpSession::new` directly, bypassing `create_session_tracked`, so its
  connect-timeout AND build-error arms must call `record_connection_failure` too,
  or a hanging always-mode server's breaker never opens (audit F4). Also, on a
  per-tool `call_tool` timeout in always-mode, `break` out of the tool loop rather
  than reusing a session whose transport has a cancelled in-flight request (audit
  F7). (DEC-6.) Round-2 completion (audit F-r2-4): always-mode also CONSULTS the
  breaker (`check_connection_breaker`, made `pub(crate)`) before dialing, so the
  opened breaker actually suppresses always-mode re-dials — otherwise INV-3's
  re-dial-suppression was wired but not behaving for always-mode.

## Files to touch

- `src-app/server/src/modules/mcp/chat_extension/mcp.rs` (ITEM-1, ITEM-2, ITEM-6, ITEM-7)
- `src-app/server/src/modules/mcp/client/stdio.rs` (ITEM-3, ITEM-4, ITEM-5)
- `src-app/server/src/modules/mcp/client/manager.rs` (ITEM-6: `pub(crate)` +
  `record_failure_into`; INV-3 breaker unit test)
- `src-app/server/tests/mcp/mcp_extension_test.rs` (TEST for INV-2 + the real
  stdio-hang INV-1/INV-2 acceptance)
- `src-app/server/tests/mcp/fixtures/hang_stdio_server.js` (hang fixture for TEST-7)
- in-source `#[cfg(test)]` in `stdio.rs` (INV-1 helper unit test) and
  `manager.rs` (INV-3 breaker unit test)

## Patterns to follow

- **Timeout budget + shape**: mirror `mcp/client/http.rs:1097`
  (`let timeout_secs = server.timeout_seconds.max(1) as u64;`) and the existing
  `errors::upstream_error(name, UpstreamFailure::Unreachable, msg)` calls already in
  `stdio.rs` (lines 231, 283, 296) — the timeout error must be byte-shape-identical
  to the existing serve()-failure error so it flows through `McpSession::new` →
  `manager.rs::create_session_tracked` → `record_connection_failure` unchanged.
- **Warn + skip arms**: mirror the EXISTING `tracing::warn!(… "— skipping"); continue;`
  arm at `mcp.rs:2446-2453` (auto-mode connect Err) and the always-mode `warn!` arms
  at `mcp.rs:2304-2310 / 2326-2333 / 2387-2394`. A timeout routes into the same arm.
- **Stalling HTTP stub for tests**: `tests/mcp/fixtures/mock_mcp_server.rs`
  `MockResponse::DelayedJsonOk { delay_ms, value }` + `on_method("initialize", …)`,
  as used by `tests/mcp/tool_call_timeout_test.rs`.
- **Breaker unit tests**: mirror `manager.rs` `mod breaker_tests` (`should_attempt_connect`,
  `record_connection_failure`).

## Non-goals / scope guard

No new permission, no migration, no new module, no public API/schema change, no
new settings row (DEC-1 reuses the existing `server.timeout_seconds`). No OpenAPI
regen (no handler-signature or schema change). Tier is LIGHT.

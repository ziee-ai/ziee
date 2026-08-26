# INFRA_INTEGRATION — mcp-toolcollect-timeout

The three mandatory Phase-5 walks. This is a backend-only robustness fix with no
UI surface and no new entity, so the walks are scoped accordingly.

## (1) User-experience walk

- **Who hits it**: any user who sends a chat message in a conversation with ≥1
  attached MCP server. Before the fix, if ANY attached server was misconfigured
  and hung on the stdio `initialize` handshake, the whole send stalled — the LLM
  was never called and the user saw an EMPTY assistant message with no error
  (observed live: 2,606 junk stdio servers → every send produced 0 content blocks).
- **After the fix**: a hanging/failed server is bounded by its `timeout_seconds`,
  logged (`warn! … — skipping`), and skipped. The user's send proceeds with the
  reachable servers' tools; the assistant replies normally. No new user-facing
  surface, message, or setting — the behavior simply stops being broken.
- **Configurability the user/admin already has**: the per-server `timeout_seconds`
  (MCP server drawer) now also governs connect/handshake + tool-collection, so an
  admin who wants a longer/shorter budget already has the knob (DEC-1/DEC-2).

## (2) Infrastructure-integration walk

Subsystems the change touches, and how each is handled (not assumed):

- **Chat pipeline / `before_llm_call`**: the collection loop is the entry point.
  A timeout is routed into the SAME `warn + continue` arm as the existing
  connect/list `Err` arms, so the loop's contract (skip a bad server, keep the
  rest, still build the LLM request) is unchanged — only its failure coverage
  widens from "errored" to "errored OR timed out".
- **MCP session manager + circuit breaker** (`manager.rs`): the inner stdio
  timeout returns the existing `upstream_error(Unreachable)` Err, which flows
  through `McpSession::new` → `create_session_tracked` → `record_connection_failure`,
  so a repeatedly-hanging server now OPENS the breaker (before, a hang never
  returned, so the breaker never tripped and every turn re-dialed). The outer
  mcp.rs timeout deliberately does NOT record a failure (it warns+continues) —
  that is why BOTH layers exist: outer = never stall the turn; inner = open the
  breaker.
- **stdio transport** (`stdio.rs`): native + both sandboxed (LinuxBwrap / VM)
  handshakes are bounded. The native timeout path still drains the child's
  captured stderr to the LOG (never the HTTP body — the existing security note is
  preserved) and returns the stable `Unreachable` message.
- **HTTP transport**: already bounded by the reqwest overall timeout
  (`http.rs:1097`, same `timeout_seconds.max(1)`). The new outer timeout is
  defense-in-depth here; the change is consistent with the HTTP path's existing
  budget.
- **Always-mode pre-run**: session build, `list_tools`, and each `call_tool` are
  bounded so an always-mode server cannot stall the turn either.
- **Approval flow / tool-call recording / sync**: untouched — the change only
  bounds tool-COLLECTION awaits; dispatch, approval, `mcp_tool_calls` recording,
  and sync emission are unchanged. A skipped server simply contributes no tools.
- **Permissions**: no change — no new permission; accessibility is still resolved
  the same way before collection.

## (3) Entity-lifecycle walk

No new entity is introduced. The only "entity" in scope is a transient MCP
`session`:
- **create**: bounded by the handshake timeout; on timeout the session is never
  created and the breaker records a failure.
- **failure/timeout**: warn + skip at the loop; breaker opens at the manager.
- **remove/drop**: on the native timeout path the transport is dropped when
  `serve()`'s future is cancelled by `tokio::time::timeout`, which kills the child
  (rmcp's `TokioChildProcess` drops → child killed) and closes the stderr pipe —
  no orphaned child. Sandboxed paths hold their inflight/VM guards only on the
  success arm, so a timed-out sandboxed connect drops its transport without
  leaking a mount/VM guard.
- No persisted rows, no cache dirs, no FS artifacts created by this path.

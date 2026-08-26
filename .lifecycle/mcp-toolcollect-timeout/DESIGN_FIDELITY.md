# DESIGN_FIDELITY — mcp-toolcollect-timeout

One fidelity verdict per invariant (design = CODING_GUIDELINES.md §2/§5/§6 + diagnosis).

- **INV-1** — fidelity: UPHELD — ITEM-3/ITEM-4/ITEM-5 wrap every stdio handshake
  `serve()` await (native + both sandboxed) in `tokio::time::timeout`, and ITEM-1/ITEM-2
  bound the outer collection awaits. No unbounded connect/handshake await remains on
  the tool-collection path, satisfying §2's "always set connect/timeout on every
  external client".
- **INV-2** — fidelity: UPHELD — ITEM-1/ITEM-2 route an elapsed/failed server into the
  existing `warn + continue` arms, so the collection loop tolerates it and still builds
  the LLM request with the reachable servers' tools (§6: surface + skip, never swallow
  into a hang; fixes the empty-assistant-message bug).
- **INV-3** — fidelity: UPHELD — ITEM-3/ITEM-4 return the existing
  `upstream_error(Unreachable)` on elapse, which flows through `create_session_tracked`
  → `record_connection_failure` when the INNER timeout wins (e.g. the test-connection
  path). On the tool-collection path the OUTER timeout wins (audit F1), so ITEM-6/ITEM-7
  explicitly call `record_connection_failure` in the auto and always-mode
  connect-timeout/build-error arms — the breaker therefore opens regardless of which
  timer fires, on both usage modes. Round 2 (F-r2-4) additionally made always-mode
  CONSULT the breaker (`check_connection_breaker`) before dialing, so a hanging
  always-mode server that already tripped the breaker is skipped rather than
  re-dialed every turn — the re-dial-suppression half of INV-3 now holds for
  always-mode too (auto-mode already consults it via `get_or_create*`).

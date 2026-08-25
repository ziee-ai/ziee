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
  → `record_connection_failure`, opening the breaker on a repeatedly-hanging server
  (§5). The outer timeout alone would NOT do this (it warns+continues without recording),
  which is exactly why both layers are required.

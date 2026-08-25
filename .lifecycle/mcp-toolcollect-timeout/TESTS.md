# TESTS — mcp-toolcollect-timeout

Bipartite coverage: every ITEM has ≥1 TEST; every INV has ≥1 `[acceptance]` test tagged `[invariant: INV-N]`. Backend-only diff → no e2e tier required (no `src-app/ui/**` touched; no new permission).

## Acceptance tests (design-invariant proofs)

- **TEST-1** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-3, ITEM-4, ITEM-5] file: `src-app/server/src/modules/mcp/client/stdio.rs` — asserts: the stdio handshake-timeout helper, given a `std::future::pending()` serve future (a handshake that NEVER completes) and a 1s budget, returns Err (an Unreachable upstream_error) within ~2s wall-clock rather than hanging; and given a ready Ok future passes the value through unchanged. Would FAIL (hang → test-timeout) if the serve() await were left unbounded.
- **TEST-2** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-1, ITEM-2] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: with one HEALTHY HTTP MCP server (normal initialize + tools/list) and one STALLING HTTP MCP server (initialize DelayedJsonOk huge delay, timeout_seconds=1) both attached, before_llm_call completes in bounded time and the built LLM ChatRequest CONTAINS the healthy server's tool(s); the send is not aborted by the stalling server. Would FAIL if the loop propagated the stalling server's error instead of warn+skip.
- **TEST-3** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-3, ITEM-4, ITEM-6, ITEM-7] file: `src-app/server/src/modules/mcp/client/manager.rs` — asserts: driving the REAL production recording fn `record_failure_into` (the body of `record_connection_failure`, now called by the outer connect-timeout arm and the always-mode failure arm) with the exact timeout-origin upstream_error(Unreachable) opens the breaker: consecutive==1 after the first timeout, should_attempt_connect returns false, a second timeout deepens the streak to 2, and an unrelated server is not suppressed. Exercises production increment/stamp logic, so it FAILS if that logic breaks (not a hand-built state).
- **TEST-7** (tier: integration) [acceptance] [invariant: INV-1] [covers: ITEM-3, ITEM-5] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: a REAL stdio server whose child spawns (embedded Bun via the node launcher) but never completes the MCP initialize handshake has its handshake TIME-BOUNDED on the real `connect_native` -> `().serve()` path: creating the server runs the connection health probe (the exact unbounded handshake before the fix), and the create returns in bounded time (< 20s) AND takes at least ~the configured 2s handshake timeout (>= 1.5s, proving the handshake genuinely hung until `with_handshake_timeout` fired, not a fast spawn failure). Reproduces the real unbounded-stdio-serve() bug on the real path; nothing but the new stdio handshake timeout makes create terminate. (INV-2 turn-path tolerance is covered by TEST-2/TEST-6.)

## Item-coverage tests

- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/mcp/client/stdio.rs` — asserts: the helper floors the budget to timeout_seconds.max(1) — a 0 or negative configured timeout becomes 1s, never a 0-duration timeout that fires instantly (boundary test of the budget math shared by ITEM-3/ITEM-4).
- **TEST-5** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: regression/no-op on the healthy path — a single healthy auto-mode HTTP MCP server's tools are still collected into the LLM request unchanged (the timeout wrap does not alter the success path); mirrors test_mcp_tools_added_to_llm_request.
- **TEST-6** (tier: integration) [covers: ITEM-2] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: an always-mode server that STALLS on initialize does not stall the turn — before_llm_call returns in bounded time and downstream auto-mode tools are still collected (always-mode path of INV-2, distinct from TEST-2's auto-mode subject).

## INV → acceptance-test map

- INV-1 → TEST-1, TEST-7
- INV-2 → TEST-2 (+ TEST-6 always-mode)
- INV-3 → TEST-3

## Plan-coverage (FB-7)

Every ITEM is covered: ITEM-1→TEST-2,TEST-5; ITEM-2→TEST-2,TEST-6; ITEM-3→TEST-1,TEST-3,TEST-7; ITEM-4→TEST-1,TEST-3; ITEM-5→TEST-1,TEST-4,TEST-7; ITEM-6→TEST-3; ITEM-7→TEST-3. No `[DESCOPED]` items.

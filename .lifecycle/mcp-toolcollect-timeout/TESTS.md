# TESTS — mcp-toolcollect-timeout

Bipartite coverage: every ITEM has ≥1 TEST; every INV has ≥1 `[acceptance]` test tagged `[invariant: INV-N]`. Backend-only diff → no e2e tier required (no `src-app/ui/**` touched; no new permission).

## Acceptance tests (design-invariant proofs)

- **TEST-1** (tier: unit) [acceptance] [invariant: INV-1] [covers: ITEM-3, ITEM-4, ITEM-5] file: `src-app/server/src/modules/mcp/client/stdio.rs` — asserts: the stdio handshake-timeout helper, given a `std::future::pending()` serve future (a handshake that NEVER completes) and a 1s budget, returns Err (an Unreachable upstream_error) within ~2s wall-clock rather than hanging; and given a ready Ok future passes the value through unchanged. Would FAIL (hang → test-timeout) if the serve() await were left unbounded.
- **TEST-2** (tier: integration) [acceptance] [invariant: INV-2] [covers: ITEM-1, ITEM-2] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: with one HEALTHY HTTP MCP server (normal initialize + tools/list) and one STALLING HTTP MCP server (initialize DelayedJsonOk huge delay, timeout_seconds=1) both attached, before_llm_call completes in bounded time and the built LLM ChatRequest CONTAINS the healthy server's tool(s); the send is not aborted by the stalling server. Would FAIL if the loop propagated the stalling server's error instead of warn+skip.
- **TEST-3** (tier: unit) [acceptance] [invariant: INV-3] [covers: ITEM-3, ITEM-4] file: `src-app/server/src/modules/mcp/client/manager.rs` — asserts: recording a connection failure whose error is the exact timeout-origin upstream_error(Unreachable) value the stdio timeout returns opens the breaker — should_attempt_connect(Some(&state), now) returns false inside the cooldown after the failure is recorded. Would FAIL if a timeout Err did not increment the breaker.

## Item-coverage tests

- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/mcp/client/stdio.rs` — asserts: the helper floors the budget to timeout_seconds.max(1) — a 0 or negative configured timeout becomes 1s, never a 0-duration timeout that fires instantly (boundary test of the budget math shared by ITEM-3/ITEM-4).
- **TEST-5** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: regression/no-op on the healthy path — a single healthy auto-mode HTTP MCP server's tools are still collected into the LLM request unchanged (the timeout wrap does not alter the success path); mirrors test_mcp_tools_added_to_llm_request.
- **TEST-6** (tier: integration) [covers: ITEM-2] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: an always-mode server that STALLS on initialize does not stall the turn — before_llm_call returns in bounded time and downstream auto-mode tools are still collected (always-mode path of INV-2, distinct from TEST-2's auto-mode subject).

## INV → acceptance-test map

- INV-1 → TEST-1
- INV-2 → TEST-2
- INV-3 → TEST-3

## Plan-coverage (FB-7)

Every ITEM is covered: ITEM-1→TEST-2,TEST-5; ITEM-2→TEST-2,TEST-6; ITEM-3→TEST-1,TEST-3; ITEM-4→TEST-1,TEST-3; ITEM-5→TEST-1,TEST-4. No `[DESCOPED]` items.

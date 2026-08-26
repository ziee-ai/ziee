# FIX_ROUND-1 — mcp-toolcollect-timeout

Blind audit ran three angles (correctness, design-conformance, concurrency-resource)
over `git diff main...HEAD`. Ten confirmed findings; the two HIGH ones about INV-3
were corroborated by two angles each.

## Confirmed findings fixed

- **F1 (HIGH, 2 angles) — outer timeout shadows inner → breaker never opens (INV-3
  defeated on the auto path).** The outer `tokio::time::timeout` shares the inner
  stdio handshake budget and starts earlier, so on a hanging stdio server it always
  wins and cancels `get_or_create_with_context` before
  `create_session_tracked → record_connection_failure` runs. **Fix (DEC-5/ITEM-6):**
  the auto outer connect-timeout arm now calls
  `session_manager.record_connection_failure(*server_id, &err)`. `record_connection_failure`
  is `pub(crate)`; its body is split into a testable free fn `record_failure_into`.
- **F2 (HIGH+MEDIUM, 2 angles) — TEST-3 hollow/tautological.** Rewrote TEST-3 to drive
  the REAL `record_failure_into` with the timeout-origin `Unreachable` error and assert
  the breaker opens (consecutive 1→2, `should_attempt_connect` false, unrelated server
  not suppressed). It now FAILS if the recording logic breaks.
- **F3 (HIGH) — INV-2 tests hollow; no real stdio acceptance.** Added **TEST-7**: a
  real stdio server (`node`→embedded Bun fixture `hang_stdio_server.js`) that spawns
  but never completes the handshake, attached beside a healthy HTTP server. Asserts the
  send is bounded (< 20s) AND actually hung (>= 1.5s, so it's not a fast spawn failure)
  AND the healthy tool reaches the LLM. This is the only test that reproduces the real
  unbounded-`serve()` bug.
- **F4 (MEDIUM) — always-mode bypasses the breaker.** Always-mode connect-timeout AND
  build-error arms now call `record_connection_failure` (DEC-6/ITEM-7).
- **F7 (LOW) — always-mode reuses a session after a cancelled call_tool.** On a
  `call_tool` timeout the tool loop now `break`s instead of reusing the possibly-desynced
  session (DEC-6/ITEM-7).

## Confirmed findings dispositioned WONTFIX (DEC-7)

- **F5 (MEDIUM)** — N stalling servers cost up to N×timeout on the FIRST turn (serial
  collection). Inherent; the breaker fix short-circuits subsequent turns; parallelizing
  collection is out of scope for a LIGHT hardening fix.
- **F6 (LOW)** — auto path holds the per-session write guard across the bounded
  `list_tools`. Pre-existing and strictly improved by the bound; not a regression.
- **F8 (LOW)** — sandboxed handshake-timeout drops the inflight/VM guards at the early
  return; verified no leak, byte-identical teardown to the existing `serve()`-Err path.

## New confirmed findings: 0

LIGHT tier: one audit round complete; all confirmed HIGH/MEDIUM findings are fixed or
have a recorded WONTFIX disposition. No finding concentrated on a guard file (no
GUARD-SUB), profile is not flat/rising (round 1). Loop terminates per the LIGHT rule.

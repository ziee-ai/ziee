# FIX_ROUND-1 — sandbox-tool-approval-loop

## Fixes applied for the Phase-6 blind-audit findings (LEDGER.jsonl)

- **L1 (state-management, map leak, medium)** — FIXED. Bounded `tool_name_server_map`
  with `MAX_PENDING_TOOL_MAPS = 1024`: on overflow (only reachable via streams that
  abort before finalize orphan their entry) the best-effort recovery cache is cleared,
  so growth is bounded. Degrades at most to a clear "could not resolve" error that
  self-heals next turn.
- **L2 (correctness, `__`-in-name mis-recovery, low)** — FIXED. Reworked finalize
  resolution into the pure helper `resolve_server_and_tool`: a well-formed
  `<uuid>__tool` splits on the first `__` with a UUID-validated prefix; a prefix-less
  name is recovered by trying the WHOLE name and — only for an empty-prefix `__tool` —
  the remainder after the leading `__`. Handles `execute_command`, `__query_rag`, and
  `get__weather`; keeps a middle `__` as part of the tool name.
- **L4 (perf, O(n^2) blob load, medium)** — FIXED. Replaced `get_message_with_content`
  (loads + parses ALL blocks incl. large tool_result blobs) with a targeted
  `SELECT content FROM message_contents WHERE message_id=$1 AND content_type='tool_use'`
  (indexed `content_type`, bound param), so only the small tool_use rows are loaded.
- **L5 (tests-quality, unique-id not exercised e2e, medium)** — FIXED. Added an
  end-to-end assertion to `mcp_approval_loop_bare_name_recovers_and_executes`: the
  resume re-emits the same provider id (`tool_use`) and the test asserts the new pending
  approval carries a freshly minted `call_` id (≠ the first), exercising the `used_ids`
  DB-seed cross-iteration dedup through the real chat/finalize/approval path.
- **L3 (error-handling, DB-error degrade, plausible)** — REJECTED (accepted tradeoff).
  On a transient DB error the seed degrades to within-batch dedup only. Failing the whole
  finalize on a DB read error would be strictly worse; the degrade is safe and only
  weakens a defensive net for the (unobserved) constant-id-model case. No change.

## Re-audit (blind round 7) results

- **R1 (correctness, 2-candidate mis-dispatch, plausible→confirmed)** — FIXED. The first
  candidate rework could recover a non-advertised `get__weather` to a DIFFERENT server's
  `weather` tool via the post-`__` suffix. Restricted the suffix candidate to empty-prefix
  (`strip_prefix("__")`) names only, so a middle `__` is never stripped. Added 7 unit
  tests on `resolve_server_and_tool` (well-formed, double-underscore-in-name, bare,
  empty-prefix, middle-`__`-not-mis-dispatched, middle-`__`-advertised, unknown).
- **R2 (concurrency, `guard.clear()` wipes all, plausible/low)** — REJECTED (accepted
  tradeoff). Reaching the cap needs 1024 concurrently-orphaned streams; the clear degrades
  a concurrent bare-name call to a clear error that self-heals next turn. Both audit agents
  judged it acceptable and it is documented at the call site. An LRU/orphan-only eviction
  would add disproportionate complexity for a scenario that can't arise in normal operation.

Tests after fixes: 13 unit + 2 integration PASS.

**New confirmed findings:** 1

# FIX_ROUND-2 — converge

## Fix applied
- **TEST-8 reworked** (`group_assistant_blocks_dedups_duplicate_result`) — now uses two
  co-pending tool_uses (A, B) so the duplicate A result arrives BEFORE the flush, genuinely
  exercising the keep-first `results_by_id.entry().or_insert` dedup. Verified it now FAILS
  against pre-fix code (the flat `current_results` vec would carry `[first(A), dup(A), rb(B)]`
  → `result_ids == [A,A,B]`, failing the `[A,B]` assertion) and passes post-fix.

## Re-audit (blind round 3)
One fresh diff-only auditor reviewed the final state (correctness, edge-cases, api-contract,
tests-quality), with explicit attention to the reworked dedup test and the mixed
approval-batch concern. Result: **no real reachable defects**. It independently confirmed:
- the reworked dedup test genuinely exercises keep-first and fails pre-fix;
- `flush_assistant_tool_pair` pairs strictly by id in order, synthesizes `is_error` with the
  carried name, drops orphans;
- `batch_has_result` correctly separates completed-partial from awaiting-approval;
- TEST-1/5a/6/7 all genuinely fail pre-fix and assert real wire-format behavior (no mock);
- the summarizer snap guard is correct and provider-safe.

**New confirmed findings:** 0

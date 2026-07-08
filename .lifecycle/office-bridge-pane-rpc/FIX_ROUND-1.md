# FIX_ROUND-1 — office-bridge pane RPC

Merged the phase-6 ledger, fixed every confirmed finding (and the two suspected
mutation-safety findings, since they are legitimate), then re-ran a full blind round
(2 fresh agents over the updated diff `/tmp/pane-rpc-v2.diff`).

## Fixes applied from the phase-6 ledger (committed in 83791e9b)

- **[high] resolve_pane basename-shadows-exact** → exact-path match now wins over
  basename; ambiguous basename → none.
- **[high] route_response cross-pane spoofing** → replies are bound to the pane a
  request was routed to (`route_response(from_pane, resp)`); a reply from another
  pane is rejected + logged.
- **[high] sole-pane wrong-doc mutation** → sole-pane fallback only for an unsaved
  (empty-key) doc.
- **[med] close doesn't fail in-flight** → `unregister_pane` drains + drops PENDING
  for the pane (fast-fail → NOT_CONNECTED, no 15s hang).
- **[med] read_document uncapped** → `capText` caps Word/Excel output.
- **[med] register untrusted input** → host/doc_key length-capped.
- **[med] pane ignores doc_id** → taskpane.js validates the request target vs its own
  document.
- **[med] no positive multi-pane test** → TEST-15 added.
- **[low] get_tracked_changes PPT inconsistency** → grouped with the Word-only pre-gate;
  pane `-32002` → `OFFICE_UNSUPPORTED_ON_HOST`.
- **[low] BridgeResponse.jsonrpc no default / result-XOR-error / timeout copy / stale-id
  logging** → all addressed.

## Re-audit round (2 blind agents on the updated diff)

Agent A (correctness/concurrency/security): confirmed the concurrency, response-binding,
TOCTOU, and lock-ordering are correct. Raised 2 **suspected** residual mutation-safety
issues — fixed here:
- **[suspected/med] empty-key sole-pane binds a named-path target** → resolve_pane now
  requires the target to be a bare name (no path separator) for the empty-key sole-pane
  fallback; a named path errors NOT_CONNECTED instead of mutating an unsaved doc.
- **[suspected/med] basename match crosses directories** → taskpane.js's target guard
  now compares full normalized paths (`sameDoc`) when both are path-like, so
  `/work/Report.docx` vs `/personal/Report.docx` is rejected, not silently mutated.

Agent B (error-handling/api-contract/tests-quality): verified error-code mapping,
test determinism, and the serde default are correct. 2 **confirmed** (low) + 1
suspected — all fixed here:
- **[confirmed/low] serde default untested** → added `protocol.rs`
  `response_deserializes_without_jsonrpc_field`.
- **[confirmed/low] stale dispatch_tool doc comment** ("returns requires-task-pane
  until that RPC lands") → rewritten to describe the now-live broker routing.
- **[suspected/low] truncation only in structured flag** → `capText` now appends an
  IN-BAND `…[truncated]` marker to the readable text channel.

## New confirmed findings: 2

The re-audit of the post-ledger-fix diff surfaced **2 new confirmed** (low) findings
(serde-default untested; stale dispatch_tool doc comment) plus 3 suspected (2 med
mutation-safety, 1 low truncation). All were fixed in this round's follow-up commit.
Because the re-audit found new confirmed findings, the loop is NOT yet converged — a
second re-audit round (FIX_ROUND-2) re-checks the post-fix diff.

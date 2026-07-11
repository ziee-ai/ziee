# FIX_ROUND-1 — merge ledger → fix → re-audit

Blind round 1 ran 4 fresh diff-only auditors across 10 angles (correctness, edge-cases,
concurrency, api-contract, patterns-conformance, tests-quality, error-handling, security,
perf, state-management). See `LEDGER.jsonl`.

## Fixes applied this round

- **TEST-7** added (`group_assistant_blocks_drops_mid_stream_orphan_result`) — the
  tests-quality finding that the orphan-drop coverage exercised only trailing orphans
  (which pre-fix also dropped), not the mid-stream orphan `[use A, result X, result A]`
  that actually changed behavior. New test fails against pre-fix code.
- **TEST-8** added (`group_assistant_blocks_dedups_duplicate_result`) — covers the
  first-wins duplicate-result dedup that was untested.
- **Docstring accuracy** — the `group_assistant_blocks` doc no longer overclaims "ALWAYS
  upheld"; it now scopes the guarantee to batches whose tools have run and documents the
  awaiting-approval exception explicitly (correctness + state-management overclaim finding).
- **Summarizer `drop_until > system_prefix_len` guard** — proactive hardening so a
  `message_count == 0` (drop-nothing) case never snaps a leading Tool.

## Findings rejected (with rationale) — not code defects

- **Zero-result completed batch → dangling** (correctness/api-contract, medium/low):
  rejected-unreachable. The MCP layer persists a real OR is_error tool_result for EVERY
  executed tool (verified across all `mcp.rs` failure branches), so a completed batch
  always has ≥1 result; the zero-result trailing case is exclusively awaiting-approval,
  whose result is appended separately. Docstring now states this.
- **Mixed partial-approval `[use A, use B, result A]` synthesizes B prematurely**
  (edge-cases, low): rejected-unreachable. The Continue handler appends all of an
  iteration's tool_results together (all-or-nothing per parallel batch), so a stored
  message never holds some-results/some-awaiting for one batch.
- **Duplicate tool_use ids** (edge-cases, low): rejected-out-of-scope — malformed input
  the assembler neither creates nor is tasked to repair; providers reject duplicate ids
  upstream regardless.
- **Gemini identical-name parallel functionResponse ambiguity** (api-contract, low):
  rejected-out-of-scope — pre-existing Gemini-adapter limitation that affects real results
  identically; not introduced by this fix.
- **clear_old_tool_results rewrites a synthesized id → get_tool_result empty**
  (state-management, low): accepted-benign — the result was already absent (that is why it
  was synthesized), so recall correctly returns not-found; no correctness impact.

## Positive confirmations (no action)

- Synthesized `ToolResult` converts correctly for Anthropic (by tool_use_id), OpenAI (by
  tool_call_id), and Gemini (functionResponse by carried name) — the fix does NOT regress
  OpenAI/Gemini.
- The summarizer forward-snap is provider-safe and preserves the `[System*, Summary, …]`
  invariant.
- Both functions remain pure/synchronous; no unwrap/index panics; no secret/id leak; O(n)
  with no algorithmic regression.

## Re-audit (blind round 2)

Two fresh diff-only auditors re-audited the post-fix state (correctness/edge/contract and
tests/patterns/state). Result: **1 new confirmed finding** —

- **tests-quality (medium)**: `group_assistant_blocks_dedups_duplicate_result` used a
  single tool_use, so the first result flushed immediately and the duplicate was dropped as
  a trailing orphan by BOTH pre- and post-fix code — the test passed against unfixed code
  and never exercised the keep-first `or_insert` branch it is named for. → Carried to
  FIX_ROUND-2.

The correctness/edge/api-contract re-auditor found **0** production-logic defects, and
additionally clarified the mixed builtin+awaiting-approval batch: it IS reachable, but the
synthesized `is_error` is a valid, transient, never-persisted payload (LEDGER line 5 updated
from "unreachable" to "reachable-but-correct").

**New confirmed findings:** 1

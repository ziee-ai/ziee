# FIX_ROUND-3 (Iteration 2 — resume-chain flicker)

## Context
Human review (FB-3) found the iteration-1 fix did NOT cover the multi-tool / tool-approval flow.
Branch was updated onto current `origin/khoi` (merge), which brought `#137` (tool_use/tool_result
pairing) + `#138` (stale-artifact). On the MERGED backend the multi-tool empty-completion NOTICE is
already resolved by those; the remaining frontend defect was the RESUME disappear (ITEM-8).

## Fix applied
- `resumeOrFreshPlaceholder` (messageWindow.ts) + its use in the `content` handler: a tool-approval
  resume continues the SAME assistant message id, so the streaming placeholder must REUSE the
  existing assistant row instead of overwriting it with `contents: []` (which blanked the row →
  `ChatMessage` bails to null on zero blocks → bubble vanished then reappeared).

## Blind audit (fresh auditor, diff-only)
Verified against the backend contract: `streaming.rs` resumes the same `assistant_message_id`,
retains its content, and appends only new deltas → reuse-then-append is correct (no duplicate text,
no duplicate tool_use — the MCP extension's `existsInStreaming` dedup matches the adopted row, no
sequence gaps). Genuinely-new turns are byte-identical. Teardown paths safe with an adopted row.

- **Fixed this round**: the new object-aliasing invariant (`streamingMessage` is now
  reference-identical to the persisted row) was undocumented → documented the copy-on-write
  requirement in the helper docstring.
- Rejected (non-actionable / not regressions): first-delta bypasses `provideStreamingContent`
  (equivalent block, replaced by DB truth at complete); a resume that OPENS with a brand-new
  tool_use spawns a synthetic second row (PRE-EXISTING, does not recur the fixed symptom).

## Live validation (real gpt-oss + tool approvals, merged-code backend)
Multi-tool fetch turns with approve-and-resume: before the fix the assistant bubble DISAPPEARED
mid-turn; after the fix, across runs with 1 and 3 approvals: **no disappear, no notice**.

**New confirmed findings:** 0

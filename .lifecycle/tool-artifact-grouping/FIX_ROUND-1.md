# FIX_ROUND-1 — tool-artifact-grouping (follow-up)

Merged the phase-6 ledger, fixed the confirmed findings, then ran a fresh blind
re-audit of the fix delta.

## Fixes applied

1. **Single-tool wrapper name duplication** (LEDGER: regressions/correctness —
   MEDIUM, flagged by 2 independent angles). The single-tool wrapper showed the
   tool name+server+status in BOTH the group header AND an inner
   McpToolCallUI/McpToolUseRenderer card. Fixed: the wrapper header now stands in
   for the tool-call block, and the expanded body renders ONLY the remaining
   blocks (the tool_result / files) — `run.filter(b => !singleUse || hasError ||
   b.content_type !== 'tool_use')`. A multi-tool group is unaffected (`singleUse`
   is null → renders every member).

2. **Tautological "agreement" unit test** (LEDGER: tests-quality — MEDIUM).
   Re-framed to assert `shouldWrapRun`'s determinism + totality (the property that
   actually lets `McpToolUseGroup` and `contentSpan` share one decision); the
   end-to-end no-desync is exercised by the wrapping e2e. No longer re-derives
   `consumed` from the same call.

3. **Unguarded single-tool cast + missing matchMedia check** (LEDGER:
   error-handling — LOW). `toolUses[0]?.content as … | undefined` + `title` on the
   header name; `typeof window.matchMedia === 'function'` guard before calling it.

## Re-audit (fix delta) + its findings

A fresh blind agent reviewed the option-A delta. No new HIGH/MEDIUM. It raised two
LOWs, handled as:

- **error+artifact edge** (a tool_result carrying BOTH `resource_links` and
  `is_error` would wrap yet drop the error-text card): **FIXED** — the body filter
  keeps the tool_use card when `hasError`, so the error alert/message stays visible
  (the normal artifact flow never sets `is_error` on a `resource_links` result, so
  this is a belt-and-suspenders guard). Verified by inspection + tsc; the common
  single-tool success path is unchanged (e2e green).
- **args/result-text not shown in a single-tool wrapper** (MessageFilesView renders
  only files): **accepted by design** — an artifact tool's output IS the file; a
  clean single card (no nested duplicate) is the goal, and multi-tool groups + bare
  non-artifact cards still expose args. Surfaced to the human in HUMAN_FEEDBACK as a
  deliberate design point they may revisit.

## Accepted-by-design / won't-fix-here (documented, not defects)

- The `scrolledApprovals` module Set grows by one id per approval per session
  (tiny, bounded); a re-pending approval after a POST failure isn't re-scrolled
  (the user just interacted with it).
- `Stores.McpServer.servers` reactive read on every group card matches the
  `McpToolUseRenderer` pattern; server-list changes are rare (negligible re-render).
- The mid-stream reflow (plain card → wrapped when the artifact arrives) is inherent
  and functionally correct (contentSpan + render stay in lockstep).
- No aria-live announcement on the approval scroll / truncate-without-title mirror
  pre-existing approval/tool-card patterns (a broader a11y improvement, out of scope).

## Verification

Fresh-build e2e (07-mcp + reconciled resource-links, `--workers=1`): 18 pass; the
single failure was a Playwright "Channel closed" browser-infra flake on a #133
test that passes in every other run (not an assertion failure). tsc + unit green.

**New confirmed findings:** 0

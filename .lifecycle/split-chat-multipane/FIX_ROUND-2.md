# FIX_ROUND-2 — phase-6 re-audit of the post-audit hunks

FIX_ROUND-1 converged the ORIGINAL blind audit to 0. This round covers the hunks
added AFTER that audit — the phase-7 composer fix (`TextInput.tsx`), the testid
uniqueness fix, the `SplitView.store.test.ts` unit suite, and the five
`14-split-chat/` e2e specs — which the AUDIT_COVERAGE coverage law now requires to
be reviewed by ≥3 distinct angles each. Five blind fork-auditors ran
(correctness, tests-quality, state-management, patterns-conformance,
error-handling); every new hunk is covered ≥3 angles in `AUDIT_COVERAGE.tsv`.

## Confirmed findings this round (2 MEDIUM + 5 LOW)

- **MEDIUM** `persistence.spec.ts:60` (correctness) — the reload width assertion
  could false-green if persistence silently reset the divider to default and the
  keyboard resize happened to move < tolerance. **Fixed:** capture the default
  width first, assert the resize diverged from default by > tolerance BEFORE
  reload, and assert the reloaded width still diverges from default (not just ≈
  widthBefore). Re-ran the full suite green.
- **MEDIUM** `SplitView.store.test.ts:56` (tests-quality; flagged independently by
  the correctness auditor too) — the `closePane` focus assertion used
  `=== a || === c`, which would false-green a regression that changed
  neighbour-selection to `panes[idx-1]`. **Fixed:** pinned to `=== c` (the impl
  deterministically reassigns to `panes[idx]`), verified against the impl. 10/10
  unit pass.
- **LOW** `independent-input.spec.ts:89` (error-handling) — inline seed lacked a
  status check. **Fixed:** added `expect(res.status()).toBeLessThan(300)`.
- **LOW** `independent-scroll.spec.ts:16` (patterns-conformance) — seed used node
  `fetch()` not `page.request.post`. **Fixed:** converted to the Playwright
  request context, matching the sibling specs.
- **LOW** `SplitView.store.test.ts:110` (tests-quality) — `reorderPanes` `toIndex`
  out-of-bounds branch untested. **Fixed:** added the `reorderPanes(0, 9)` case.
- **LOW** `TextInput.tsx:32` (state-management) — the `as typeof Stores.Chat` cast
  could mask a future shape divergence. **Dismissed:** it is the established
  `Stores.Chat` bridge-type pattern used across the pane migration; sound today.
- **LOW** `independent-streaming.spec.ts:79` (error-handling) — pane-B-idle is a
  single point-in-time check. **Dismissed:** pane A's reply is fully awaited
  first, so a later cross-bleed frame is not a realistic path; the negative
  assertion is sufficient.

The correctness auditor returned NO-FINDINGS on `TextInput.tsx` + the unit suite
(unconditional hook, sound single-pane fallback, genuine invariants).

## Re-audit of the fixes

All fixes are test-assertion tightenings + one test-helper refactor — they make
the suite STRICTER, touch no production code, and each is verified by a green run
(`SplitView.store.test.ts` 10/10; full `14-split-chat/` e2e 6/6 after a fresh
`dist-e2e` rebuild). No new production behavior, no new confirmed defect.

**New confirmed findings:** 0

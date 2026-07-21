# FIX_ROUND-1 — collapse-border-overlay

Merged the round-1 ledger (three blind auditors: correctness/state/react,
tests/a11y/perf, css/patterns/regression) and fixed every confirmed finding.

## Structural outcome

The round-1 audit did not produce a list of patches — it invalidated the
approach. Two HIGH findings, raised independently by two auditors, showed the
ITEM-3 split broke height-bounding: the collapse DECISION is computed over the
whole message while the split made the clamp SCOPE only the trailing prose. I
reproduced the worse case directly (a long turn ending on a tool card rendered
**1044px with no collapsible and no toggle**, previously 384px).

Because the reproduction had already proven the inset ALONE fixes the reported
bug, the split was not load-bearing. Escalated to the user as an explicit option
picker rather than silently reversing their approved DEC-1; they chose to drop
it. `ChatMessage.tsx`, `collapsible.ts` and `collapsible.test.ts` are restored to
`origin/khoi` and the diff no longer touches them.

## Findings resolved

| # | angle | severity | resolution |
|---|---|---|---|
| 1 | correctness | high | ends-on-structural turn loses collapse → ITEM-3 reverted |
| 2 | correctness | high | decision/scope mismatch drops the toggle → ITEM-3 reverted |
| 3 | regression-risk | medium | virtualizer estimate flips to under-estimate → moot with revert |
| 4 | tests-quality | medium | TEST-8 assertion vacuous → spec rewritten, guard added |
| 5 | tests-quality | medium | order claim unbacked (prose excluded) → order signature includes prose |
| 6 | tests-quality | medium | theme loop was coverage inflation → loop now exercises theme-dependent behaviour |
| 7 | tests-quality | medium | the `clampedNodes.length > 0` consequence was unpinned → moot; TEST-5 added for height-bounding |
| 8 | tests-quality | low | global `querySelector` could measure the wrong turn → scoped to the assistant turn |
| 9 | maintainability | low | TEST-4 asserted the mechanism → now asserts the effect |
| 10 | css-correctness | medium | unenforced parent-padding invariant → documented; effect-level test |
| 11 | correctness | low | inset horizontal-only (top hairline) → carried into round 2 |
| 12 | maintainability | medium | `classifyNode` failed open on unknown types → moot with revert |
| 13 | react-correctness | low | node migration between parents loses local state → moot with revert |
| 14 | state-management | medium | measure() target silently narrowed → moot with revert |
| 15 | a11y | low | spacing seam at the split → moot with revert |
| 16 | css / perf / RTL | low | verified correct, no action |

Rejected (not defects): the RTL hypothesis (`mx`/`px` are symmetric and absent
from the lint's rule table), and the per-node classify cost (O(n) over disjoint
slices, dwarfed by element construction).

**New confirmed findings:** 0

# FIX_ROUND-1 — message-scroll-stability

Merged the LEDGER (17 findings across 12 angles from 3 blind agents), fixed every
CONFIRMED finding, then ran a full blind RE-AUDIT (fresh agent, diff-only) on the
changed files.

## Confirmed findings fixed

- **HIGH api-contract** (perf-spec regression): the body region rendered the
  `inline-file-preview-body` testid for EVERY expanded preview → broke
  `mcp-resource-links-perf.spec.ts`'s lazy-mount count. FIX: split the render —
  the heavy viewer body (`inline-file-preview-body`) mounts ONLY when `seen`; an
  unseen preview renders a same-height `inline-file-preview-skeleton` instead. The
  lazy-mount contract (`count(body) < COUNT`) is restored AND the height stays
  fixed.
- **MEDIUM perf/state** (×2 agents): whole-`files`-map subscription caused a
  scroll-driven re-render storm (markFileSeen re-rendered every mounted preview).
  FIX: scoped selector `useMessageViewStateStore(s => s.files[key])` (immer
  structural sharing → re-renders only on this key's change). Same scoped selector
  applied to CollapsibleBlock's collapsed flag.
- **MEDIUM concurrency**: drag handle had no `pointercancel`/`lostpointercapture`
  → a cancelled gesture left a stuck drag. FIX: added `onPointerCancel` +
  `onLostPointerCapture` → `endDrag` (idempotent via the `dragStart` guard).
- **MEDIUM a11y**: `aria-valuemax` diverged from the real clamp ceiling
  (`max(400,0.8vh)`), producing an incoherent range on short viewports. FIX:
  `aria-valuemin={INLINE_FILE_MIN_PX}` / `aria-valuemax={maxReservedPx(vh)}`.
- **MEDIUM tests-quality** (×2): TEST-7 was largely self-consistent-by-construction
  and the pointer-drag path was untested. FIX: TEST-7 now asserts the body is
  FIXED at the 400px default (caps, not hugs, the 180px image) AND the enclosing
  message-row height is stable across image decode; added TEST-13 driving a real
  Playwright `page.mouse` pointer-drag + persistence.
- **LOW perf**: `commitDrag` called the store setter inside a `setState` updater
  (impure). FIX: commit from a `dragHeightRef` outside the updater.
- **LOW a11y**: missing `aria-controls`/`id` linkage. FIX: body region gets
  `id={bodyId}`; chevron + resize separator reference it.
- **LOW i18n**: aria bounds hardcoded — folded into the aria-valuemax fix.
- **LOW perf** (useInPlaceAnchor): overlapping rAF chains could double-adjust; a
  late virtualizer adjustment could make the 2-rAF read nudge. FIX: cancel the
  prior pending rAF chain on re-entry + on unmount, and BOUND the correction to
  ≤48px (a larger delta means the virtualizer is mid-adjusting → defer to it).
- **LOW patterns**: file→chat coupling + `as never` seed cast. FIX: added a
  clarifying comment on the (intentional) chat-extension coupling; typed the demo
  seed as `Partial<ChatState>` instead of `as never`.

## Rejected / not-fixed (with rationale)

- **LOW tests-quality — TEST-6/TEST-9 not fully discriminating**: acknowledged, not
  "fixed" by inflating the tests. TEST-6 is a legitimate idle-no-correction-storm
  guard; TEST-9 is the user-visible no-jump guarantee. The in-place-anchor MECHANISM
  (`inPlaceAnchorDelta`) is proven by the unit tests (TEST-3), which is the honest
  place for that logic. Recorded as a known scope note, not overclaimed.
- **LOW perf — findScrollParent per-keystep walk** (suspected): a getComputedStyle
  ancestor walk per key-repeat is negligible; the rAF-cancel fix also collapses
  overlapping chains. Not worth a cache.

## Re-audit (full blind round on the fix diff)

A fresh blind agent re-reviewed the rewritten files (correctness, concurrency,
state, error-handling, react-hooks, a11y, tests-quality). It confirmed the
scoped-selector equality, the rAF-cancel logic, drag idempotency, the testid
split, and the aria bounds are all correct, and found **no new high/medium
defects**. The ONE low/suspected item — a dangling `aria-controls` when collapsed
(body unmounted) + the separator referencing the `aria-hidden` skeleton — was
fixed in this same round: the chevron now sets `aria-controls` only while expanded,
and `aria-hidden` moved from the skeleton region onto its decorative inner element.

**New confirmed findings:** 0

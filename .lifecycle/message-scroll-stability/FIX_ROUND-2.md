# FIX_ROUND-2 — message-scroll-stability

Phase 8 (gated e2e run) surfaced a real ITEM-7 defect the earlier rounds' static
review had NOT caught, plus a fresh blind re-audit of that fix found more. This
round records both.

## Phase-8 discovery: the expand teleport (ITEM-7)

Running the enumerated e2e (`chat-scroll-stability.spec.ts`) caught **TEST-9
failing with a 1329px viewport jump** on expand. Root cause: when the expanding
row straddles the viewport-top fold, `@tanstack/react-virtual`'s OWN above-fold
size-change adjustment (`resizeItem`) compensates as if the growth were above the
fold, teleporting the viewport — and the hook's former 48px correction cap made
`useInPlaceAnchor` decline to fix it.

**Fix**: MessageList now imperatively assigns the virtualizer instance property
`shouldAdjustScrollPositionOnItemSizeChange` (read by virtual-core's resizeItem;
not a typed option) — returning FALSE for the row whose key is parked in the
shared `inPlaceAnchorSignal` during an intentional in-place change, so THAT row
grows downward from its current top instead of being auto-compensated;
`useInPlaceAnchor` parks/unparks the enclosing `[data-message-id]` around its
2-rAF pin (cap removed). Also fixed the two test-only bugs that masked coverage:
`scrollToMessage` now resets to top + brings the row into view (so file previews
cross the `seen` band and mount their real body), and TEST-13 drives the drag via
dispatched PointerEvents (Playwright synthetic mouse + setPointerCapture was
flaky); `setPointerCapture` guarded with try/catch. Demo inline image switched to
a PNG data URI (the image viewer doesn't claim `image/svg+xml`).

Result: **7/7 e2e pass**.

## Blind re-audit of the ITEM-7 fix — confirmed + suspected findings, all fixed

A fresh blind agent reviewed the FIX_ROUND-2 delta and found:

- **CONFIRMED medium (api-contract)** — the replicated default predicate was
  UNFAITHFUL to the actually-resolved `virtual-core@3.17.3` (not 3.14.5): it
  dropped the `+ scrollAdjustments` term and read the raw `scrollOffset` instead
  of `getScrollOffset()`. During a measurement burst `scrollAdjustments`
  accumulates, so non-parked above-fold rows the library WOULD adjust were being
  skipped — reintroducing exactly the estimate-correction / prepend-anchor drift
  this feature guards. **Fixed**: predicate now replicates 3.17.3 index.js:869
  verbatim (`item.start < getScrollOffset() + scrollAdjustments && (!has(key) ||
  scrollDirection !== 'backward')`); comment corrected to 3.17.3.
- **SUSPECTED low (concurrency)** — cross-row clobber of the module-singleton
  `inPlaceAnchorSignal`: if two different rows are resized within ~2 frames, row
  A's raf2 would unpark row B's key mid-change. **Fixed**: each hook records the
  key it parked and unparks (in raf2 AND cancel/unmount) ONLY if the signal still
  holds its own key (`unparkIfOwned`).
- **SUSPECTED low (correctness)** — `releasePointerCapture` was unguarded (`?.`
  guards undefined, not a throw); a spec-conformant engine (WebKit/Firefox)
  throws `NotFoundError` when no capture is active (synthetic pointer), which
  would abort the height commit. **Fixed**: wrapped in try/catch, symmetric with
  `setPointerCapture`.

The agent explicitly CLEARED: assigning the instance property every render is
safe (virtual-core never overwrites it; predicate can't throw); the unmount
cancel handles the stuck-key/leak case; the parked `[data-message-id]` IS the
correct virtualizer item key; TEST-9's 6px tolerance is meaningful (real teleport
~1300px); the dispatched PointerEvents DO reach the production handlers (not a
bypass).

All three findings fixed; tsc + 25 unit + 7/7 e2e green after the fixes. A final
blind re-audit of the three fixes is recorded in FIX_ROUND-3.

**New confirmed findings:** 1
